use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing;
use walkdir::WalkDir;

/// Maximum number of skills to index (safety limit).
const MAX_SKILLS_TO_INDEX: usize = 5_000;

/// Default embedding model — small, fast, good quality.
const DEFAULT_MODEL: fastembed::EmbeddingModel = fastembed::EmbeddingModel::AllMiniLML6V2;

// ─── Public types ────────────────────────────────────────────────────────────

/// A matched skill from semantic search.
#[derive(Debug, Clone)]
pub struct SkillMatch {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub score: f32,
    pub source_path: PathBuf,
}

/// Parsed SKILL.md frontmatter.
#[derive(Debug, Deserialize, Default)]
struct SkillFrontmatter {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
}

/// Discovered skill on disk before embedding.
#[derive(Debug)]
struct DiscoveredSkill {
    skill_id: String,
    name: String,
    description: String,
    source_path: PathBuf,
}

// ─── KnowledgeIndex ──────────────────────────────────────────────────────────

/// RAG engine for semantic skill search.
///
/// Uses `fastembed` for local ONNX embeddings and `sqlite-vec` for vector KNN search.
/// The index is built once on startup and lives in memory (`:memory:` SQLite).
pub struct KnowledgeIndex {
    db: Connection,
    model: fastembed::TextEmbedding,
    embedding_dim: usize,
}

impl KnowledgeIndex {
    /// Build the knowledge index by scanning skill directories, embedding their
    /// frontmatter, and storing vectors in an in-memory SQLite database.
    pub fn build(skill_roots: Vec<PathBuf>) -> Result<Self> {
        tracing::info!(
            roots = ?skill_roots,
            "Building knowledge index from skill roots"
        );

        // 1. Initialize embedding model
        let model = fastembed::TextEmbedding::try_new(
            fastembed::InitOptions::new(DEFAULT_MODEL).with_show_download_progress(false),
        )
        .context("Failed to initialize fastembed embedding model")?;

        // Determine embedding dimension from a probe
        let probe = model
            .embed(vec!["probe"], None)
            .context("Failed to probe embedding dimension")?;
        let embedding_dim = probe.first().map(|v| v.len()).unwrap_or(384);

        // 2. Initialize in-memory SQLite with sqlite-vec
        let db = Connection::open_in_memory().context("Failed to open in-memory SQLite")?;

        // Load sqlite-vec extension
        unsafe {
            sqlite_vec::sqlite3_vec_init();
        }

        // Create metadata table
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS skills (
                skill_id   TEXT PRIMARY KEY,
                name       TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                source_path TEXT NOT NULL
            );",
        )
        .context("Failed to create skills table")?;

