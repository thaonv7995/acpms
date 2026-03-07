use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use walkdir::WalkDir;

/// Maximum number of skills to index (safety limit).
const MAX_SKILLS_TO_INDEX: usize = 5_000;
const SEARCH_TEXT_BODY_LIMIT: usize = 4_000;

/// A global skill root that can be indexed for skill search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeRoot {
    pub path: PathBuf,
    pub origin: String,
}

/// A matched skill from search.
#[derive(Debug, Clone)]
pub struct SkillMatch {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub score: f32,
    pub source_path: PathBuf,
    pub origin: String,
}

/// High-level status for suggested knowledge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillKnowledgeStatus {
    Disabled,
    Pending,
    Ready,
    Failed,
    NoMatches,
}

/// Parsed SKILL.md frontmatter.
#[derive(Debug, Deserialize, Default)]
struct SkillFrontmatter {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
}

/// Discovered skill on disk before indexing.
#[derive(Debug)]
struct DiscoveredSkill {
    skill_id: String,
    name: String,
    description: String,
    source_path: PathBuf,
    origin: String,
    search_terms: HashSet<String>,
}

#[derive(Debug)]
struct IndexedSkill {
    skill_id: String,
    name: String,
    description: String,
    source_path: PathBuf,
    origin: String,
    normalized_skill_id: String,
    normalized_name: String,
    normalized_description: String,
    normalized_origin: String,
    skill_id_terms: HashSet<String>,
    name_terms: HashSet<String>,
    search_terms: HashSet<String>,
}

impl From<DiscoveredSkill> for IndexedSkill {
    fn from(value: DiscoveredSkill) -> Self {
        Self {
            normalized_skill_id: normalize_for_match(&value.skill_id),
            normalized_name: normalize_for_match(&value.name),
            normalized_description: normalize_for_match(&value.description),
            normalized_origin: normalize_for_match(&value.origin),
            skill_id_terms: tokenize(&value.skill_id),
            name_terms: tokenize(&value.name),
            skill_id: value.skill_id,
            name: value.name,
            description: value.description,
            source_path: value.source_path,
            origin: value.origin,
            search_terms: value.search_terms,
        }
    }
}

/// Trait abstraction for skill lookup backends.
pub trait SkillKnowledgeBackend: Send + Sync {
    fn search(&self, query: &str, top_k: usize) -> Result<Vec<SkillMatch>>;
    fn read_skill(&self, skill_id: &str) -> Result<Option<String>>;
    fn skill_count(&self) -> usize;
}

/// Backend backed by an in-memory [`KnowledgeIndex`].
pub struct IndexedKnowledgeBackend {
    index: KnowledgeIndex,
}

impl IndexedKnowledgeBackend {
    pub fn new(index: KnowledgeIndex) -> Self {
        Self { index }
    }
}

impl SkillKnowledgeBackend for IndexedKnowledgeBackend {
    fn search(&self, query: &str, top_k: usize) -> Result<Vec<SkillMatch>> {
        self.index.search(query, top_k)
    }

    fn read_skill(&self, skill_id: &str) -> Result<Option<String>> {
        self.index.read_skill(skill_id)
    }

    fn skill_count(&self) -> usize {
        self.index.skill_count()
    }
}

#[derive(Clone)]
pub enum SkillKnowledgeSnapshot {
    Disabled,
    Pending,
    Ready(Arc<dyn SkillKnowledgeBackend>),
    Failed(String),
}

enum SkillKnowledgeState {
    Disabled,
    Pending,
    Ready(Arc<dyn SkillKnowledgeBackend>),
    Failed(String),
}

/// Thread-safe handle for the global skill knowledge subsystem.
#[derive(Clone)]
pub struct SkillKnowledgeHandle {
    state: Arc<RwLock<SkillKnowledgeState>>,
}

impl SkillKnowledgeHandle {
    pub fn disabled() -> Self {
        Self {
            state: Arc::new(RwLock::new(SkillKnowledgeState::Disabled)),
        }
    }

