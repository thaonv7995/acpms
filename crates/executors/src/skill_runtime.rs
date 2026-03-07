use acpms_db::models::{ProjectSettings, ProjectType, Task};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::knowledge_index::{SkillKnowledgeHandle, SkillKnowledgeStatus};
use crate::task_skills::RuntimeLoadedSkill;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillPlanDecision {
    SelectedRequired,
    SelectedSuggested,
    SkippedDuplicate,
    SkippedLowConfidence,
    SkippedBudget,
    SkippedUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlannedSkill {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub source_path: Option<String>,
    pub origin: Option<String>,
    pub score: Option<f32>,
    pub phase: String,
    pub proposed_by: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkippedSkill {
    pub skill_id: String,
    pub source_path: Option<String>,
    pub origin: Option<String>,
    pub score: Option<f32>,
    pub phase: String,
    pub proposed_by: String,
    pub reason: String,
    pub decision: SkillPlanDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillSelectionTrace {
    pub skill_id: String,
    pub phase: String,
    pub proposed_by: String,
    pub decision: SkillPlanDecision,
    pub score: Option<f32>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillPlan {
    pub required: Vec<PlannedSkill>,
    pub suggested: Vec<PlannedSkill>,
    pub skipped: Vec<SkippedSkill>,
    pub trace: Vec<SkillSelectionTrace>,
    pub knowledge_status: SkillKnowledgeStatus,
    pub knowledge_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeSkillSearchMatch {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub score: f32,
    pub source_path: String,
    pub origin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeSkillSearchResult {
    pub status: SkillKnowledgeStatus,
    pub detail: Option<String>,
    pub matches: Vec<RuntimeSkillSearchMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeSkillLoadResult {
    pub status: SkillKnowledgeStatus,
    pub detail: Option<String>,
    pub skill: Option<RuntimeLoadedSkill>,
}

impl Default for SkillPlan {
    fn default() -> Self {
        Self {
            required: Vec::new(),
            suggested: Vec::new(),
            skipped: Vec::new(),
            trace: Vec::new(),
            knowledge_status: SkillKnowledgeStatus::Disabled,
            knowledge_detail: Some("Global skill knowledge is disabled.".to_string()),
        }
    }
}

impl SkillPlan {
    pub fn required_skill_ids(&self) -> Vec<String> {
        self.required
            .iter()
            .map(|skill| skill.skill_id.clone())
            .collect()
    }

    pub fn suggested_skill_ids(&self) -> Vec<String> {
        self.suggested
            .iter()
            .map(|skill| skill.skill_id.clone())
            .collect()
    }
}

#[derive(Clone, Default)]
pub struct SkillRuntime {
    knowledge_handle: Option<SkillKnowledgeHandle>,
}

impl SkillRuntime {
    pub fn new(knowledge_handle: Option<&SkillKnowledgeHandle>) -> Self {
        Self {
            knowledge_handle: knowledge_handle.cloned(),
        }
    }

    pub fn knowledge_handle(&self) -> Option<&SkillKnowledgeHandle> {
        self.knowledge_handle.as_ref()
    }

    pub fn plan_for_attempt(
        &self,
        task: &Task,
        settings: &ProjectSettings,
        project_type: ProjectType,
        repo_path: Option<&Path>,
    ) -> SkillPlan {
        crate::task_skills::build_skill_plan(
            task,
            settings,
            project_type,
            repo_path,
            self.knowledge_handle(),
        )
    }

    pub fn search_runtime(&self, query: &str, top_k: usize) -> RuntimeSkillSearchResult {
        let query = query.trim();
        if query.is_empty() {
            return RuntimeSkillSearchResult {
                status: SkillKnowledgeStatus::NoMatches,
                detail: Some("Skill search query was empty.".to_string()),
                matches: Vec::new(),
            };
        }

        let top_k = top_k.clamp(1, 8);
        match self.knowledge_handle().map(SkillKnowledgeHandle::snapshot) {
            None | Some(crate::knowledge_index::SkillKnowledgeSnapshot::Disabled) => {
                RuntimeSkillSearchResult {
                    status: SkillKnowledgeStatus::Disabled,
                    detail: Some("Global skill knowledge is disabled.".to_string()),
                    matches: Vec::new(),
                }
            }
            Some(crate::knowledge_index::SkillKnowledgeSnapshot::Pending) => {
                RuntimeSkillSearchResult {
                    status: SkillKnowledgeStatus::Pending,
                    detail: Some("Global skill knowledge index is still building.".to_string()),
                    matches: Vec::new(),
                }
            }
            Some(crate::knowledge_index::SkillKnowledgeSnapshot::Failed(detail)) => {
                RuntimeSkillSearchResult {
                    status: SkillKnowledgeStatus::Failed,
                    detail: Some(detail),
                    matches: Vec::new(),
                }
            }
            Some(crate::knowledge_index::SkillKnowledgeSnapshot::Ready(backend)) => {
                match backend.search(query, top_k) {
                    Ok(matches) => {
                        let mapped = matches
                            .into_iter()
                            .map(|skill| RuntimeSkillSearchMatch {
                                skill_id: skill.skill_id,
                                name: skill.name,
                                description: skill.description,
                                score: skill.score,
                                source_path: skill.source_path.to_string_lossy().to_string(),
                                origin: skill.origin,
                            })
                            .collect::<Vec<_>>();

                        let detail = if mapped.is_empty() {
                            Some(
                                "No matching skills found in the global knowledge index."
                                    .to_string(),
                            )
                        } else {
                            Some(format!(
                                "Found {} runtime skill candidate(s) in the global knowledge index.",
                                mapped.len()
                            ))
                        };

                        RuntimeSkillSearchResult {
                            status: if mapped.is_empty() {
                                SkillKnowledgeStatus::NoMatches
                            } else {
                                SkillKnowledgeStatus::Ready
                            },
                            detail,
                            matches: mapped,
                        }
                    }
                    Err(error) => RuntimeSkillSearchResult {
                        status: SkillKnowledgeStatus::Failed,
                        detail: Some(error.to_string()),
                        matches: Vec::new(),
                    },
                }
            }
        }
    }

    pub fn load_runtime(&self, skill_id: &str, repo_path: Option<&Path>) -> RuntimeSkillLoadResult {
        let skill_id = skill_id.trim();
        if skill_id.is_empty() {
            return RuntimeSkillLoadResult {
                status: SkillKnowledgeStatus::NoMatches,
                detail: Some("Skill id was empty.".to_string()),
                skill: None,
            };
        }

        if let Some(skill) = crate::task_skills::get_runtime_skill_attachment(skill_id, repo_path) {
            return RuntimeSkillLoadResult {
                status: SkillKnowledgeStatus::Ready,
                detail: Some(format!("Loaded runtime skill `{}`.", skill_id)),
                skill: Some(skill),
            };
        }

        match self.knowledge_handle().map(SkillKnowledgeHandle::snapshot) {
            None | Some(crate::knowledge_index::SkillKnowledgeSnapshot::Disabled) => {
                RuntimeSkillLoadResult {
                    status: SkillKnowledgeStatus::Disabled,
                    detail: Some("Global skill knowledge is disabled.".to_string()),
                    skill: None,
                }
            }
            Some(crate::knowledge_index::SkillKnowledgeSnapshot::Pending) => {
                RuntimeSkillLoadResult {
                    status: SkillKnowledgeStatus::Pending,
                    detail: Some("Global skill knowledge index is still building.".to_string()),
                    skill: None,
                }
            }
            Some(crate::knowledge_index::SkillKnowledgeSnapshot::Failed(detail)) => {
                RuntimeSkillLoadResult {
                    status: SkillKnowledgeStatus::Failed,
                    detail: Some(detail),
                    skill: None,
                }
            }
            Some(crate::knowledge_index::SkillKnowledgeSnapshot::Ready(backend)) => {
                match backend.read_skill(skill_id) {
                    Ok(Some(content)) => {
                        let metadata = backend.search(skill_id, 8).ok().and_then(|matches| {
                            matches
                                .into_iter()
                                .find(|candidate| candidate.skill_id == skill_id)
                        });
                        RuntimeSkillLoadResult {
                            status: SkillKnowledgeStatus::Ready,
                            detail: Some(format!("Loaded runtime skill `{}`.", skill_id)),
                            skill: Some(RuntimeLoadedSkill {
                                skill_id: skill_id.to_string(),
                                content,
                                source_path: metadata
                                    .as_ref()
                                    .map(|match_| match_.source_path.to_string_lossy().to_string()),
                                origin: metadata.map(|match_| match_.origin),
                            }),
                        }
                    }
                    Ok(None) => RuntimeSkillLoadResult {
                        status: SkillKnowledgeStatus::NoMatches,
                        detail: Some(format!("Skill `{}` was not found.", skill_id)),
                        skill: None,
                    },
                    Err(error) => RuntimeSkillLoadResult {
                        status: SkillKnowledgeStatus::Failed,
                        detail: Some(error.to_string()),
                        skill: None,
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_index::{SkillKnowledgeBackend, SkillKnowledgeHandle, SkillMatch};
    use anyhow::Result;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct FakeBackend {
        matches: Vec<SkillMatch>,
        contents: HashMap<String, String>,
    }

    impl SkillKnowledgeBackend for FakeBackend {
        fn search(&self, _query: &str, top_k: usize) -> Result<Vec<SkillMatch>> {
            Ok(self.matches.iter().take(top_k).cloned().collect())
        }

        fn read_skill(&self, skill_id: &str) -> Result<Option<String>> {
            Ok(self.contents.get(skill_id).cloned())
        }

        fn skill_count(&self) -> usize {
            self.matches.len()
        }
    }

    #[test]
    fn search_runtime_returns_ready_matches() {
        let handle = SkillKnowledgeHandle::pending();
        handle.set_ready_backend(Arc::new(FakeBackend {
            matches: vec![SkillMatch {
                skill_id: "openai-docs".to_string(),
                name: "OpenAI Docs".to_string(),
                description: "Use official docs".to_string(),
                score: 0.88,
                source_path: PathBuf::from("/tmp/openai-docs/SKILL.md"),
                origin: "community-openai".to_string(),
            }],
            contents: HashMap::new(),
        }));
        let runtime = SkillRuntime::new(Some(&handle));

        let result = runtime.search_runtime("openai docs", 5);

        assert_eq!(result.status, SkillKnowledgeStatus::Ready);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].skill_id, "openai-docs");
    }

    #[test]
    fn load_runtime_can_read_repo_local_skill_without_knowledge_index() {
        let temp_dir = tempfile::tempdir().unwrap();
        let skill_dir = temp_dir.path().join(".acpms/skills/runtime-probe");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: runtime-probe\ndescription: test\n---\n# Runtime Probe",
        )
        .unwrap();

        let runtime = SkillRuntime::new(None);
        let result = runtime.load_runtime("runtime-probe", Some(temp_dir.path()));

        assert_eq!(result.status, SkillKnowledgeStatus::Ready);
        let skill = result.skill.expect("runtime skill should load");
        assert_eq!(skill.skill_id, "runtime-probe");
        assert_eq!(skill.origin.as_deref(), Some("repo-local"));
        assert!(skill.content.contains("Runtime Probe"));
    }
}