        // Create virtual vec0 table for vector search
        db.execute(
            &format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS skill_vectors USING vec0(
                    skill_id TEXT PRIMARY KEY,
                    embedding float[{embedding_dim}]
                );"
            ),
            [],
        )
        .context("Failed to create skill_vectors table")?;

        let mut index = Self {
            db,
            model,
            embedding_dim,
        };

        // 3. Discover and index skills
        let skills = discover_skills(&skill_roots);
        tracing::info!(count = skills.len(), "Discovered skills to index");

        if !skills.is_empty() {
            index.index_skills(&skills)?;
        }

        Ok(index)
    }

    /// Index a batch of discovered skills.
    fn index_skills(&mut self, skills: &[DiscoveredSkill]) -> Result<()> {
        // Build texts to embed: "name: description"
        let texts: Vec<String> = skills
            .iter()
            .map(|s| {
                if s.description.is_empty() {
                    s.name.clone()
                } else {
                    format!("{}: {}", s.name, s.description)
                }
            })
            .collect();

        // Batch embed
        let embeddings = self
            .model
            .embed(texts.clone(), None)
            .context("Failed to embed skill texts")?;

        // Insert into SQLite
        let tx = self.db.transaction()?;
        for (skill, embedding) in skills.iter().zip(embeddings.iter()) {
            // Insert metadata
            tx.execute(
                "INSERT OR REPLACE INTO skills (skill_id, name, description, source_path) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    skill.skill_id,
                    skill.name,
                    skill.description,
                    skill.source_path.to_string_lossy().to_string(),
                ],
            )?;

            // Insert vector (serialize as blob)
            let blob = vec_to_blob(embedding);
            tx.execute(
                "INSERT OR REPLACE INTO skill_vectors (skill_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![skill.skill_id, blob],
            )?;
        }
        tx.commit()?;

        tracing::info!(indexed = skills.len(), "Knowledge index built successfully");
        Ok(())
    }

    /// Semantic search: embed the query and find top-k similar skills.
    pub fn search(&self, query: &str, top_k: usize) -> Vec<SkillMatch> {
        let embeddings = match self.model.embed(vec![query.to_string()], None) {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!(%err, "Failed to embed search query");
                return Vec::new();
            }
        };

        let query_vec = match embeddings.first() {
            Some(v) => v,
            None => return Vec::new(),
        };

        let blob = vec_to_blob(query_vec);

        let mut stmt = match self.db.prepare(
            "SELECT sv.skill_id, sv.distance, s.name, s.description, s.source_path
             FROM skill_vectors sv
             JOIN skills s ON s.skill_id = sv.skill_id
             WHERE sv.embedding MATCH ?1
             ORDER BY sv.distance
             LIMIT ?2",
        ) {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(%err, "Failed to prepare search query");
                return Vec::new();
            }
        };

        let results = stmt
            .query_map(rusqlite::params![blob, top_k as i64], |row| {
                Ok(SkillMatch {
                    skill_id: row.get(0)?,
                    score: {
                        let distance: f64 = row.get(1)?;
                        // Convert distance to similarity (1 - distance for cosine)
                        (1.0 - distance as f32).max(0.0)
                    },
                    name: row.get(2)?,
                    description: row.get(3)?,
                    source_path: {
                        let p: String = row.get(4)?;
                        PathBuf::from(p)
                    },
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
            .unwrap_or_default();

        results
    }

    /// Read a specific skill's full SKILL.md content.
    pub fn read_skill(&self, skill_id: &str) -> Option<String> {
        let path: String = self
            .db
            .query_row(
                "SELECT source_path FROM skills WHERE skill_id = ?1",
                rusqlite::params![skill_id],
                |row| row.get(0),
            )
            .ok()?;

        std::fs::read_to_string(&path).ok()
    }

    /// Return the number of indexed skills.
    pub fn skill_count(&self) -> usize {
        self.db
            .query_row("SELECT COUNT(*) FROM skills", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap_or(0) as usize
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Walk skill root directories and discover all SKILL.md files.
fn discover_skills(roots: &[PathBuf]) -> Vec<DiscoveredSkill> {
    let mut skills = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for root in roots {
        if !root.is_dir() {
            continue;
        }

        for entry in WalkDir::new(root)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() != "SKILL.md" {
                continue;
            }

            let skill_dir = match entry.path().parent() {
                Some(d) => d,
                None => continue,
            };

            let skill_id = match skill_dir.file_name().and_then(|n| n.to_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };

            // Deduplicate
            if !seen_ids.insert(skill_id.clone()) {
                continue;
            }

            // Read and parse frontmatter
            let content = match std::fs::read_to_string(entry.path()) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let fm = parse_frontmatter(&content);

            let name = if fm.name.is_empty() {
                skill_id.clone()
            } else {
                fm.name
            };

            skills.push(DiscoveredSkill {
                skill_id,
                name,
                description: fm.description,
                source_path: entry.path().to_path_buf(),
            });

            if skills.len() >= MAX_SKILLS_TO_INDEX {
                tracing::warn!("Reached max skill indexing limit ({MAX_SKILLS_TO_INDEX})");
                return skills;
            }
        }
    }

    skills
}

/// Parse YAML frontmatter from a SKILL.md file.
/// Expects `---\nkey: value\n---\n` at the start of the file.
fn parse_frontmatter(content: &str) -> SkillFrontmatter {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return SkillFrontmatter::default();
    }

    let after_first = &trimmed[3..];
    let end = match after_first.find("---") {
        Some(pos) => pos,
        None => return SkillFrontmatter::default(),
    };

    let yaml_str = &after_first[..end].trim();
    serde_yaml::from_str(yaml_str).unwrap_or_default()
}

/// Convert f32 vector to raw bytes for sqlite-vec.
fn vec_to_blob(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_frontmatter_extracts_name_and_description() {
        let content = r#"---
name: test-skill
description: A test skill for unit tests
---

# Test Skill

Body content here.
"#;
        let fm = parse_frontmatter(content);
        assert_eq!(fm.name, "test-skill");
        assert_eq!(fm.description, "A test skill for unit tests");
    }

    #[test]
    fn parse_frontmatter_handles_missing() {
        let content = "# No frontmatter\nJust body.";
        let fm = parse_frontmatter(content);
        assert!(fm.name.is_empty());
        assert!(fm.description.is_empty());
    }

    #[test]
    fn discover_skills_finds_skill_files() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: hello\n---\n# Skill",
        )
        .unwrap();

        let skills = discover_skills(&[tmp.path().to_path_buf()]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].skill_id, "my-skill");
        assert_eq!(skills[0].name, "my-skill");
        assert_eq!(skills[0].description, "hello");
    }

    #[test]
    fn vec_to_blob_roundtrip() {
        let v = vec![1.0f32, 2.0, 3.0];
        let blob = vec_to_blob(&v);
        assert_eq!(blob.len(), 12); // 3 * 4 bytes
    }
}