    pub fn pending() -> Self {
        Self {
            state: Arc::new(RwLock::new(SkillKnowledgeState::Pending)),
        }
    }

    pub fn set_failed(&self, detail: impl Into<String>) {
        let mut state = self.state.write().expect("skill knowledge state poisoned");
        *state = SkillKnowledgeState::Failed(detail.into());
    }

    pub fn set_ready_index(&self, index: KnowledgeIndex) -> usize {
        self.set_ready_backend(Arc::new(IndexedKnowledgeBackend::new(index)))
    }

    pub fn set_ready_backend(&self, backend: Arc<dyn SkillKnowledgeBackend>) -> usize {
        let skill_count = backend.skill_count();
        let mut state = self.state.write().expect("skill knowledge state poisoned");
        *state = SkillKnowledgeState::Ready(backend);
        skill_count
    }

    pub fn snapshot(&self) -> SkillKnowledgeSnapshot {
        let state = self.state.read().expect("skill knowledge state poisoned");
        match &*state {
            SkillKnowledgeState::Disabled => SkillKnowledgeSnapshot::Disabled,
            SkillKnowledgeState::Pending => SkillKnowledgeSnapshot::Pending,
            SkillKnowledgeState::Ready(backend) => SkillKnowledgeSnapshot::Ready(backend.clone()),
            SkillKnowledgeState::Failed(detail) => SkillKnowledgeSnapshot::Failed(detail.clone()),
        }
    }

    pub fn status(&self) -> SkillKnowledgeStatus {
        match self.snapshot() {
            SkillKnowledgeSnapshot::Disabled => SkillKnowledgeStatus::Disabled,
            SkillKnowledgeSnapshot::Pending => SkillKnowledgeStatus::Pending,
            SkillKnowledgeSnapshot::Ready(_) => SkillKnowledgeStatus::Ready,
            SkillKnowledgeSnapshot::Failed(_) => SkillKnowledgeStatus::Failed,
        }
    }
}

/// Discover all global skill roots that should be part of the shared knowledge base.
pub fn discover_global_skill_roots() -> Vec<KnowledgeRoot> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |path: PathBuf, origin: &str| {
        if path.as_os_str().is_empty() || !seen.insert(path.clone()) {
            return;
        }
        roots.push(KnowledgeRoot {
            path,
            origin: origin.to_string(),
        });
    };

    if let Ok(dir) = std::env::var("ACPMS_SKILLS_DIR") {
        let skills_path = PathBuf::from(dir);
        push(skills_path.clone(), "platform");

        if let Ok(entries) = std::fs::read_dir(&skills_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = entry.file_name();
                let file_name = file_name.to_string_lossy();
                if path.is_dir() && file_name.starts_with("community-") {
                    push(path, &file_name);
                }
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        push(cwd.join(".acpms").join("skills"), "cwd");
    }

    if let Ok(codex_home) = std::env::var("CODEX_HOME") {
        push(PathBuf::from(codex_home).join("skills"), "codex-home");
    } else if let Some(home) = dirs::home_dir() {
        push(home.join(".codex").join("skills"), "codex-home");
    }

    roots
}

/// In-memory knowledge index for lexical skill search.
pub struct KnowledgeIndex {
    skills: Vec<IndexedSkill>,
    paths_by_skill_id: HashMap<String, PathBuf>,
}

impl KnowledgeIndex {
    /// Build the knowledge index by scanning skill directories and indexing
    /// frontmatter plus a trimmed body excerpt for lightweight matching.
    pub fn build(skill_roots: Vec<KnowledgeRoot>) -> Result<Self> {
        tracing::info!(
            roots = ?skill_roots,
            "Building knowledge index from skill roots"
        );

        let discovered = discover_skills(&skill_roots);
        tracing::info!(count = discovered.len(), "Discovered skills to index");

        let skills = discovered
            .into_iter()
            .map(IndexedSkill::from)
            .collect::<Vec<_>>();
        let paths_by_skill_id = skills
            .iter()
            .map(|skill| (skill.skill_id.clone(), skill.source_path.clone()))
            .collect::<HashMap<_, _>>();

        tracing::info!(indexed = skills.len(), "Knowledge index built successfully");
        Ok(Self {
            skills,
            paths_by_skill_id,
        })
    }

