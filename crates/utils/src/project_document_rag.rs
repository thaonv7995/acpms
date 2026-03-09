use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const PROJECT_DOCUMENT_EMBEDDING_DIMENSIONS: usize = 256;
pub const PROJECT_DOCUMENT_CHUNK_TARGET_CHARS: usize = 1_200;
pub const PROJECT_DOCUMENT_CHUNK_OVERLAP_CHARS: usize = 180;
pub const PROJECT_DOCUMENT_MAX_CHUNKS: usize = 128;
pub const PROJECT_DOCUMENT_RUNTIME_TOP_K_LIMIT: usize = 8;

const INDEXABLE_PROJECT_DOCUMENT_APPLICATION_CONTENT_TYPES: &[&str] = &[
    "application/json",
    "application/yaml",
    "application/x-yaml",
    "application/xml",
    "application/toml",
    "application/javascript",
    "application/x-javascript",
    "application/typescript",
    "application/graphql",
    "application/sql",
];

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectDocumentChunkDraft {
    pub chunk_index: usize,
    pub content: String,
    pub content_hash: String,
    pub token_count: usize,
    pub embedding: Vec<f32>,
}

pub fn is_indexable_project_document_content_type(content_type: &str) -> bool {
    let normalized = normalize_content_type(content_type);
    normalized.starts_with("text/")
        || is_json_like_content_type(&normalized)
        || INDEXABLE_PROJECT_DOCUMENT_APPLICATION_CONTENT_TYPES.contains(&normalized.as_str())
}

pub fn normalize_project_document_text(content_type: &str, bytes: &[u8]) -> Result<String> {
    if !is_indexable_project_document_content_type(content_type) {
        return Err(anyhow!(
            "Unsupported content type for v1 indexing: {}",
            content_type
        ));
    }

    let text =
        String::from_utf8(bytes.to_vec()).map_err(|_| anyhow!("Document is not valid UTF-8"))?;
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let normalized_content_type = normalize_content_type(content_type);
    let normalized = match normalized_content_type.as_str() {
        value if is_json_like_content_type(value) => serde_json::from_str::<serde_json::Value>(&normalized)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok())
            .unwrap_or(normalized),
        _ => normalized,
    };

    Ok(normalized.trim().to_string())
}

pub fn build_project_document_chunks(text: &str) -> Vec<ProjectDocumentChunkDraft> {
    split_project_document_text(
        text,
        PROJECT_DOCUMENT_CHUNK_TARGET_CHARS,
        PROJECT_DOCUMENT_CHUNK_OVERLAP_CHARS,
        PROJECT_DOCUMENT_MAX_CHUNKS,
    )
    .into_iter()
    .enumerate()
    .map(|(chunk_index, content)| ProjectDocumentChunkDraft {
        chunk_index,
        content_hash: sha256_hex(&content),
        token_count: tokenize_for_search(&content).len(),
        embedding: embed_project_document_text(&content),
        content,
    })
    .collect()
}

pub fn split_project_document_text(
    text: &str,
    target_chars: usize,
    overlap_chars: usize,
    max_chunks: usize,
) -> Vec<String> {
    let normalized = text.trim();
    if normalized.is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = normalized.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let target_chars = target_chars.max(200);
    let overlap_chars = overlap_chars.min(target_chars / 2);
    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < chars.len() && chunks.len() < max_chunks.max(1) {
        let hard_end = (start + target_chars).min(chars.len());
        let mut end = hard_end;

        if hard_end < chars.len() {
            let soft_min = start + (target_chars / 2);
            if let Some(candidate) = find_breakpoint(&chars, soft_min, hard_end) {
                end = candidate;
            }
        }

        if end <= start {
            end = hard_end;
        }

        let chunk = chars[start..end].iter().collect::<String>();
        let trimmed = chunk.trim().to_string();
        if !trimmed.is_empty() {
            chunks.push(trimmed);
        }

        if end >= chars.len() {
            break;
        }

        let mut next_start = end.saturating_sub(overlap_chars);
        while next_start < chars.len() && chars[next_start].is_whitespace() {
            next_start += 1;
        }
        if next_start <= start {
            next_start = end;
        }
        start = next_start;
    }

    chunks
}

pub fn embed_project_document_text(text: &str) -> Vec<f32> {
    let features = build_embedding_features(text);
    let mut embedding = vec![0.0f32; PROJECT_DOCUMENT_EMBEDDING_DIMENSIONS];

    for feature in features {
        let hash = Sha256::digest(feature.as_bytes());
        let idx = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]) as usize
            % PROJECT_DOCUMENT_EMBEDDING_DIMENSIONS;
        let sign = if hash[4] % 2 == 0 { 1.0 } else { -1.0 };
        let weight = if feature.contains("__") {
            1.35
        } else if feature.chars().all(|ch| ch.is_ascii_digit()) {
            1.75
        } else {
            1.0
        };
        embedding[idx] += sign * weight;
    }

    normalize_vector(&mut embedding);
    embedding
}