    /// Lexical search over skill ids, frontmatter, and a body excerpt.
    pub fn search(&self, query: &str, top_k: usize) -> Result<Vec<SkillMatch>> {
        let query_terms = tokenize(query);
        if query_terms.is_empty() || top_k == 0 {
            return Ok(Vec::new());
        }

        let normalized_query = normalize_for_match(query);
        let mut results = self
            .skills
            .iter()
            .filter_map(|skill| {
                lexical_score(skill, &normalized_query, &query_terms).map(|score| SkillMatch {
                    skill_id: skill.skill_id.clone(),
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    score,
                    source_path: skill.source_path.clone(),
                    origin: skill.origin.clone(),
                })
            })
            .collect::<Vec<_>>();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.skill_id.cmp(&b.skill_id))
        });
        results.truncate(top_k);
        Ok(results)
    }

    /// Read a specific skill's full SKILL.md content.
    pub fn read_skill(&self, skill_id: &str) -> Result<Option<String>> {
        let Some(path) = self.paths_by_skill_id.get(skill_id) else {
            return Ok(None);
        };

        let content = std::fs::read_to_string(path)?;
        Ok(Some(content))
    }

    pub fn skill_count(&self) -> usize {
        self.skills.len()
    }
}

fn discover_skills(roots: &[KnowledgeRoot]) -> Vec<DiscoveredSkill> {
    let mut skills = Vec::new();
    let mut seen_ids = HashSet::new();

    for root in roots {
        if !root.path.is_dir() {
            continue;
        }

        for entry in WalkDir::new(&root.path)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() != "SKILL.md" {
                continue;
            }

            let skill_dir = match entry.path().parent() {
                Some(dir) => dir,
                None => continue,
            };

            let skill_id = match skill_dir.file_name().and_then(|name| name.to_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };

            if !seen_ids.insert(skill_id.clone()) {
                continue;
            }

            let content = match std::fs::read_to_string(entry.path()) {
                Ok(content) => content,
                Err(_) => continue,
            };

            let fm = parse_frontmatter(&content);
            let name = if fm.name.is_empty() {
                skill_id.clone()
            } else {
                fm.name
            };
            let description = fm.description;

            skills.push(DiscoveredSkill {
                search_terms: tokenize(&build_search_text(
                    &skill_id,
                    &name,
                    &description,
                    &root.origin,
                    &content,
                )),
                skill_id,
                name,
                description,
                source_path: entry.path().to_path_buf(),
                origin: root.origin.clone(),
            });

            if skills.len() >= MAX_SKILLS_TO_INDEX {
                tracing::warn!("Reached max skill indexing limit ({MAX_SKILLS_TO_INDEX})");
                return skills;
            }
        }
    }

    skills
}

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

fn build_search_text(
    skill_id: &str,
    name: &str,
    description: &str,
    origin: &str,
    content: &str,
) -> String {
    let body = skill_body_excerpt(content);
    format!("{skill_id}\n{name}\n{description}\n{origin}\n{body}")
}

fn skill_body_excerpt(content: &str) -> String {
    let trimmed = content.trim_start();
    let body = if !trimmed.starts_with("---") {
        trimmed
    } else {
        let after_first = &trimmed[3..];
        match after_first.find("---") {
            Some(pos) => &after_first[(pos + 3)..],
            None => trimmed,
        }
    };

    body.chars().take(SEARCH_TEXT_BODY_LIMIT).collect()
}

fn tokenize(input: &str) -> HashSet<String> {
    input
        .split(|ch: char| !ch.is_alphanumeric())
        .map(normalize_for_match)
        .filter(|term| term.len() >= 2)
        .filter(|term| !is_stop_word(term))
        .collect()
}

fn normalize_for_match(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn is_stop_word(term: &str) -> bool {
    matches!(
        term,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "do"
            | "for"
            | "from"
            | "help"
            | "how"
            | "i"
            | "in"
            | "into"
            | "is"
            | "it"
            | "latest"
            | "me"
            | "need"
            | "of"
            | "on"
            | "or"
            | "our"
            | "please"
            | "research"
            | "show"
            | "that"
            | "the"
            | "this"
            | "to"
            | "up"
            | "use"
            | "using"
            | "want"
            | "we"
            | "with"
    )
}

fn lexical_score(
    skill: &IndexedSkill,
    normalized_query: &str,
    query_terms: &HashSet<String>,
) -> Option<f32> {
    let mut matched_terms = 0usize;
    let mut points = 0.0f32;

    for term in query_terms {
        let mut matched = false;

        if skill.normalized_skill_id == *term {
            points += 5.0;
            matched = true;
        } else if skill.normalized_skill_id.contains(term) {
            points += 3.0;
            matched = true;
        }

        if skill.normalized_name.contains(term) {
            points += 2.5;
            matched = true;
        }

        if skill.normalized_description.contains(term) {
            points += 2.0;
            matched = true;
        }

        if skill.normalized_origin.contains(term) {
            points += 0.5;
            matched = true;
        }

        if skill.search_terms.contains(term) {
            points += 1.5;
            matched = true;
        }

        if matched {
            matched_terms += 1;
        }
    }

    if matched_terms == 0 {
        return None;
    }

    if !skill.skill_id_terms.is_empty() && skill.skill_id_terms.is_subset(query_terms) {
        points += 4.0;
    }

    if !skill.name_terms.is_empty() && skill.name_terms.is_subset(query_terms) {
        points += 2.5;
    }

    if !normalized_query.is_empty()
        && (skill.normalized_name.contains(normalized_query)
            || skill.normalized_description.contains(normalized_query)
            || skill.normalized_skill_id.contains(normalized_query))
    {
        points += 2.0;
    }

    let coverage = matched_terms as f32 / query_terms.len() as f32;
    let density = (points / (query_terms.len() as f32 * 8.0)).min(1.0);
    Some((coverage * 0.65 + density * 0.35).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeKnowledgeBackend;

    impl SkillKnowledgeBackend for FakeKnowledgeBackend {
        fn search(&self, _query: &str, _top_k: usize) -> Result<Vec<SkillMatch>> {
            Ok(Vec::new())
        }

        fn read_skill(&self, _skill_id: &str) -> Result<Option<String>> {
            Ok(None)
        }

        fn skill_count(&self) -> usize {
            7
        }
    }

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
    fn discover_skills_finds_skill_files_and_origin() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: hello\n---\n# Skill",
        )
        .unwrap();

        let skills = discover_skills(&[KnowledgeRoot {
            path: tmp.path().to_path_buf(),
            origin: "platform".to_string(),
        }]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].skill_id, "my-skill");
        assert_eq!(skills[0].name, "my-skill");
        assert_eq!(skills[0].description, "hello");
        assert_eq!(skills[0].origin, "platform");
        assert!(skills[0].search_terms.contains("hello"));
    }

    #[test]
    fn knowledge_index_build_and_search_returns_lexical_match() {
        let tmp = tempfile::tempdir().unwrap();

        let openai_dir = tmp.path().join("openai-docs");
        std::fs::create_dir_all(&openai_dir).unwrap();
        std::fs::write(
            openai_dir.join("SKILL.md"),
            "---\nname: OpenAI Docs\ndescription: Use official OpenAI API documentation\n---\nResponses API, embeddings, models, limits",
        )
        .unwrap();

        let cloudflare_dir = tmp.path().join("cloudflare-deploy");
        std::fs::create_dir_all(&cloudflare_dir).unwrap();
        std::fs::write(
            cloudflare_dir.join("SKILL.md"),
            "---\nname: Cloudflare Deploy\ndescription: Deploy workers and pages\n---\nWorkers, Pages, deployments",
        )
        .unwrap();

        let index = KnowledgeIndex::build(vec![KnowledgeRoot {
            path: tmp.path().to_path_buf(),
            origin: "platform".to_string(),
        }])
        .unwrap();

        let matches = index
            .search("Need OpenAI API docs for embeddings", 3)
            .unwrap();

        assert!(!matches.is_empty());
        assert_eq!(matches[0].skill_id, "openai-docs");
        assert_eq!(matches[0].origin, "platform");
        assert!(matches[0].score > 0.0);
    }

    #[test]
    fn knowledge_index_prioritizes_openai_docs_for_docs_query_with_stop_words() {
        let tmp = tempfile::tempdir().unwrap();

        let openai_dir = tmp.path().join("openai-docs");
        std::fs::create_dir_all(&openai_dir).unwrap();
        std::fs::write(
            openai_dir.join("SKILL.md"),
            "---\nname: openai-docs\ndescription: Use official OpenAI documentation with citations for Responses API, Chat Completions API, Agents SDK, and model limits\n---\nOpenAI docs MCP, Responses API, Chat Completions API, Agents SDK, model limits, citations",
        )
        .unwrap();

        let apps_dir = tmp.path().join("chatgpt-apps");
        std::fs::create_dir_all(&apps_dir).unwrap();
        std::fs::write(
            apps_dir.join("SKILL.md"),
            "---\nname: chatgpt-apps\ndescription: Build ChatGPT Apps SDK applications\n---\nApps SDK, MCP server, widgets",
        )
        .unwrap();

        let index = KnowledgeIndex::build(vec![KnowledgeRoot {
            path: tmp.path().to_path_buf(),
            origin: "community-openai".to_string(),
        }])
        .unwrap();

        let matches = index
            .search(
                "Research OpenAI Responses API docs for tool calling. Need official OpenAI documentation, citations, Responses API, Chat Completions, Agents SDK, model limits.",
                5,
            )
            .unwrap();

        assert!(!matches.is_empty());
        assert_eq!(matches[0].skill_id, "openai-docs");
        assert!(matches[0].score >= 0.2);
    }

    #[test]
    fn skill_knowledge_handle_transitions_pending_to_ready() {
        let handle = SkillKnowledgeHandle::pending();
        assert_eq!(handle.status(), SkillKnowledgeStatus::Pending);

        let count = handle.set_ready_backend(Arc::new(FakeKnowledgeBackend));
        assert_eq!(count, 7);
        assert_eq!(handle.status(), SkillKnowledgeStatus::Ready);

        match handle.snapshot() {
            SkillKnowledgeSnapshot::Ready(backend) => assert_eq!(backend.skill_count(), 7),
            _ => panic!("expected ready snapshot"),
        }
    }

    #[test]
    fn skill_knowledge_handle_transitions_pending_to_failed() {
        let handle = SkillKnowledgeHandle::pending();
        handle.set_failed("embedding init failed");

        assert_eq!(handle.status(), SkillKnowledgeStatus::Failed);
        match handle.snapshot() {
            SkillKnowledgeSnapshot::Failed(detail) => {
                assert_eq!(detail, "embedding init failed");
            }
            _ => panic!("expected failed snapshot"),
        }
    }

    #[test]
    fn skill_knowledge_handle_can_be_disabled() {
        let handle = SkillKnowledgeHandle::disabled();
        assert_eq!(handle.status(), SkillKnowledgeStatus::Disabled);
        assert!(matches!(
            handle.snapshot(),
            SkillKnowledgeSnapshot::Disabled
        ));
    }
}