pub fn cosine_similarity(lhs: &[f32], rhs: &[f32]) -> f32 {
    let len = lhs.len().min(rhs.len());
    if len == 0 {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut lhs_norm = 0.0f32;
    let mut rhs_norm = 0.0f32;
    for idx in 0..len {
        dot += lhs[idx] * rhs[idx];
        lhs_norm += lhs[idx] * lhs[idx];
        rhs_norm += rhs[idx] * rhs[idx];
    }

    if lhs_norm <= f32::EPSILON || rhs_norm <= f32::EPSILON {
        0.0
    } else {
        dot / (lhs_norm.sqrt() * rhs_norm.sqrt())
    }
}

pub fn score_project_document_chunk(
    query: &str,
    query_embedding: &[f32],
    chunk_content: &str,
    chunk_embedding: &[f32],
) -> f32 {
    let vector_score = cosine_similarity(query_embedding, chunk_embedding).max(0.0);
    let lexical_score = lexical_overlap_score(query, chunk_content);
    let phrase_bonus = phrase_overlap_bonus(query, chunk_content);
    (vector_score * 0.8) + (lexical_score * 0.45) + phrase_bonus
}

pub fn tokenize_for_search(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            if current.len() >= 2 || current.chars().all(|token| token.is_ascii_digit()) {
                tokens.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }

    if !current.is_empty()
        && (current.len() >= 2 || current.chars().all(|token| token.is_ascii_digit()))
    {
        tokens.push(current);
    }

    tokens
}

fn build_embedding_features(text: &str) -> Vec<String> {
    let tokens = tokenize_for_search(text);
    let mut features = Vec::with_capacity(tokens.len() * 2);
    for token in &tokens {
        features.push(token.clone());
    }
    for window in tokens.windows(2) {
        features.push(format!("{}__{}", window[0], window[1]));
    }
    features
}

fn lexical_overlap_score(query: &str, chunk_content: &str) -> f32 {
    let query_terms: HashSet<String> = tokenize_for_search(query).into_iter().collect();
    if query_terms.is_empty() {
        return 0.0;
    }

    let chunk_terms: HashSet<String> = tokenize_for_search(chunk_content).into_iter().collect();
    let overlap = query_terms.intersection(&chunk_terms).count();

    overlap as f32 / query_terms.len() as f32
}

fn phrase_overlap_bonus(query: &str, chunk_content: &str) -> f32 {
    let query_tokens = tokenize_for_search(query);
    if query_tokens.len() < 2 {
        return 0.0;
    }

    let chunk_tokens: HashSet<String> = build_embedding_features(chunk_content)
        .into_iter()
        .collect();
    let bigram_hits = query_tokens
        .windows(2)
        .map(|window| format!("{}__{}", window[0], window[1]))
        .filter(|feature| chunk_tokens.contains(feature))
        .count();

    if bigram_hits == 0 {
        0.0
    } else {
        (bigram_hits as f32 / (query_tokens.len() - 1) as f32) * 0.2
    }
}

fn find_breakpoint(chars: &[char], soft_min: usize, hard_end: usize) -> Option<usize> {
    (soft_min..hard_end)
        .rev()
        .find(|idx| chars[*idx].is_whitespace())
}

fn normalize_content_type(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase()
}

fn is_json_like_content_type(content_type: &str) -> bool {
    content_type == "application/json" || content_type.ends_with("+json")
}

fn normalize_vector(values: &mut [f32]) {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm <= f32::EPSILON {
        return;
    }
    for value in values.iter_mut() {
        *value /= norm;
    }
}

fn sha256_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_is_indexable() {
        assert!(is_indexable_project_document_content_type(
            "text/markdown; charset=utf-8"
        ));
        assert!(is_indexable_project_document_content_type("text/html"));
        assert!(is_indexable_project_document_content_type(
            "application/vnd.api+json"
        ));
        assert!(!is_indexable_project_document_content_type(
            "application/pdf"
        ));
    }

    #[test]
    fn json_is_pretty_printed_for_indexing() {
        let text = normalize_project_document_text("application/json", br#"{"a":1,"b":{"c":2}}"#)
            .expect("json should normalize");

        assert!(text.contains("\"b\": {"));
    }

    #[test]
    fn json_suffix_content_type_is_pretty_printed_for_indexing() {
        let text = normalize_project_document_text(
            "application/vnd.api+json",
            br#"{"data":{"id":"doc-1"}}"#,
        )
        .expect("json suffix content type should normalize");

        assert!(text.contains("\"data\": {"));
    }

    #[test]
    fn chunk_builder_creates_overlapping_chunks() {
        let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu ".repeat(80);
        let chunks = split_project_document_text(&text, 300, 60, 8);

        assert!(chunks.len() > 1);
        assert!(chunks[0].contains("alpha beta"));
        assert!(chunks[1].contains("lambda") || chunks[1].contains("kappa"));
    }

    #[test]
    fn search_score_prefers_related_chunk() {
        let query = "What is the secret code?";
        let query_embedding = embed_project_document_text(query);
        let matching = "Deployment checklist. The secret code is 998877. Keep it private.";
        let unrelated = "Frontend palette uses ocean blue and sand accents.";

        let matching_score = score_project_document_chunk(
            query,
            &query_embedding,
            matching,
            &embed_project_document_text(matching),
        );
        let unrelated_score = score_project_document_chunk(
            query,
            &query_embedding,
            unrelated,
            &embed_project_document_text(unrelated),
        );

        assert!(matching_score > unrelated_score);
    }

    #[test]
    fn chunk_drafts_include_embeddings() {
        let chunks = build_project_document_chunks(
            "Architecture overview\n\nUse Redis for queues and PostgreSQL for source-of-truth data.",
        );

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(
            chunks[0].embedding.len(),
            PROJECT_DOCUMENT_EMBEDDING_DIMENSIONS
        );
        assert!(chunks[0].token_count > 0);
    }
}
