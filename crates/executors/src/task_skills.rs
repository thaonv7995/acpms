use acpms_db::models::{ProjectSettings, ProjectType, Task, TaskType};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::knowledge_index::{SkillKnowledgeHandle, SkillKnowledgeSnapshot, SkillKnowledgeStatus};
use crate::skill_runtime::{
    PlannedSkill, SkillPlan, SkillPlanDecision, SkillRuntime, SkillSelectionTrace, SkippedSkill,
};

const MAX_RAG_SUGGESTIONS: usize = 3;
const MAX_RAG_SEARCH_CANDIDATES: usize = 64;
const MIN_RAG_SCORE: f32 = 0.2;
const REQUIRED_SKILL_BUDGET: usize = 16_000;
const SUGGESTED_SKILL_BUDGET: usize = 6_000;
const RUNTIME_SKILL_RESERVE: usize = 2_000;
const MAX_REQUIRED_SKILL_CHARS: usize = 900;
const MIN_REQUIRED_SKILL_CHARS: usize = 180;
const MAX_SUGGESTED_SKILL_CHARS: usize = 1_200;
const MIN_SUGGESTED_SKILL_CHARS: usize = 240;

#[derive(Debug, Clone)]
struct LoadedSkill {
    id: String,
    content: String,
    source: Option<PathBuf>,
    origin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuggestedSkill {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub score: f32,
    pub source_path: String,
    pub origin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeLoadedSkill {
    pub skill_id: String,
    pub content: String,
    pub source_path: Option<String>,
    pub origin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedSkillFile {
    pub skill_id: String,
    pub origin: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillInstructionContext {
    pub block: String,
    pub resolved_skill_chain: Vec<String>,
    pub loaded_active_skills: Vec<String>,
    pub suggested_skills: Vec<SuggestedSkill>,
    pub knowledge_status: SkillKnowledgeStatus,
    pub knowledge_detail: Option<String>,
    pub plan: SkillPlan,
    pub render_budget: SkillRenderBudget,
}

#[derive(Debug, Default)]
struct SuggestedKnowledgeRenderOutcome {
    added: usize,
    skipped_for_budget: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SkillSuggestionExtension {
    AgentTriage,
    TaskHints,
    RepoSignalHints,
}

impl SkillSuggestionExtension {
    fn label(self) -> &'static str {
        match self {
            Self::AgentTriage => "agent_triage",
            Self::TaskHints => "task_hints",
            Self::RepoSignalHints => "repo_signal_hints",
        }
    }
}

#[derive(Debug, Clone)]
struct SuggestionQuery {
    query: String,
    extension: SkillSuggestionExtension,
    reason: String,
}

#[derive(Debug, Clone)]
struct AggregatedSuggestedCandidate {
    candidate: crate::knowledge_index::SkillMatch,
    supporting_extensions: Vec<SkillSuggestionExtension>,
    reasons: Vec<String>,
}

#[derive(Debug, Clone)]
struct SkillRootCandidate {
    path: PathBuf,
    origin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillRenderBudget {
    pub required_budget: usize,
    pub suggested_budget: usize,
    pub runtime_reserve: usize,
    pub required_used: usize,
    pub suggested_used: usize,
}

impl Default for SkillRenderBudget {
    fn default() -> Self {
        Self {
            required_budget: REQUIRED_SKILL_BUDGET,
            suggested_budget: SUGGESTED_SKILL_BUDGET,
            runtime_reserve: RUNTIME_SKILL_RESERVE,
            required_used: 0,
            suggested_used: 0,
        }
    }
}

pub fn build_skill_instruction_block(
    task: &Task,
    settings: &ProjectSettings,
    project_type: ProjectType,
    repo_path: Option<&Path>,
) -> String {
    build_skill_instruction_context(task, settings, project_type, repo_path, None).block
}

pub fn build_skill_instruction_block_with_rag(
    task: &Task,
    settings: &ProjectSettings,
    project_type: ProjectType,
    repo_path: Option<&Path>,
    knowledge_handle: Option<&SkillKnowledgeHandle>,
) -> String {
    build_skill_instruction_context(task, settings, project_type, repo_path, knowledge_handle).block
}

pub fn build_skill_instruction_context(
    task: &Task,
    settings: &ProjectSettings,
    project_type: ProjectType,
    repo_path: Option<&Path>,
    knowledge_handle: Option<&SkillKnowledgeHandle>,
) -> SkillInstructionContext {
    let plan = SkillRuntime::new(knowledge_handle).plan_for_attempt(
        task,
        settings,
        project_type,
        repo_path,
    );
    render_skill_instruction_context(&plan, repo_path)
}

pub fn render_skill_instruction_context(
    plan: &SkillPlan,
    repo_path: Option<&Path>,
) -> SkillInstructionContext {
    let mut context = SkillInstructionContext {
        block: String::new(),
        resolved_skill_chain: plan.required_skill_ids(),
        loaded_active_skills: Vec::new(),
        suggested_skills: Vec::new(),
        knowledge_status: plan.knowledge_status.clone(),
        knowledge_detail: plan.knowledge_detail.clone(),
        plan: plan.clone(),
        render_budget: SkillRenderBudget::default(),
    };
    let mut loaded_ids: HashSet<String> = HashSet::new();

    if !plan.required.is_empty() {
        let active_header = r#"

## Active Skills (Required)
Follow these skill playbooks strictly for this attempt. If a skill cannot be executed, state why in the final report.
"#;
        context.block.push_str(active_header);
        context.render_budget.required_used = context
            .render_budget
            .required_used
            .saturating_add(active_header.len());

        for required_skill in &plan.required {
            if context.render_budget.required_used >= context.render_budget.required_budget {
                break;
            }

            let skill_id = required_skill.skill_id.as_str();
            let loaded = load_skill(skill_id, repo_path).unwrap_or_else(|| LoadedSkill {
                id: skill_id.to_string(),
                content: builtin_skill_content(skill_id)
                    .unwrap_or(
                        "No external skill file found. Use standard best-practice execution for this capability.",
                    )
                    .to_string(),
                source: None,
                origin: Some("builtin".to_string()),
            });

            let available_budget = context
                .render_budget
                .required_budget
                .saturating_sub(context.render_budget.required_used);
            let Some((rendered, rendered_len)) =
                render_required_skill_block(required_skill, &loaded, available_budget)
            else {
                break;
            };

            context.render_budget.required_used = context
                .render_budget
                .required_used
                .saturating_add(rendered_len);
            context.block.push_str(&rendered);
            context.loaded_active_skills.push(loaded.id.clone());
            loaded_ids.insert(loaded.id);
        }
    }

    if context.render_budget.required_used >= context.render_budget.required_budget {
        if !plan.suggested.is_empty() {
            context.knowledge_status = SkillKnowledgeStatus::NoMatches;
            context.knowledge_detail = Some(
                "Required skill lane exhausted the prompt budget reserved for active skills."
                    .to_string(),
            );
        }
    }

    let render_outcome = append_suggested_knowledge(
        &plan.suggested,
        &mut context.block,
        &mut context.suggested_skills,
        &mut context.render_budget.suggested_used,
        &mut loaded_ids,
    );

    if render_outcome.added > 0 {
        context.knowledge_status = SkillKnowledgeStatus::Ready;
        context.knowledge_detail = Some(format!(
            "Added {} suggested skill(s) from the global knowledge index.",
            context.suggested_skills.len()
        ));
    } else if !plan.suggested.is_empty() && render_outcome.skipped_for_budget > 0 {
        context.knowledge_status = SkillKnowledgeStatus::NoMatches;
        context.knowledge_detail = Some(
            "Matching skills were found in the global knowledge index, but the remaining prompt budget was too small to attach them."
                .to_string(),
        );
    }

    context.block.push_str(runtime_skill_extension_block());

    context
}

pub fn build_skill_plan(
    task: &Task,
    settings: &ProjectSettings,
    project_type: ProjectType,
    repo_path: Option<&Path>,
    knowledge_handle: Option<&SkillKnowledgeHandle>,
) -> SkillPlan {
    let required_skill_ids = resolve_skill_chain(task, settings, project_type);
    let mut plan = SkillPlan {
        required: required_skill_ids
            .into_iter()
            .map(planned_required_skill)
            .collect(),
        ..SkillPlan::default()
    };

    for required_skill in &plan.required {
        plan.trace.push(SkillSelectionTrace {
            skill_id: required_skill.skill_id.clone(),
            phase: required_skill.phase.clone(),
            proposed_by: required_skill.proposed_by.clone(),
            decision: SkillPlanDecision::SelectedRequired,
            score: required_skill.score,
            reason: required_skill.reason.clone(),
        });
    }

    let mut selected_ids = plan
        .required
        .iter()
        .map(|skill| skill.skill_id.clone())
        .collect::<HashSet<_>>();

    match knowledge_handle.map(SkillKnowledgeHandle::snapshot) {
        None | Some(SkillKnowledgeSnapshot::Disabled) => {
            plan.knowledge_status = SkillKnowledgeStatus::Disabled;
            plan.knowledge_detail = Some("Global skill knowledge is disabled.".to_string());
        }
        Some(SkillKnowledgeSnapshot::Pending) => {
            plan.knowledge_status = SkillKnowledgeStatus::Pending;
            plan.knowledge_detail =
                Some("Global skill knowledge index is still building.".to_string());
        }
        Some(SkillKnowledgeSnapshot::Failed(detail)) => {
            plan.knowledge_status = SkillKnowledgeStatus::Failed;
            plan.knowledge_detail = Some(detail);
        }
        Some(SkillKnowledgeSnapshot::Ready(backend)) => {
            let search_limit = backend
                .skill_count()
                .min((MAX_RAG_SUGGESTIONS + selected_ids.len()).max(MAX_RAG_SEARCH_CANDIDATES));
            match collect_suggested_skill_matches(task, repo_path, backend.as_ref(), search_limit) {
                Ok(matches) => {
                    let mut skipped_any = false;

                    for candidate in matches {
                        if plan.suggested.len() >= MAX_RAG_SUGGESTIONS {
                            break;
                        }

                        if !suggested_skill_allowed_for_task(task, &candidate.candidate.skill_id) {
                            skipped_any = true;
                            plan.skipped.push(skipped_skill_from_match(
                                &candidate,
                                SkillPlanDecision::SkippedUnavailable,
                                "Candidate skill family is not relevant to this task type or intent.",
                            ));
                            plan.trace.push(SkillSelectionTrace {
                                skill_id: candidate.candidate.skill_id.clone(),
                                phase: "suggestion_pipeline".to_string(),
                                proposed_by: candidate
                                    .supporting_extensions
                                    .iter()
                                    .map(|extension| extension.label())
                                    .collect::<Vec<_>>()
                                    .join("+"),
                                decision: SkillPlanDecision::SkippedUnavailable,
                                score: Some(candidate.candidate.score),
                                reason: "Candidate skill family is not relevant to this task type or intent."
                                    .to_string(),
                            });
                            continue;
                        }

                        if selected_ids.contains(&candidate.candidate.skill_id) {
                            skipped_any = true;
                            plan.skipped.push(skipped_skill_from_match(
                                &candidate,
                                SkillPlanDecision::SkippedDuplicate,
                                "Already selected by the deterministic skill chain.",
                            ));
                            plan.trace.push(SkillSelectionTrace {
                                skill_id: candidate.candidate.skill_id.clone(),
                                phase: "suggestion_pipeline".to_string(),
                                proposed_by: candidate
                                    .supporting_extensions
                                    .iter()
                                    .map(|extension| extension.label())
                                    .collect::<Vec<_>>()
                                    .join("+"),
                                decision: SkillPlanDecision::SkippedDuplicate,
                                score: Some(candidate.candidate.score),
                                reason: "Already selected by the deterministic skill chain."
                                    .to_string(),
                            });
                            continue;
                        }

                        if candidate.candidate.score < MIN_RAG_SCORE {
                            skipped_any = true;
                            plan.skipped.push(skipped_skill_from_match(
                                &candidate,
                                SkillPlanDecision::SkippedLowConfidence,
                                "Candidate score was below the minimum confidence threshold.",
                            ));
                            plan.trace.push(SkillSelectionTrace {
                                skill_id: candidate.candidate.skill_id.clone(),
                                phase: "suggestion_pipeline".to_string(),
                                proposed_by: candidate
                                    .supporting_extensions
                                    .iter()
                                    .map(|extension| extension.label())
                                    .collect::<Vec<_>>()
                                    .join("+"),
                                decision: SkillPlanDecision::SkippedLowConfidence,
                                score: Some(candidate.candidate.score),
                                reason:
                                    "Candidate score was below the minimum confidence threshold."
                                        .to_string(),
                            });
                            continue;
                        }

                        selected_ids.insert(candidate.candidate.skill_id.clone());
                        let planned = planned_suggested_skill_from_match(candidate);
                        plan.trace.push(SkillSelectionTrace {
                            skill_id: planned.skill_id.clone(),
                            phase: planned.phase.clone(),
                            proposed_by: planned.proposed_by.clone(),
                            decision: SkillPlanDecision::SelectedSuggested,
                            score: planned.score,
                            reason: planned.reason.clone(),
                        });
                        plan.suggested.push(planned);
                    }

                    if plan.suggested.is_empty() {
                        plan.knowledge_status = SkillKnowledgeStatus::NoMatches;
                        plan.knowledge_detail = Some(if skipped_any {
                            "Knowledge candidates were found, but none survived duplicate or confidence filtering."
                                .to_string()
                        } else {
                            "No matching skills found in the global knowledge index.".to_string()
                        });
                    } else {
                        plan.knowledge_status = SkillKnowledgeStatus::Ready;
                        plan.knowledge_detail = Some(format!(
                            "Planned {} suggested skill(s) from the global knowledge index.",
                            plan.suggested.len()
                        ));
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "Failed to build suggested skill plan");
                    plan.knowledge_status = SkillKnowledgeStatus::Failed;
                    plan.knowledge_detail = Some(error.to_string());
                }
            }
        }
    }

    plan
}

pub fn build_skill_metadata_patch(
    context: &SkillInstructionContext,
    source: &str,
) -> serde_json::Value {
    serde_json::json!({
        "resolved_skill_chain": context.resolved_skill_chain,
        "resolved_skill_chain_source": source,
        "skill_plan": context.plan,
        "skill_render_budget": context.render_budget,
        "knowledge_suggestions": {
            "status": context.knowledge_status,
            "detail": context.knowledge_detail,
            "items": context.suggested_skills,
        }
    })
}

pub fn format_loaded_skills_log_line(context: &SkillInstructionContext) -> String {
    let mut parts = vec![format!(
        "Loaded skills: {}",
        summarize_skill_ids_for_log(&context.loaded_active_skills, 6)
    )];

    match context.knowledge_status {
        SkillKnowledgeStatus::Ready => {
            if context.suggested_skills.is_empty() {
                parts.push("suggested: none".to_string());
            } else {
                let suggested_ids = context
                    .suggested_skills
                    .iter()
                    .map(|skill| skill.skill_id.clone())
                    .collect::<Vec<_>>();
                parts.push(format!(
                    "suggested: {}",
                    summarize_skill_ids_for_log(&suggested_ids, MAX_RAG_SUGGESTIONS)
                ));
            }
        }
        SkillKnowledgeStatus::NoMatches => {
            parts.push("suggested: none".to_string());
        }
        SkillKnowledgeStatus::Disabled => {
            parts.push("knowledge: disabled".to_string());
        }
        SkillKnowledgeStatus::Pending => {
            parts.push("knowledge: pending".to_string());
        }
        SkillKnowledgeStatus::Failed => {
            parts.push("knowledge: failed".to_string());
        }
    }

    parts.join("; ")
}

fn build_rag_query(task: &Task) -> String {
    let mut query = task.title.clone();
    if let Some(desc) = &task.description {
        query.push(' ');
        query.push_str(desc);
    }
    // Truncate overly long queries
    if query.len() > 500 {
        query.truncate(500);
    }
    query
}

pub fn resolve_skill_chain(
    task: &Task,
    settings: &ProjectSettings,
    project_type: ProjectType,
) -> Vec<String> {
    derive_skill_chain(task, settings, project_type)
}

/// Load content for a single skill (from file or builtin). Used for custom flows like import analysis.
pub fn get_skill_content(skill_id: &str, repo_path: Option<&Path>) -> String {
    load_skill(skill_id, repo_path)
        .map(|s| s.content)
        .or_else(|| builtin_skill_content(skill_id).map(str::to_string))
        .unwrap_or_else(|| format!("Execute: {}", skill_id))
}

pub fn get_runtime_skill_attachment(
    skill_id: &str,
    repo_path: Option<&Path>,
) -> Option<RuntimeLoadedSkill> {
    load_skill(skill_id, repo_path)
        .map(|skill| RuntimeLoadedSkill {
            skill_id: skill.id,
            content: skill.content,
            source_path: skill.source.map(|path| path.to_string_lossy().to_string()),
            origin: skill.origin,
        })
        .or_else(|| {
            builtin_skill_content(skill_id).map(|content| RuntimeLoadedSkill {
                skill_id: skill_id.to_string(),
                content: content.to_string(),
                source_path: None,
                origin: Some("builtin".to_string()),
            })
        })
}

fn planned_required_skill(skill_id: String) -> PlannedSkill {
    PlannedSkill {
        skill_id: skill_id.clone(),
        name: skill_id.clone(),
        description: "Required skill selected by deterministic task policy.".to_string(),
        source_path: None,
        origin: Some("deterministic".to_string()),
        score: None,
        phase: "deterministic_chain".to_string(),
        proposed_by: "deterministic_chain".to_string(),
        reason: "Selected by deterministic task policy for this attempt.".to_string(),
    }
}

fn planned_suggested_skill_from_match(candidate: AggregatedSuggestedCandidate) -> PlannedSkill {
    let proposed_by = candidate
        .supporting_extensions
        .iter()
        .map(|extension| extension.label())
        .collect::<Vec<_>>()
        .join("+");
    let reason = format!(
        "Matched the suggestion pipeline via {}.",
        candidate.reasons.join("; ")
    );
    PlannedSkill {
        skill_id: candidate.candidate.skill_id,
        name: candidate.candidate.name,
        description: candidate.candidate.description,
        source_path: Some(
            candidate
                .candidate
                .source_path
                .to_string_lossy()
                .to_string(),
        ),
        origin: Some(candidate.candidate.origin),
        score: Some(candidate.candidate.score),
        phase: "suggestion_pipeline".to_string(),
        proposed_by,
        reason,
    }
}

fn skipped_skill_from_match(
    candidate: &AggregatedSuggestedCandidate,
    decision: SkillPlanDecision,
    reason: &str,
) -> SkippedSkill {
    SkippedSkill {
        skill_id: candidate.candidate.skill_id.clone(),
        source_path: Some(
            candidate
                .candidate
                .source_path
                .to_string_lossy()
                .to_string(),
        ),
        origin: Some(candidate.candidate.origin.clone()),
        score: Some(candidate.candidate.score),
        phase: "suggestion_pipeline".to_string(),
        proposed_by: candidate
            .supporting_extensions
            .iter()
            .map(|extension| extension.label())
            .collect::<Vec<_>>()
            .join("+"),
        reason: reason.to_string(),
        decision,
    }
}

fn append_suggested_knowledge(
    planned_suggestions: &[PlannedSkill],
    block: &mut String,
    suggested_skills: &mut Vec<SuggestedSkill>,
    suggested_chars_used: &mut usize,
    loaded_ids: &mut HashSet<String>,
) -> SuggestedKnowledgeRenderOutcome {
    let mut outcome = SuggestedKnowledgeRenderOutcome::default();

    for planned in planned_suggestions {
        if outcome.added >= MAX_RAG_SUGGESTIONS || *suggested_chars_used >= SUGGESTED_SKILL_BUDGET {
            break;
        }
        if loaded_ids.contains(&planned.skill_id) {
            continue;
        }

        let available_budget = SUGGESTED_SKILL_BUDGET.saturating_sub(*suggested_chars_used);
        let Some((rendered, rendered_len)) =
            render_suggested_skill_block(planned, outcome.added == 0, available_budget)
        else {
            outcome.skipped_for_budget += 1;
            continue;
        };

        *suggested_chars_used = (*suggested_chars_used).saturating_add(rendered_len);
        block.push_str(&rendered);

        suggested_skills.push(SuggestedSkill {
            skill_id: planned.skill_id.clone(),
            name: planned.name.clone(),
            description: planned.description.clone(),
            score: planned.score.unwrap_or_default(),
            source_path: planned.source_path.clone().unwrap_or_default(),
            origin: planned
                .origin
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        });

        loaded_ids.insert(planned.skill_id.clone());
        outcome.added += 1;
    }

    outcome
}

fn render_required_skill_block(
    planned: &PlannedSkill,
    loaded: &LoadedSkill,
    available_budget: usize,
) -> Option<(String, usize)> {
    let source = loaded
        .source
        .as_ref()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "builtin".to_string());
    let prefix = format!(
        "\n### Skill: {}\nSource: `{}`\n```text\n",
        loaded.id, source
    );
    let suffix = "\n```\n";
    let fixed_len = prefix.len().saturating_add(suffix.len());
    if available_budget <= fixed_len {
        return None;
    }

    let available_for_content = available_budget.saturating_sub(fixed_len);
    if available_for_content < MIN_REQUIRED_SKILL_CHARS {
        return None;
    }

    let content = compact_required_skill_content(
        planned,
        &loaded.content,
        available_for_content.min(MAX_REQUIRED_SKILL_CHARS),
    );
    let rendered = format!("{prefix}{}{suffix}", content.trim());
    Some((rendered.clone(), rendered.len()))
}

fn render_suggested_skill_block(
    planned: &PlannedSkill,
    include_section_header: bool,
    available_budget: usize,
) -> Option<(String, usize)> {
    let section_header = if include_section_header {
        "\n\n## Suggested Knowledge\nThe following skills were found in the global knowledge index. Use them as reference where relevant. If you need the full playbook, use the runtime skill extension with `load_skill`.\n"
    } else {
        ""
    };
    if available_budget < MIN_SUGGESTED_SKILL_CHARS {
        return None;
    }

    let fixed_prefix = format!(
        "{section_header}\n### Suggested Skill: {}\nSkill ID: `{}`\nOrigin: `{}`\nRelevance: `{:.0}%`\n",
        planned.name,
        planned.skill_id,
        planned.origin.as_deref().unwrap_or("unknown"),
        planned.score.unwrap_or_default() * 100.0,
    );
    let fixed_suffix = "\nUse `load_skill` if you need the full playbook during execution.\n";
    let fixed_len = fixed_prefix.len().saturating_add(fixed_suffix.len());
    if available_budget <= fixed_len {
        return None;
    }

    let content = compact_suggested_skill_content(
        planned,
        available_budget
            .saturating_sub(fixed_len)
            .min(MAX_SUGGESTED_SKILL_CHARS),
    );
    let rendered = format!("{fixed_prefix}{}{fixed_suffix}", content.trim());
    Some((rendered.clone(), rendered.len()))
}

fn compact_suggested_skill_content(planned: &PlannedSkill, max_content_len: usize) -> String {
    let mut skill_summary = format!(
        "Why suggested: {}\nSummary: {}",
        planned.reason.trim(),
        planned.description.trim(),
    );

    if skill_summary.len() <= max_content_len {
        return skill_summary;
    }

    let trailer = "\n... (suggested skill summary truncated)";
    let take_len = max_content_len.saturating_sub(trailer.len());
    skill_summary.truncate(skill_summary.len().min(take_len));
    let mut truncated = skill_summary.chars().take(take_len).collect::<String>();
    truncated.push_str(trailer);
    truncated
}

fn compact_required_skill_content(
    planned: &PlannedSkill,
    raw_content: &str,
    max_content_len: usize,
) -> String {
    let body = strip_skill_frontmatter(raw_content).trim();
    let header = format!(
        "Skill ID: {}\nSelection: {}\n\n",
        planned.skill_id, planned.reason
    );
    let skill_summary = format!("{header}{body}");

    if skill_summary.len() <= max_content_len {
        return skill_summary;
    }

    let trailer = "\n... (required skill excerpt truncated)";
    let take_len = max_content_len.saturating_sub(trailer.len());
    let mut truncated = skill_summary.chars().take(take_len).collect::<String>();
    truncated.push_str(trailer);
    truncated
}

fn strip_skill_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return trimmed;
    }

    let after_first = &trimmed[3..];
    match after_first.find("---") {
        Some(pos) => &after_first[(pos + 3)..],
        None => trimmed,
    }
}

fn collect_suggested_skill_matches(
    task: &Task,
    repo_path: Option<&Path>,
    backend: &dyn crate::knowledge_index::SkillKnowledgeBackend,
    search_limit: usize,
) -> anyhow::Result<Vec<AggregatedSuggestedCandidate>> {
    #[derive(Debug)]
    struct AggregatedCandidate {
        candidate: crate::knowledge_index::SkillMatch,
        best_score: f32,
        hit_count: usize,
        extensions: HashSet<SkillSuggestionExtension>,
        reasons: Vec<String>,
    }

    let queries = build_suggestion_queries(task, repo_path);
    let mut by_skill_id: HashMap<String, AggregatedCandidate> = HashMap::new();

    for query in queries {
        for candidate in backend.search(&query.query, search_limit)? {
            let entry = by_skill_id
                .entry(candidate.skill_id.clone())
                .or_insert_with(|| AggregatedCandidate {
                    candidate: candidate.clone(),
                    best_score: candidate.score,
                    hit_count: 0,
                    extensions: HashSet::new(),
                    reasons: Vec::new(),
                });
            entry.best_score = entry.best_score.max(candidate.score);
            entry.hit_count += 1;
            entry.extensions.insert(query.extension);
            if !entry.reasons.iter().any(|reason| reason == &query.reason) {
                entry.reasons.push(query.reason.clone());
            }
            entry.candidate = candidate;
        }
    }

    let mut matches = by_skill_id
        .into_values()
        .map(|mut entry| {
            let multi_query_bonus = (entry.hit_count.saturating_sub(1) as f32 * 0.06).min(0.18);
            let extension_bonus =
                (entry.extensions.len().saturating_sub(1) as f32 * 0.05).min(0.15);
            entry.candidate.score =
                (entry.best_score + multi_query_bonus + extension_bonus).min(1.0);
            let mut supporting_extensions = entry.extensions.into_iter().collect::<Vec<_>>();
            supporting_extensions.sort_by_key(|extension| extension.label());
            AggregatedSuggestedCandidate {
                candidate: entry.candidate,
                supporting_extensions,
                reasons: entry.reasons,
            }
        })
        .collect::<Vec<_>>();

    matches.sort_by(|a, b| {
        b.candidate
            .score
            .partial_cmp(&a.candidate.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.candidate.skill_id.cmp(&b.candidate.skill_id))
    });
    matches.truncate(search_limit);
    Ok(matches)
}

#[cfg(test)]
fn build_skill_search_queries(task: &Task, repo_path: Option<&Path>) -> Vec<String> {
    build_suggestion_queries(task, repo_path)
        .into_iter()
        .map(|query| query.query)
        .collect()
}

fn build_suggestion_queries(task: &Task, repo_path: Option<&Path>) -> Vec<SuggestionQuery> {
    let task_text = task_text(task);
    let task_terms = extract_query_terms(&task_text);
    let repo_terms = collect_repo_signal_terms(repo_path);

    let mut queries = Vec::new();
    push_suggestion_query(
        &mut queries,
        SkillSuggestionExtension::AgentTriage,
        build_rag_query(task),
        "Agent triage over the raw task title and description.",
    );

    if !task_terms.is_empty() {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::TaskHints,
            task_terms.join(" "),
            "Task keyword hints extracted from normalized task text.",
        );
    }

    if should_hint_openai_docs(&task_terms, &HashSet::new()) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::TaskHints,
            "openai-docs openai docs official documentation citations responses api chat completions agents sdk realtime codex model limits",
            "Task text suggests OpenAI API or documentation work.",
        );
    }

    if should_hint_openai_docs(&[], &repo_terms) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::RepoSignalHints,
            "openai-docs openai docs official documentation citations responses api chat completions agents sdk realtime codex model limits",
            "Repository signals suggest OpenAI SDK or docs-related work.",
        );
    }

    if should_hint_playwright(&task_terms, &HashSet::new()) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::TaskHints,
            "playwright browser automation e2e screenshot snapshot ui flow debugging",
            "Task text suggests browser automation or UI debugging work.",
        );
    }

    if should_hint_playwright(&[], &repo_terms) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::RepoSignalHints,
            "playwright browser automation e2e screenshot snapshot ui flow debugging",
            "Repository signals suggest Playwright or browser automation work.",
        );
    }

    if should_hint_figma(&task_terms, &HashSet::new()) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::TaskHints,
            "figma design context node id screenshot variables design-to-code assets",
            "Task text suggests Figma design implementation work.",
        );
    }

    if should_hint_figma(&[], &repo_terms) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::RepoSignalHints,
            "figma design context node id screenshot variables design-to-code assets",
            "Repository signals suggest Figma-related work.",
        );
    }

    if should_hint_docx(&task_terms, &HashSet::new()) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::TaskHints,
            "doc docx word document python-docx formatting template layout tables",
            "Task text suggests DOCX or document formatting work.",
        );
    }

    if should_hint_docx(&[], &repo_terms) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::RepoSignalHints,
            "doc docx word document python-docx formatting template layout tables",
            "Repository signals suggest DOCX or document formatting work.",
        );
    }

    if should_hint_cloudflare(&task_terms, &HashSet::new()) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::TaskHints,
            "cloudflare deploy workers pages tunnel wrangler dns preview",
            "Task text suggests Cloudflare deployment or tunnel work.",
        );
    }

    if should_hint_cloudflare(&[], &repo_terms) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::RepoSignalHints,
            "cloudflare deploy workers pages tunnel wrangler dns preview",
            "Repository signals suggest Cloudflare deployment work.",
        );
    }

    if should_hint_imagegen(&task_terms, &HashSet::new()) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::TaskHints,
            "imagegen image generation edit mask transparent background product shots",
            "Task text suggests image generation or editing work.",
        );
    }

    if should_hint_imagegen(&[], &repo_terms) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::RepoSignalHints,
            "imagegen image generation edit mask transparent background product shots",
            "Repository signals suggest image generation or editing work.",
        );
    }

    if should_hint_sora(&task_terms, &HashSet::new()) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::TaskHints,
            "sora video generation remix storyboard thumbnail spritesheet",
            "Task text suggests Sora video work.",
        );
    }

    if should_hint_sora(&[], &repo_terms) {
        push_suggestion_query(
            &mut queries,
            SkillSuggestionExtension::RepoSignalHints,
            "sora video generation remix storyboard thumbnail spritesheet",
            "Repository signals suggest Sora video work.",
        );
    }

    queries
}

fn task_text(task: &Task) -> String {
    match task.description.as_deref() {
        Some(description) if !description.trim().is_empty() => {
            format!("{} {}", task.title, description)
        }
        _ => task.title.clone(),
    }
}

fn push_suggestion_query(
    queries: &mut Vec<SuggestionQuery>,
    extension: SkillSuggestionExtension,
    query: impl Into<String>,
    reason: impl Into<String>,
) {
    let query = query.into();
    let normalized = query.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty()
        || queries
            .iter()
            .any(|existing| existing.extension == extension && existing.query == normalized)
    {
        return;
    }
    queries.push(SuggestionQuery {
        query: normalized,
        extension,
        reason: reason.into(),
    });
}

fn extract_query_terms(input: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut seen = HashSet::new();

    for raw in input.split(|ch: char| !ch.is_alphanumeric()) {
        let term = raw.trim().to_ascii_lowercase();
        if term.len() < 2 || is_query_stop_word(&term) || !seen.insert(term.clone()) {
            continue;
        }
        terms.push(term);
        if terms.len() >= 24 {
            break;
        }
    }

    terms
}

fn is_query_stop_word(term: &str) -> bool {
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

fn collect_repo_signal_terms(repo_path: Option<&Path>) -> HashSet<String> {
    let Some(repo_path) = repo_path else {
        return HashSet::new();
    };
    if !repo_path.is_dir() {
        return HashSet::new();
    }

    let mut terms = HashSet::new();
    let walker = walkdir::WalkDir::new(repo_path)
        .max_depth(3)
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            !name.starts_with('.')
                && name != "node_modules"
                && name != "target"
                && name != "vendor"
                && name != "dist"
                && name != "build"
        });

    for entry in walker.flatten().take(200) {
        let path = entry.path();
        if entry.file_type().is_file() {
            if let Ok(rel_path) = path.strip_prefix(repo_path) {
                for term in extract_query_terms(&rel_path.to_string_lossy()) {
                    terms.insert(term);
                }
            }

            if should_read_repo_signal_file(path) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    for term in
                        extract_query_terms(&content.chars().take(20_000).collect::<String>())
                    {
                        terms.insert(term);
                    }
                }
            }
        }
    }

    terms
}

fn should_read_repo_signal_file(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|value| value.to_str()),
        Some(
            "package.json"
                | "pyproject.toml"
                | "requirements.txt"
                | "go.mod"
                | "Cargo.toml"
                | "wrangler.toml"
                | "pubspec.yaml"
                | "Gemfile"
                | "Podfile"
        )
    )
}

fn should_hint_openai_docs(task_terms: &[String], repo_terms: &HashSet<String>) -> bool {
    let task_set = task_terms
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mentions_openai = contains_any(
        &task_set,
        &[
            "openai",
            "responses",
            "completions",
            "agents",
            "realtime",
            "codex",
            "gpt",
            "model",
        ],
    );
    let asks_for_docs = contains_any(
        &task_set,
        &[
            "docs",
            "documentation",
            "citation",
            "citations",
            "sdk",
            "api",
            "apis",
            "tool",
        ],
    );

    (mentions_openai && asks_for_docs)
        || repo_terms.contains("openai")
        || repo_terms.contains("responses")
        || repo_terms.contains("chatgpt")
}

fn should_hint_playwright(task_terms: &[String], repo_terms: &HashSet<String>) -> bool {
    let task_set = task_terms
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    contains_any(
        &task_set,
        &[
            "playwright",
            "browser",
            "automation",
            "e2e",
            "snapshot",
            "screenshot",
            "ui",
            "flow",
        ],
    ) || repo_terms.contains("playwright")
}

fn should_hint_figma(task_terms: &[String], repo_terms: &HashSet<String>) -> bool {
    let task_set = task_terms
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    contains_any(
        &task_set,
        &[
            "figma",
            "design",
            "node",
            "component",
            "screenshot",
            "variables",
        ],
    ) || repo_terms.contains("figma")
}

fn should_hint_docx(task_terms: &[String], repo_terms: &HashSet<String>) -> bool {
    let task_set = task_terms
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    contains_any(
        &task_set,
        &[
            "doc", "docx", "document", "word", "template", "report", "table",
        ],
    ) || repo_terms.contains("docx")
        || (repo_terms.contains("python") && repo_terms.contains("docx"))
}

fn should_hint_cloudflare(task_terms: &[String], repo_terms: &HashSet<String>) -> bool {
    let task_set = task_terms
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    contains_any(
        &task_set,
        &[
            "cloudflare",
            "workers",
            "pages",
            "tunnel",
            "wrangler",
            "dns",
            "deploy",
        ],
    ) || repo_terms.contains("cloudflare")
        || repo_terms.contains("wrangler")
}

fn should_hint_imagegen(task_terms: &[String], repo_terms: &HashSet<String>) -> bool {
    let task_set = task_terms
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    contains_any(
        &task_set,
        &[
            "image",
            "images",
            "inpaint",
            "mask",
            "background",
            "transparent",
            "product",
        ],
    ) || repo_terms.contains("image")
}

fn should_hint_sora(task_terms: &[String], repo_terms: &HashSet<String>) -> bool {
    let task_set = task_terms
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    contains_any(
        &task_set,
        &[
            "sora",
            "video",
            "remix",
            "storyboard",
            "thumbnail",
            "spritesheet",
        ],
    ) || repo_terms.contains("sora")
}

fn contains_any(haystack: &HashSet<&str>, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn summarize_skill_ids_for_log(skill_ids: &[String], max_visible: usize) -> String {
    if skill_ids.is_empty() {
        return "none".to_string();
    }

    let visible = skill_ids
        .iter()
        .take(max_visible)
        .cloned()
        .collect::<Vec<_>>();
    let remaining = skill_ids.len().saturating_sub(visible.len());

    if remaining == 0 {
        visible.join(", ")
    } else {
        format!("{} +{} more", visible.join(", "), remaining)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskExecutionIntent {
    Delivery,
    FocusedChange,
    ResearchProbe,
}

fn derive_skill_chain(
    task: &Task,
    settings: &ProjectSettings,
    project_type: ProjectType,
) -> Vec<String> {
    let execution_intent = infer_task_execution_intent(task);
    let mut ids: Vec<String> = vec!["task-preflight-check".to_string()];
    let mut seen: HashSet<String> = ids.iter().cloned().collect();
    let expects_delivery_flow = matches!(execution_intent, TaskExecutionIntent::Delivery);
    let preview_flow_required = task_requires_preview_flow(task, settings, execution_intent);
    let artifact_required = task_requires_artifact_skill(task, execution_intent, preview_flow_required);
    let git_handoff_required = task_requires_git_handoff(task, execution_intent);
    let release_summary_required = task_requires_release_summary(task, execution_intent);

    if expects_delivery_flow || task_explicitly_requests_env_validation(task) {
        push_skill(&mut ids, &mut seen, "env-and-secrets-validate");
    }

    let require_review = task
        .metadata
        .get("execution")
        .and_then(|v| v.get("require_review"))
        .and_then(|v| v.as_bool())
        .or_else(|| {
            task.metadata
                .get("require_review")
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(settings.require_review);

    let run_build_and_tests = task
        .metadata
        .get("execution")
        .and_then(|v| v.get("run_build_and_tests"))
        .and_then(|v| v.as_bool())
        .unwrap_or(expects_delivery_flow || task_requires_build_validation(task));

    if run_build_and_tests {
        push_skill(&mut ids, &mut seen, "verify-test-build");
    }

    if matches!(task.task_type, TaskType::Deploy) {
        push_skill(&mut ids, &mut seen, "build-artifact");
        push_skill(&mut ids, &mut seen, "deploy-ssh-remote");
        if run_build_and_tests {
            push_skill(&mut ids, &mut seen, "verify-test-build");
        }
        push_skill(&mut ids, &mut seen, "release-note-and-delivery-summary");
        for explicit in extract_explicit_skills(&task.metadata) {
            push_skill(&mut ids, &mut seen, &explicit);
        }
        push_skill(&mut ids, &mut seen, "final-report");
        return ids;
    }

    if matches!(execution_intent, TaskExecutionIntent::ResearchProbe) {
        for explicit in extract_explicit_skills(&task.metadata) {
            push_skill(&mut ids, &mut seen, &explicit);
        }
        push_skill(&mut ids, &mut seen, "final-report");
        return ids;
    }

    if matches!(task.task_type, TaskType::Init) {
        push_skill(&mut ids, &mut seen, "init-read-references");
        match project_type {
            ProjectType::Web => push_skill(&mut ids, &mut seen, "init-web-scaffold"),
            ProjectType::Api => push_skill(&mut ids, &mut seen, "init-api-scaffold"),
            ProjectType::Mobile => push_skill(&mut ids, &mut seen, "init-mobile-scaffold"),
            ProjectType::Extension => push_skill(&mut ids, &mut seen, "init-extension-scaffold"),
            ProjectType::Desktop => push_skill(&mut ids, &mut seen, "init-desktop-scaffold"),
            ProjectType::Microservice => {
                push_skill(&mut ids, &mut seen, "init-microservice-scaffold")
            }
        }
        push_skill(&mut ids, &mut seen, "init-project-bootstrap");
        push_skill(&mut ids, &mut seen, "init-project-context-file");
        push_skill(&mut ids, &mut seen, "init-source-repository");
    } else {
        push_skill(&mut ids, &mut seen, "code-implement");
    }

    if settings.auto_retry {
        push_skill(&mut ids, &mut seen, "retry-triage-and-recovery");
    }

    match project_type {
        ProjectType::Web => {
            if artifact_required {
                push_skill(&mut ids, &mut seen, "build-artifact");
            }
            if preview_flow_required {
                push_skill(&mut ids, &mut seen, "cloudflare-config-validate");
                push_skill(&mut ids, &mut seen, "cloudflare-tunnel-setup-guide");
                push_skill(&mut ids, &mut seen, "deploy-precheck-cloudflare");
                push_skill(&mut ids, &mut seen, "setup-cloudflare-tunnel");
            }
        }
        ProjectType::Api => {
            if artifact_required {
                push_skill(&mut ids, &mut seen, "build-artifact");
            }
            if preview_flow_required {
                push_skill(&mut ids, &mut seen, "cloudflare-config-validate");
                push_skill(&mut ids, &mut seen, "cloudflare-tunnel-setup-guide");
                push_skill(&mut ids, &mut seen, "deploy-precheck-cloudflare");
                push_skill(&mut ids, &mut seen, "setup-cloudflare-tunnel");
            }
        }
        ProjectType::Desktop => {
            if artifact_required {
                push_skill(&mut ids, &mut seen, "build-artifact");
            }
            if preview_flow_required {
                push_skill(&mut ids, &mut seen, "preview-artifact-desktop");
            }
        }
        ProjectType::Mobile => {
            if artifact_required {
                push_skill(&mut ids, &mut seen, "build-artifact");
            }
            if preview_flow_required {
                push_skill(&mut ids, &mut seen, "preview-artifact-mobile");
            }
        }
        ProjectType::Extension => {
            if artifact_required {
                push_skill(&mut ids, &mut seen, "build-artifact");
            }
            if preview_flow_required {
                push_skill(&mut ids, &mut seen, "preview-artifact-extension");
            }
        }
        ProjectType::Microservice => {
            if artifact_required {
                push_skill(&mut ids, &mut seen, "build-artifact");
            }
            if preview_flow_required {
                push_skill(&mut ids, &mut seen, "post-deploy-smoke-and-healthcheck");
                push_skill(&mut ids, &mut seen, "update-deployment-metadata");
            }
        }
    }

    if task_mentions_database_changes(task) {
        push_skill(&mut ids, &mut seen, "db-migration-safety");
    }

    if task_mentions_deploy_cancel_or_cleanup(task) {
        push_skill(&mut ids, &mut seen, "deploy-cancel-stop-cleanup");
    }

    if require_review {
        push_skill(&mut ids, &mut seen, "review-handoff");
        push_skill(&mut ids, &mut seen, "gitlab-rebase-conflict-resolution");
    } else if !matches!(task.task_type, TaskType::Init) && git_handoff_required {
        push_skill(&mut ids, &mut seen, "gitlab-branch-and-commit");
        push_skill(&mut ids, &mut seen, "gitlab-merge-request");
        push_skill(&mut ids, &mut seen, "gitlab-issue-sync");
    }

    if release_summary_required {
        push_skill(&mut ids, &mut seen, "release-note-and-delivery-summary");
    }

    for explicit in extract_explicit_skills(&task.metadata) {
        push_skill(&mut ids, &mut seen, &explicit);
    }

    push_skill(&mut ids, &mut seen, "final-report");
    ids
}

fn infer_task_execution_intent(task: &Task) -> TaskExecutionIntent {
    if matches!(task.task_type, TaskType::Init | TaskType::Deploy) {
        return TaskExecutionIntent::Delivery;
    }

    if task
        .metadata
        .get("execution")
        .and_then(|value| value.get("no_code"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        || task
            .metadata
            .get("no_code")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    {
        return TaskExecutionIntent::ResearchProbe;
    }

    if let Some(intent) = task
        .metadata
        .get("execution")
        .and_then(|value| value.get("intent"))
        .and_then(|value| value.as_str())
        .or_else(|| task.metadata.get("intent").and_then(|value| value.as_str()))
    {
        let normalized = intent.trim().to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "research" | "analysis" | "probe" | "runtime_skill_probe" | "read_only" | "no_code"
        ) {
            return TaskExecutionIntent::ResearchProbe;
        }
        if matches!(
            normalized.as_str(),
            "delivery" | "deploy" | "preview" | "release" | "init" | "bootstrap"
        ) {
            return TaskExecutionIntent::Delivery;
        }
        if matches!(
            normalized.as_str(),
            "focused_change" | "focused" | "implementation" | "small_task" | "small-change"
        ) {
            return TaskExecutionIntent::FocusedChange;
        }
    }

    let haystack = task_text(task).to_ascii_lowercase();
    if task_has_runtime_skill_probe_markers(&haystack) {
        return TaskExecutionIntent::ResearchProbe;
    }

    let research_markers = task_has_research_markers(&haystack);
    let change_markers = task_has_change_markers(&haystack);
    let research_task_type = matches!(task.task_type, TaskType::Spike);
    let docs_research_task = matches!(task.task_type, TaskType::Docs)
        && (haystack.contains("official documentation")
            || haystack.contains("citation")
            || haystack.contains("citations")
            || haystack.contains("reference"));

    if (research_markers && !change_markers)
        || ((research_task_type || docs_research_task) && !change_markers)
    {
        return TaskExecutionIntent::ResearchProbe;
    }

    if matches!(task.task_type, TaskType::SmallTask) || !task_has_delivery_markers(&haystack) {
        return TaskExecutionIntent::FocusedChange;
    }

    TaskExecutionIntent::Delivery
}

fn task_requires_build_validation(task: &Task) -> bool {
    let haystack = task_text(task).to_ascii_lowercase();
    matches!(task.task_type, TaskType::Test)
        || [
            "build",
            "test",
            "verify",
            "validation",
            "validate",
            "compile",
            "lint",
            "ci",
            "dist",
            "bundle",
        ]
        .iter()
        .any(|needle| haystack.contains(needle))
}

fn task_requires_artifact_skill(
    task: &Task,
    execution_intent: TaskExecutionIntent,
    preview_flow_required: bool,
) -> bool {
    if matches!(task.task_type, TaskType::Init | TaskType::Deploy) || preview_flow_required {
        return true;
    }

    let haystack = task_text(task).to_ascii_lowercase();
    matches!(execution_intent, TaskExecutionIntent::Delivery)
        || [
            "artifact",
            "bundle",
            "package",
            "dist",
            "build output",
            "binary",
        ]
        .iter()
        .any(|needle| haystack.contains(needle))
}

fn task_requires_preview_flow(
    task: &Task,
    settings: &ProjectSettings,
    execution_intent: TaskExecutionIntent,
) -> bool {
    if matches!(task.task_type, TaskType::Init | TaskType::Deploy) {
        return settings.preview_enabled || settings.auto_deploy || task_requests_auto_deploy(task);
    }

    let haystack = task_text(task).to_ascii_lowercase();
    let mentions_preview_or_deploy = [
        "preview",
        "deploy",
        "deployment",
        "cloudflare",
        "workers",
        "pages",
        "tunnel",
        "wrangler",
        "publish",
        "release",
        "go live",
    ]
    .iter()
    .any(|needle| haystack.contains(needle));

    task_requests_auto_deploy(task)
        || (matches!(execution_intent, TaskExecutionIntent::Delivery)
            && mentions_preview_or_deploy
            && (settings.preview_enabled || settings.auto_deploy))
}

fn task_requires_git_handoff(task: &Task, execution_intent: TaskExecutionIntent) -> bool {
    if matches!(task.task_type, TaskType::Init | TaskType::Deploy)
        || matches!(execution_intent, TaskExecutionIntent::Delivery)
    {
        return true;
    }

    let haystack = task_text(task).to_ascii_lowercase();
    [
        "gitlab",
        "merge request",
        "pull request",
        "commit",
        "branch",
        "push",
        "repo",
        "repository",
        "handoff",
        "review",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn task_requires_release_summary(task: &Task, execution_intent: TaskExecutionIntent) -> bool {
    if matches!(task.task_type, TaskType::Init | TaskType::Deploy)
        || matches!(execution_intent, TaskExecutionIntent::Delivery)
    {
        return true;
    }

    let haystack = task_text(task).to_ascii_lowercase();
    [
        "release note",
        "delivery summary",
        "handoff",
        "what changed",
        "release",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn task_requests_auto_deploy(task: &Task) -> bool {
    task.metadata
        .get("execution")
        .and_then(|v| v.get("auto_deploy"))
        .and_then(|v| v.as_bool())
        .or_else(|| task.metadata.get("auto_deploy").and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

fn task_expects_init_flow(task: &Task) -> bool {
    if matches!(task.task_type, TaskType::Init) {
        return true;
    }

    let haystack = task_text(task).to_ascii_lowercase();
    [
        "from scratch",
        "bootstrap",
        "scaffold",
        "initialize",
        "initialise",
        "new project",
        "starter",
        "kickoff project",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn suggested_skill_allowed_for_task(task: &Task, skill_id: &str) -> bool {
    if skill_id.starts_with("init-") && !task_expects_init_flow(task) {
        return false;
    }

    true
}

fn task_has_delivery_markers(haystack: &str) -> bool {
    [
        "ship",
        "release",
        "deliver",
        "deploy",
        "deployment",
        "preview",
        "publish",
        "go live",
        "handoff",
        "scaffold",
        "bootstrap",
        "create repository",
        "create repo",
        "gitlab repository",
        "push to",
        "merge request",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn task_explicitly_requests_env_validation(task: &Task) -> bool {
    let haystack = task_text(task).to_ascii_lowercase();
    [
        "api key",
        "token",
        "credential",
        "credentials",
        "auth",
        "environment variable",
        "env ",
        "mcp",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn task_has_runtime_skill_probe_markers(haystack: &str) -> bool {
    [
        "runtime skill extension",
        "search_skills",
        "load_skill",
        "which skill was loaded",
        "what you loaded and why",
        "summary-only",
        "no-code probe",
        "no code probe",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn task_has_research_markers(haystack: &str) -> bool {
    [
        "research ",
        "investigate",
        "analysis",
        "analyze",
        "probe",
        "explore",
        "evaluate",
        "compare",
        "official documentation",
        "citations",
        "reference",
        "references",
        "read-only",
        "read only",
        "no-code",
        "no code",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn task_has_change_markers(haystack: &str) -> bool {
    [
        "implement",
        "fix",
        "refactor",
        "change ",
        "edit ",
        "update ",
        "add ",
        "remove ",
        "create ",
        "write ",
        "patch",
        "ship ",
        "deploy",
        "commit",
        "merge request",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn extract_explicit_skills(metadata: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut push_from = |value: Option<&serde_json::Value>| {
        let Some(value) = value else {
            return;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                if let Some(skill) = item.as_str() {
                    let skill = skill.trim();
                    if !skill.is_empty() {
                        out.push(skill.to_string());
                    }
                }
            }
        }
    };

    push_from(metadata.get("skills"));
    push_from(metadata.get("execution").and_then(|v| v.get("skills")));
    push_from(metadata.get("execution").and_then(|v| v.get("skill_chain")));
    out
}

fn task_mentions_deploy_cancel_or_cleanup(task: &Task) -> bool {
    let mut haystack = String::new();
    haystack.push_str(&task.title.to_lowercase());
    haystack.push(' ');
    if let Some(description) = &task.description {
        haystack.push_str(&description.to_lowercase());
    }

    [
        "cancel deploy",
        "dừng deploy",
        "stop deploy",
        "stop container",
        "dừng container",
        "docker down",
        "docker stop",
        "xoá resource",
        "remove resource",
        "cleanup deploy",
        "tear down",
        "rollback",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn task_mentions_database_changes(task: &Task) -> bool {
    let metadata_hint = task
        .metadata
        .get("execution")
        .and_then(|v| v.get("database_migration"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if metadata_hint {
        return true;
    }

    let mut haystack = String::new();
    haystack.push_str(&task.title.to_lowercase());
    haystack.push(' ');
    if let Some(description) = &task.description {
        haystack.push_str(&description.to_lowercase());
    }

    [
        "migration",
        "migrate",
        "schema",
        "database",
        "postgres",
        "sql",
        "table",
        "column",
        "index",
        "db ",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn push_skill(ids: &mut Vec<String>, seen: &mut HashSet<String>, skill: &str) {
    if seen.insert(skill.to_string()) {
        ids.push(skill.to_string());
    }
}

fn load_skill(skill_id: &str, repo_path: Option<&Path>) -> Option<LoadedSkill> {
    if !is_safe_skill_id(skill_id) {
        return None;
    }

    load_skill_from_roots(skill_id, &candidate_skill_roots(repo_path))
}

fn load_skill_from_roots(skill_id: &str, roots: &[SkillRootCandidate]) -> Option<LoadedSkill> {
    for root in roots {
        let path = root.path.join(skill_id).join("SKILL.md");
        if !path.exists() {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        return Some(LoadedSkill {
            id: skill_id.to_string(),
            content,
            source: Some(path),
            origin: Some(root.origin.clone()),
        });
    }

    None
}

fn candidate_skill_roots(repo_path: Option<&Path>) -> Vec<SkillRootCandidate> {
    candidate_skill_roots_from_globals(
        repo_path,
        crate::knowledge_index::discover_global_skill_roots(),
    )
}

fn candidate_skill_roots_from_globals(
    repo_path: Option<&Path>,
    global_roots: Vec<crate::knowledge_index::KnowledgeRoot>,
) -> Vec<SkillRootCandidate> {
    let mut roots: Vec<SkillRootCandidate> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut push = |path: PathBuf, origin: &str| {
        if path.as_os_str().is_empty() {
            return;
        }
        if seen.insert(path.clone()) {
            roots.push(SkillRootCandidate {
                path,
                origin: origin.to_string(),
            });
        }
    };

    // 1. Per-project skills (worktree .acpms/skills)
    if let Some(repo) = repo_path {
        push(repo.join(".acpms").join("skills"), "repo-local");
    }

    for root in global_roots {
        push(root.path, &root.origin);
    }

    roots
}

fn is_safe_skill_id(skill_id: &str) -> bool {
    !skill_id.is_empty()
        && skill_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

pub fn detect_skill_file(path: &Path) -> Option<DetectedSkillFile> {
    let resolved_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    if resolved_path.file_name().and_then(|value| value.to_str()) != Some("SKILL.md") {
        return None;
    }

    let skill_id = resolved_path
        .parent()?
        .file_name()?
        .to_str()?
        .trim()
        .to_string();
    if !is_safe_skill_id(&skill_id) {
        return None;
    }

    let source_path = resolved_path.to_string_lossy().to_string();
    let origin = detect_skill_origin(&resolved_path);

    Some(DetectedSkillFile {
        skill_id,
        origin,
        source_path,
    })
}

fn detect_skill_origin(path: &Path) -> String {
    let components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();

    if let Some(component) = components
        .iter()
        .find(|component| component.starts_with("community-"))
    {
        return (*component).to_string();
    }

    if let Ok(dir) = std::env::var("ACPMS_SKILLS_DIR") {
        let skills_root = PathBuf::from(dir);
        if path.starts_with(&skills_root) {
            return "platform".to_string();
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if path.starts_with(cwd.join(".acpms").join("skills")) {
            return "cwd".to_string();
        }
    }

    if let Ok(codex_home) = std::env::var("CODEX_HOME") {
        if path.starts_with(PathBuf::from(codex_home).join("skills")) {
            return "codex-home".to_string();
        }
    } else if let Some(home) = dirs::home_dir() {
        if path.starts_with(home.join(".codex").join("skills")) {
            return "codex-home".to_string();
        }
    }

    if components
        .windows(2)
        .any(|window| window == [".acpms", "skills"])
    {
        return "repo-local".to_string();
    }

    "unknown".to_string()
}

fn runtime_skill_extension_block() -> &'static str {
    r#"

## Runtime Skill Extension
If repo exploration reveals that you need an additional ACPMS skill, and the current runtime supports ACPMS runtime skill loading (Claude Code, Codex, Gemini, or Cursor), request it by printing exactly one JSON object on its own line with no markdown fences:
{"tool":"search_skills","args":{"query":"<skill or capability you need>","top_k":5}}

After you receive results, you may load one skill by id:
{"tool":"load_skill","args":{"skill_id":"<skill-id>"}}

Use this only when the currently loaded skills are insufficient. For ACPMS-managed skills, the source of truth is the repository-managed `.acpms/skills` tree and bundled community skill library. Do not read `$CODEX_HOME/skills` copies directly for a suggested ACPMS skill; if you need the full playbook, load it through `load_skill`. After a skill is loaded, follow it for the rest of the attempt where relevant.
ACPMS exposes the merged skill mirror for this attempt at `$ACPMS_MANAGED_SKILL_ROOT`. It resolves repo-managed skills first and local fallback skills second.
"#
}

fn builtin_skill_content(skill_id: &str) -> Option<&'static str> {
    match skill_id {
        "task-preflight-check" => Some(
            r#"Run first before any implementation. Validate references and environment.
- If .acpms/references/refs_manifest.json exists and has failures: STOP, output PREFLIGHT BLOCKED report.
- If reference files listed in manifest are missing: STOP.
- If git repo is broken or missing: STOP.
- Report blocking issues clearly so user can fix before retrying."#,
        ),
        "code-implement" => Some(
            r#"Implement only scoped changes for this task.
- Keep edits minimal and coherent.
- Do not modify unrelated files.
- Preserve existing architecture and conventions."#,
        ),
        "verify-test-build" => Some(
            r#"Run relevant verification commands before finishing.
- Prefer existing project scripts (test/lint/build).
- Report what passed, failed, or was skipped with reasons."#,
        ),
        "env-and-secrets-validate" => Some(
            r#"Validate required environment variables before build/deploy.
- Report missing required env names only (never secret values).
- Stop dependent steps when required configuration is missing."#,
        ),
        "init-read-references" => Some(
            r#"If .acpms-refs/ exists and is non-empty, read reference files before scaffolding.
- List and read source code, configs, specs, or mockups.
- Use insights to replicate structure and patterns in init-project-bootstrap.
- If .acpms-refs/ is missing or empty, skip this step."#,
        ),
        "init-project-bootstrap" => Some(
            r#"Bootstrap initial project structure using selected stack.
- Keep setup minimal and reproducible.
- Run baseline validation and summarize generated artifacts."#,
        ),
        "init-project-context-file" => Some(
            r#"Create PROJECT_CONTEXT.md with architecture overview and development guidelines.
- Use project-type-appropriate content (web, API, mobile, etc.).
- Include key commands and workflows for future AI agents."#,
        ),
        "init-web-scaffold" => Some(
            r#"Web app scaffold: package.json, build tools (Vite/Next.js), TypeScript, README, .gitignore, .env.example, ESLint/Prettier, src/, public/, routing. Use Project Details for name/description."#,
        ),
        "init-api-scaffold" => Some(
            r#"API scaffold: init project (Cargo/package/requirements), web framework, README, .gitignore, Docker, routes, middleware, health check, /api/v1/, CRUD template. Use Project Details for name/description."#,
        ),
        "init-mobile-scaffold" => Some(
            r#"Mobile scaffold: React Native/Expo/Flutter, platform config, README, .gitignore, Info.plist, AndroidManifest, src/lib/, navigation, screens. Use Project Details for name/description."#,
        ),
        "init-extension-scaffold" => Some(
            r#"Extension scaffold: manifest.json V3, build tools, README, background/content/popup/options, permissions, multi-browser. Use Project Details for name/description."#,
        ),
        "init-desktop-scaffold" => Some(
            r#"Desktop scaffold: Electron/Tauri, main/renderer, README, .gitignore, IPC, packaging, code signing. Use Project Details for name/description."#,
        ),
        "init-microservice-scaffold" => Some(
            r#"Microservice scaffold: go.mod/Cargo.toml, Dockerfile, docker-compose, health/ready/live, metrics, logging, cmd/, api/, configs/. Use Project Details for name/description."#,
        ),
        "init-import-analyze" => Some(
            r#"Analyze imported repository: explore directory structure, identify services/components, evaluate tech stack.
- List key dirs (src/, app/, packages/, services/).
- Identify frontend, backend, database, auth, cache, queue, storage.
- Write .acpms/import-analysis.json with architecture (nodes, edges) and assessment (project_type, summary, services, tech_stack).
- Node types: client, frontend, api, database, cache, queue, storage, auth, gateway, service, mobile, worker.
- Read-only: do not modify source code."#,
        ),
        "build-artifact" => Some(
            r#"Produce build artifacts appropriate for project type.
- Ensure output path exists.
- Record artifact commands and output summary in report."#,
        ),
        "preview-artifact-desktop" => Some(
            r#"For desktop task previews, produce installable desktop artifacts.
- Keep build command/output dir aligned with project metadata.
- Prefer native installers or platform bundles in the desktop output folder.
- Report install notes for macOS and Windows when applicable."#,
        ),
        "preview-artifact-mobile" => Some(
            r#"For mobile task previews, produce downloadable test artifacts.
- Prefer APK/AAB for Android; note clearly when iOS requires signing or simulator-only output.
- Keep build command/output dir aligned with project metadata.
- Report install steps and platform limitations."#,
        ),
        "preview-artifact-extension" => Some(
            r#"For extension task previews, produce downloadable extension bundles.
- Prefer a ready-to-load .zip when the build already emits one; otherwise ensure the output directory can be zipped.
- Verify manifest/build output is complete.
- Report browser load/install steps in the final summary."#,
        ),
        "cloudflare-config-validate" => Some(
            r#"Validate Cloudflare account and API token before tunnel/deploy.
- If missing, report cloudflare_not_configured.
- Skip unsafe steps and continue completion flow."#,
        ),
        "cloudflare-tunnel-setup-guide" => Some(
            r#"Guide for Cloudflare tunnel preview. Required System Settings: Account ID, API Token, Zone ID, Base Domain.
- Output PREVIEW_TARGET: http://127.0.0.1:<port> when preview needed.
- When tunnel fails: tell user to ensure all 4 fields in System Settings (/settings)."#,
        ),
        "deploy-cloudflare-pages" => Some(
            r#"Deploy web build to Cloudflare Pages flow configured for this project.
- Validate deploy command/config.
- Capture resulting deployment URL."#,
        ),
        "deploy-cloudflare-workers" => Some(
            r#"Deploy API runtime to Cloudflare Workers flow configured for this project.
- Validate worker config.
- Capture resulting deployment URL/endpoint."#,
        ),
        "setup-cloudflare-tunnel" => Some(
            r#"Prepare preview tunnel details for web/api.
- Produce PREVIEW_TARGET for runtime endpoint.
- If public URL is available, output PREVIEW_URL."#,
        ),
        "deploy-precheck-cloudflare" => Some(
            r#"Before deploy/tunnel, verify Cloudflare settings are configured.
- If missing, report: cloudflare not configured.
- Skip deploy/tunnel safely and continue normal completion flow."#,
        ),
        "cloudflare-dns-route" => Some(
            r#"Ensure DNS route points to deployed target.
- Create/update record idempotently.
- Report hostname, record type, and status."#,
        ),
        "post-deploy-smoke-and-healthcheck" => Some(
            r#"Run health and smoke checks after deployment.
- Validate critical endpoints.
- Report validation status and rollback recommendation."#,
        ),
        "update-deployment-metadata" => Some(
            r#"Emit metadata-aligned deployment summary fields.
- Include deployment_status and production_deployment_status.
- Include errors/reasons when skipped or failed."#,
        ),
        "review-handoff" => Some(
            r#"Prepare reviewer handoff when require-review mode is enabled.
- Do not commit or push in review mode.
- Report changed files, risks, and reviewer actions."#,
        ),
        "gitlab-branch-and-commit" => Some(
            r#"Perform branch, stage, commit, and push workflow safely.
- Stage only task-related files.
- Report commit hash and push status."#,
        ),
        "gitlab-ci-verify" => Some(
            r#"Check CI pipeline status for pushed changes.
- Report pass/fail/pending with pipeline link or context."#,
        ),
        "gitlab-merge-request" => Some(
            r#"Create or update merge request with summary, verification, and deployment notes.
- Avoid duplicate MR creation.
- Report MR URL and action."#,
        ),
        "gitlab-issue-sync" => Some(
            r#"Sync completion status back to linked issue when available.
- Include MR/deploy links and blockers.
- Skip clearly if no issue reference exists."#,
        ),
        "retry-triage-and-recovery" => Some(
            r#"Classify failures and choose safe retry action.
- Retry only transient failures.
- Report recovery actions and retry decision."#,
        ),
        "gitlab-rebase-conflict-resolution" => Some(
            r#"Resolve branch divergence and rebase conflicts.
- Fetch origin, then ALWAYS rebase onto origin/main (do not skip; "Already up to date" from fetch ≠ branch integrated).
- Resolve conflicts using task intent as tie-breaker.
- Verify and push (--force-with-lease if rebased). Do NOT suggest "retry on GitLab"—you must run the commands and fix conflicts."#,
        ),
        "db-migration-safety" => Some(
            r#"Apply safe migration strategy with backward compatibility.
- Prefer additive changes.
- Document rollback plan and migration risk."#,
        ),
        "release-note-and-delivery-summary" => Some(
            r#"Produce release-ready delivery summary.
- Include code, validation, deploy status, and follow-ups.
- Keep summary concise and evidence-based."#,
        ),
        "final-report" => Some(
            r#"End with a final report section:
- Task summary
- Deployment status
- Commands executed
- URLs/endpoints
- Verification results
- Remaining risks/issues"#,
        ),
        "rollback-deploy" => Some(
            r#"If deployment is unsafe or broken, rollback to previous stable deployment reference.
- Report rollback target and reason."#,
        ),
        "deploy-ssh-remote" => Some(
            r#"Build artifact and deploy directly via SSH.
- Run project build. Verify artifact exists. Run tests if available.
- Use .acpms/deploy/ssh_key and .acpms/deploy/config.json to SSH to server.
- Copy artifact (rsync/scp) to deploy_path. Run deploy script if needed.
- Report build_status, artifact_paths, deployment_status in final report."#,
        ),
        "deploy-cancel-stop-cleanup" => Some(
            r#"Cancel deploy, stop containers/processes, clean resources.
- Cancel ACPMS run via UI (Deployments tab → Cancel) or API if token available.
- Use .acpms/deploy/ to SSH; run docker compose down, docker stop, pkill as needed.
- Remove resources only when task explicitly asks. Report cleanup_status."#,
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_index::{SkillKnowledgeBackend, SkillKnowledgeHandle, SkillMatch};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex;

    struct FakeBackend {
        matches: Vec<SkillMatch>,
        contents: HashMap<String, String>,
    }

    impl SkillKnowledgeBackend for FakeBackend {
        fn search(&self, _query: &str, _top_k: usize) -> anyhow::Result<Vec<SkillMatch>> {
            Ok(self.matches.clone())
        }

        fn read_skill(&self, skill_id: &str) -> anyhow::Result<Option<String>> {
            Ok(self.contents.get(skill_id).cloned())
        }

        fn skill_count(&self) -> usize {
            self.matches.len()
        }
    }

    struct SearchWindowBackend {
        min_top_k: usize,
        requested_top_ks: Mutex<Vec<usize>>,
        match_result: SkillMatch,
    }

    impl SkillKnowledgeBackend for SearchWindowBackend {
        fn search(&self, _query: &str, top_k: usize) -> anyhow::Result<Vec<SkillMatch>> {
            self.requested_top_ks.lock().unwrap().push(top_k);
            if top_k >= self.min_top_k {
                Ok(vec![self.match_result.clone()])
            } else {
                Ok(Vec::new())
            }
        }

        fn read_skill(&self, skill_id: &str) -> anyhow::Result<Option<String>> {
            Ok(Some(format!("Skill content for {skill_id}")))
        }

        fn skill_count(&self) -> usize {
            128
        }
    }

    struct QueryAwareBackend {
        requested_queries: Mutex<Vec<String>>,
        matcher: Arc<dyn Fn(&str) -> Option<SkillMatch> + Send + Sync>,
    }

    impl SkillKnowledgeBackend for QueryAwareBackend {
        fn search(&self, query: &str, _top_k: usize) -> anyhow::Result<Vec<SkillMatch>> {
            self.requested_queries
                .lock()
                .unwrap()
                .push(query.to_string());
            Ok((self.matcher)(query).into_iter().collect())
        }

        fn read_skill(&self, skill_id: &str) -> anyhow::Result<Option<String>> {
            Ok(Some(format!("Skill content for {skill_id}")))
        }

        fn skill_count(&self) -> usize {
            128
        }
    }

    fn sample_task() -> Task {
        Task {
            id: uuid::Uuid::new_v4(),
            project_id: uuid::Uuid::new_v4(),
            requirement_id: None,
            sprint_id: None,
            title: "Implement docs integration".to_string(),
            description: Some("Need better OpenAI docs grounding".to_string()),
            task_type: TaskType::Feature,
            status: acpms_db::models::TaskStatus::Todo,
            assigned_to: None,
            parent_task_id: None,
            gitlab_issue_id: None,
            metadata: serde_json::json!({}),
            created_by: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn explicit_skills_from_metadata_are_loaded() {
        let metadata = serde_json::json!({
            "skills": ["foo"],
            "execution": {
                "skills": ["bar"],
                "skill_chain": ["baz"]
            }
        });
        let skills = extract_explicit_skills(&metadata);
        assert_eq!(skills, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn unsafe_skill_id_is_rejected() {
        assert!(is_safe_skill_id("deploy-cloudflare-pages"));
        assert!(!is_safe_skill_id("../escape"));
        assert!(!is_safe_skill_id("UPPER"));
    }

    #[test]
    fn db_migration_skill_uses_metadata_hint() {
        let task = Task {
            id: uuid::Uuid::new_v4(),
            project_id: uuid::Uuid::new_v4(),
            requirement_id: None,
            sprint_id: None,
            title: "Add users index".to_string(),
            description: None,
            task_type: TaskType::Feature,
            status: acpms_db::models::TaskStatus::Todo,
            assigned_to: None,
            parent_task_id: None,
            gitlab_issue_id: None,
            metadata: serde_json::json!({
                "execution": {
                    "database_migration": true
                }
            }),
            created_by: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(task_mentions_database_changes(&task));
    }

    #[test]
    fn binary_preview_projects_add_project_specific_preview_skill() {
        let task = Task {
            id: uuid::Uuid::new_v4(),
            project_id: uuid::Uuid::new_v4(),
            requirement_id: None,
            sprint_id: None,
            title: "Ship desktop preview".to_string(),
            description: None,
            task_type: TaskType::Feature,
            status: acpms_db::models::TaskStatus::Todo,
            assigned_to: None,
            parent_task_id: None,
            gitlab_issue_id: None,
            metadata: serde_json::json!({}),
            created_by: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let settings = ProjectSettings {
            preview_enabled: true,
            ..ProjectSettings::default()
        };

        let skills = resolve_skill_chain(&task, &settings, ProjectType::Desktop);

        assert!(skills.iter().any(|skill| skill == "build-artifact"));
        assert!(skills
            .iter()
            .any(|skill| skill == "preview-artifact-desktop"));
    }

    #[test]
    fn project_preview_alias_enables_preview_target_skill_chain_without_production_deploy() {
        let task = Task {
            id: uuid::Uuid::new_v4(),
            project_id: uuid::Uuid::new_v4(),
            requirement_id: None,
            sprint_id: None,
            title: "Ship api preview".to_string(),
            description: None,
            task_type: TaskType::Feature,
            status: acpms_db::models::TaskStatus::Todo,
            assigned_to: None,
            parent_task_id: None,
            gitlab_issue_id: None,
            metadata: serde_json::json!({}),
            created_by: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let settings = ProjectSettings {
            auto_deploy: false,
            preview_enabled: true,
            ..ProjectSettings::default()
        };

        let skills = resolve_skill_chain(&task, &settings, ProjectType::Api);

        assert!(skills
            .iter()
            .any(|skill| skill == "deploy-precheck-cloudflare"));
        assert!(skills
            .iter()
            .any(|skill| skill == "setup-cloudflare-tunnel"));
        assert!(!skills
            .iter()
            .any(|skill| skill == "deploy-cloudflare-workers"));
        assert!(!skills.iter().any(|skill| skill == "cloudflare-dns-route"));
    }

    #[test]
    fn web_preview_skill_chain_skips_production_pages_deploy_for_standard_tasks() {
        let task = Task {
            id: uuid::Uuid::new_v4(),
            project_id: uuid::Uuid::new_v4(),
            requirement_id: None,
            sprint_id: None,
            title: "Ship web preview".to_string(),
            description: None,
            task_type: TaskType::Feature,
            status: acpms_db::models::TaskStatus::Todo,
            assigned_to: None,
            parent_task_id: None,
            gitlab_issue_id: None,
            metadata: serde_json::json!({}),
            created_by: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let settings = ProjectSettings {
            auto_deploy: true,
            ..ProjectSettings::default()
        };

        let skills = resolve_skill_chain(&task, &settings, ProjectType::Web);

        assert!(skills
            .iter()
            .any(|skill| skill == "deploy-precheck-cloudflare"));
        assert!(skills
            .iter()
            .any(|skill| skill == "setup-cloudflare-tunnel"));
        assert!(!skills
            .iter()
            .any(|skill| skill == "deploy-cloudflare-pages"));
    }

    #[test]
    fn runtime_skill_probe_tasks_use_minimal_skill_chain() {
        let task = Task {
            title: "Research OpenAI Responses API docs for tool calling".to_string(),
            description: Some(
                "Use the ACPMS runtime skill extension with search_skills and load_skill, then report what you loaded and why."
                    .to_string(),
            ),
            ..sample_task()
        };

        let skills = resolve_skill_chain(&task, &ProjectSettings::default(), ProjectType::Api);

        assert_eq!(skills, vec!["task-preflight-check", "final-report"]);
    }

    #[test]
    fn explicit_no_code_metadata_uses_minimal_skill_chain_even_for_feature_tasks() {
        let task = Task {
            title: "Implement docs integration".to_string(),
            description: Some("Probe the current docs setup without changing code.".to_string()),
            metadata: serde_json::json!({
                "execution": {
                    "no_code": true
                }
            }),
            ..sample_task()
        };

        let skills = resolve_skill_chain(&task, &ProjectSettings::default(), ProjectType::Web);

        assert_eq!(skills, vec!["task-preflight-check", "final-report"]);
    }

    #[test]
    fn skill_instruction_context_injects_suggested_knowledge_when_ready() {
        let handle = SkillKnowledgeHandle::pending();
        let backend = FakeBackend {
            matches: vec![SkillMatch {
                skill_id: "openai-docs".to_string(),
                name: "OpenAI Docs".to_string(),
                description: "Use official docs for API work".to_string(),
                score: 0.92,
                source_path: PathBuf::from("/tmp/openai-docs/SKILL.md"),
                origin: "community-openai".to_string(),
            }],
            contents: HashMap::from([(
                "openai-docs".to_string(),
                "Use official docs and cite exact endpoints.".to_string(),
            )]),
        };
        handle.set_ready_backend(Arc::new(backend));

        let context = build_skill_instruction_context(
            &sample_task(),
            &ProjectSettings::default(),
            ProjectType::Api,
            None,
            Some(&handle),
        );

        assert_eq!(context.knowledge_status, SkillKnowledgeStatus::Ready);
        assert_eq!(context.suggested_skills.len(), 1);
        assert!(context.block.contains("## Suggested Knowledge"));
        assert!(context.block.contains("Origin: `community-openai`"));
        assert!(context.block.contains("Relevance: `"));
        assert!(context.suggested_skills[0].score >= 0.92);
    }

    #[test]
    fn skill_instruction_context_reports_pending_without_injecting_suggestions() {
        let handle = SkillKnowledgeHandle::pending();

        let context = build_skill_instruction_context(
            &sample_task(),
            &ProjectSettings::default(),
            ProjectType::Api,
            None,
            Some(&handle),
        );

        assert_eq!(context.knowledge_status, SkillKnowledgeStatus::Pending);
        assert!(context.suggested_skills.is_empty());
        assert!(!context.block.contains("## Suggested Knowledge"));
    }

    #[test]
    fn skill_instruction_context_filters_duplicate_suggested_skill_ids() {
        let handle = SkillKnowledgeHandle::pending();
        let backend = FakeBackend {
            matches: vec![SkillMatch {
                skill_id: "env-and-secrets-validate".to_string(),
                name: "Env Validate".to_string(),
                description: "duplicate".to_string(),
                score: 0.99,
                source_path: PathBuf::from("/tmp/env/SKILL.md"),
                origin: "platform".to_string(),
            }],
            contents: HashMap::new(),
        };
        handle.set_ready_backend(Arc::new(backend));

        let context = build_skill_instruction_context(
            &sample_task(),
            &ProjectSettings::default(),
            ProjectType::Api,
            None,
            Some(&handle),
        );

        assert_eq!(context.knowledge_status, SkillKnowledgeStatus::NoMatches);
        assert!(context.suggested_skills.is_empty());
    }

    #[test]
    fn build_skill_plan_tracks_selected_and_skipped_candidates() {
        let handle = SkillKnowledgeHandle::pending();
        let backend = FakeBackend {
            matches: vec![
                SkillMatch {
                    skill_id: "env-and-secrets-validate".to_string(),
                    name: "Env Validate".to_string(),
                    description: "duplicate".to_string(),
                    score: 0.99,
                    source_path: PathBuf::from("/tmp/env/SKILL.md"),
                    origin: "platform".to_string(),
                },
                SkillMatch {
                    skill_id: "openai-docs".to_string(),
                    name: "OpenAI Docs".to_string(),
                    description: "Use official docs".to_string(),
                    score: 0.81,
                    source_path: PathBuf::from("/tmp/openai-docs/SKILL.md"),
                    origin: "community-openai".to_string(),
                },
            ],
            contents: HashMap::new(),
        };
        handle.set_ready_backend(Arc::new(backend));

        let plan = build_skill_plan(
            &sample_task(),
            &ProjectSettings::default(),
            ProjectType::Api,
            None,
            Some(&handle),
        );

        assert_eq!(plan.knowledge_status, SkillKnowledgeStatus::Ready);
        assert_eq!(plan.suggested.len(), 1);
        assert_eq!(plan.suggested[0].skill_id, "openai-docs");
        assert!(plan
            .skipped
            .iter()
            .any(|skill| skill.skill_id == "env-and-secrets-validate"
                && skill.decision == SkillPlanDecision::SkippedDuplicate));
        assert!(plan.trace.iter().any(|trace| {
            trace.skill_id == "openai-docs"
                && trace.decision == SkillPlanDecision::SelectedSuggested
        }));
    }

    #[test]
    fn build_skill_plan_combines_extension_support_in_ranking_trace() {
        let handle = SkillKnowledgeHandle::pending();
        let backend = Arc::new(QueryAwareBackend {
            requested_queries: Mutex::new(Vec::new()),
            matcher: Arc::new(|query| {
                if query.to_ascii_lowercase().contains("openai") {
                    Some(SkillMatch {
                        skill_id: "openai-docs".to_string(),
                        name: "OpenAI Docs".to_string(),
                        description: "Use official OpenAI docs".to_string(),
                        score: 0.52,
                        source_path: PathBuf::from("/tmp/openai-docs/SKILL.md"),
                        origin: "community-openai".to_string(),
                    })
                } else {
                    None
                }
            }),
        });
        handle.set_ready_backend(backend.clone());

        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_dir.path().join("package.json"),
            r#"{"dependencies":{"openai":"^4.0.0"}}"#,
        )
        .unwrap();

        let task = Task {
            title: "Research OpenAI Responses API docs for tool calling".to_string(),
            description: Some(
                "Need official OpenAI documentation, citations, Responses API, Chat Completions, Agents SDK, model limits."
                    .to_string(),
            ),
            ..sample_task()
        };

        let plan = build_skill_plan(
            &task,
            &ProjectSettings::default(),
            ProjectType::Api,
            Some(temp_dir.path()),
            Some(&handle),
        );

        assert_eq!(plan.knowledge_status, SkillKnowledgeStatus::Ready);
        assert_eq!(plan.suggested.len(), 1);
        assert!(plan.suggested[0].proposed_by.contains("agent_triage"));
        assert!(plan.suggested[0].proposed_by.contains("task_hints"));
        assert!(plan.suggested[0].proposed_by.contains("repo_signal_hints"));
        assert!(plan.suggested[0].score.unwrap() > 0.52);
        assert!(plan.trace.iter().any(|trace| {
            trace.skill_id == "openai-docs"
                && trace.decision == SkillPlanDecision::SelectedSuggested
                && trace.proposed_by.contains("repo_signal_hints")
        }));
    }

    #[test]
    fn skill_instruction_context_uses_wider_candidate_window_for_rag_search() {
        let handle = SkillKnowledgeHandle::pending();
        let backend = Arc::new(SearchWindowBackend {
            min_top_k: 64,
            requested_top_ks: Mutex::new(Vec::new()),
            match_result: SkillMatch {
                skill_id: "openai-docs".to_string(),
                name: "OpenAI Docs".to_string(),
                description: "Use official docs".to_string(),
                score: 0.71,
                source_path: PathBuf::from("/tmp/openai-docs/SKILL.md"),
                origin: "community-openai".to_string(),
            },
        });
        handle.set_ready_backend(backend.clone());

        let context = build_skill_instruction_context(
            &sample_task(),
            &ProjectSettings::default(),
            ProjectType::Api,
            None,
            Some(&handle),
        );

        assert_eq!(context.knowledge_status, SkillKnowledgeStatus::Ready);
        assert_eq!(context.suggested_skills.len(), 1);
        assert!(backend
            .requested_top_ks
            .lock()
            .unwrap()
            .iter()
            .any(|k| *k >= 64));
    }

    #[test]
    fn skill_instruction_context_expands_openai_docs_queries_during_triage() {
        let handle = SkillKnowledgeHandle::pending();
        let backend = Arc::new(QueryAwareBackend {
            requested_queries: Mutex::new(Vec::new()),
            matcher: Arc::new(|query| {
                if query.contains("openai-docs") || query.contains("official documentation") {
                    Some(SkillMatch {
                        skill_id: "openai-docs".to_string(),
                        name: "OpenAI Docs".to_string(),
                        description: "Use official OpenAI docs".to_string(),
                        score: 0.71,
                        source_path: PathBuf::from("/tmp/openai-docs/SKILL.md"),
                        origin: "community-openai".to_string(),
                    })
                } else {
                    None
                }
            }),
        });
        handle.set_ready_backend(backend.clone());

        let task = Task {
            title: "Research OpenAI Responses API docs for tool calling".to_string(),
            description: Some(
                "Need official OpenAI documentation, citations, Responses API, Chat Completions, Agents SDK, model limits."
                    .to_string(),
            ),
            ..sample_task()
        };

        let context = build_skill_instruction_context(
            &task,
            &ProjectSettings::default(),
            ProjectType::Api,
            None,
            Some(&handle),
        );

        assert_eq!(context.knowledge_status, SkillKnowledgeStatus::Ready);
        assert_eq!(context.suggested_skills.len(), 1);
        assert!(backend
            .requested_queries
            .lock()
            .unwrap()
            .iter()
            .any(|query| query.contains("openai-docs")));
    }

    #[test]
    fn build_skill_search_queries_use_repo_signals_for_openai_docs() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_dir.path().join("package.json"),
            r#"{"dependencies":{"openai":"^4.0.0"}}"#,
        )
        .unwrap();

        let task = Task {
            title: "Fix tool calling reliability".to_string(),
            description: Some("Need current API guidance and examples.".to_string()),
            ..sample_task()
        };

        let queries = build_skill_search_queries(&task, Some(temp_dir.path()));

        assert!(queries.iter().any(|query| query.contains("openai-docs")));
    }

    #[test]
    fn loaded_skills_log_line_lists_active_and_suggested_skills() {
        let line = format_loaded_skills_log_line(&SkillInstructionContext {
            block: String::new(),
            resolved_skill_chain: vec![
                "task-preflight-check".to_string(),
                "code-implement".to_string(),
                "final-report".to_string(),
            ],
            loaded_active_skills: vec![
                "task-preflight-check".to_string(),
                "code-implement".to_string(),
            ],
            suggested_skills: vec![SuggestedSkill {
                skill_id: "openai-docs".to_string(),
                name: "OpenAI Docs".to_string(),
                description: "Use official docs".to_string(),
                score: 0.92,
                source_path: "/tmp/openai-docs/SKILL.md".to_string(),
                origin: "community-openai".to_string(),
            }],
            knowledge_status: SkillKnowledgeStatus::Ready,
            knowledge_detail: Some("Added 1 suggested skill.".to_string()),
            plan: SkillPlan::default(),
            render_budget: SkillRenderBudget::default(),
        });

        assert_eq!(
            line,
            "Loaded skills: task-preflight-check, code-implement; suggested: openai-docs"
        );
    }

    #[test]
    fn loaded_skills_log_line_includes_non_ready_status_detail() {
        let line = format_loaded_skills_log_line(&SkillInstructionContext {
            block: String::new(),
            resolved_skill_chain: vec!["task-preflight-check".to_string()],
            loaded_active_skills: vec!["task-preflight-check".to_string()],
            suggested_skills: Vec::new(),
            knowledge_status: SkillKnowledgeStatus::Pending,
            knowledge_detail: Some(
                "Global skill knowledge index is still building.\nThis should stay one line."
                    .to_string(),
            ),
            plan: SkillPlan::default(),
            render_budget: SkillRenderBudget::default(),
        });

        assert_eq!(
            line,
            "Loaded skills: task-preflight-check; knowledge: pending"
        );
    }

    #[test]
    fn render_suggested_skill_block_compacts_content_to_fit_remaining_budget() {
        let candidate = PlannedSkill {
            skill_id: "openai-docs".to_string(),
            name: "OpenAI Docs".to_string(),
            description: "Use official docs".to_string(),
            source_path: Some("/tmp/openai-docs/SKILL.md".to_string()),
            origin: Some("community-openai".to_string()),
            score: Some(0.81),
            phase: "knowledge_index".to_string(),
            proposed_by: "global_knowledge_index".to_string(),
            reason: "Matched triage query. ".repeat(24),
        };

        let (rendered, rendered_len) = render_suggested_skill_block(&candidate, true, 560).unwrap();

        assert!(rendered_len <= 560);
        assert!(rendered.contains("## Suggested Knowledge"));
        assert!(rendered.contains("Skill ID: `openai-docs`"));
        assert!(rendered.contains("Use `load_skill` if you need the full playbook"));
        assert!(rendered.contains("suggested skill summary truncated"));
        assert!(!rendered.contains("/tmp/openai-docs/SKILL.md"));
    }

    #[test]
    fn render_skill_instruction_context_reserves_suggested_budget_lane() {
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_skills_dir = temp_dir.path().join(".acpms").join("skills");
        std::fs::create_dir_all(&repo_skills_dir).unwrap();

        let mut required = Vec::new();
        for index in 0..30 {
            let skill_id = format!("req-skill-{index:02}");
            let skill_dir = repo_skills_dir.join(&skill_id);
            std::fs::create_dir_all(&skill_dir).unwrap();
            std::fs::write(
                skill_dir.join("SKILL.md"),
                format!(
                    "---\nname: {skill_id}\ndescription: required skill\n---\n{}",
                    "Follow this required playbook.\n".repeat(200)
                ),
            )
            .unwrap();
            required.push(PlannedSkill {
                skill_id: skill_id.clone(),
                name: skill_id.clone(),
                description: "Required skill".to_string(),
                source_path: None,
                origin: Some("deterministic".to_string()),
                score: None,
                phase: "deterministic_chain".to_string(),
                proposed_by: "deterministic_chain".to_string(),
                reason: "Selected by deterministic task policy for this attempt.".to_string(),
            });
        }

        let suggested_dir = temp_dir.path().join("suggested").join("openai-docs");
        std::fs::create_dir_all(&suggested_dir).unwrap();
        let suggested_path = suggested_dir.join("SKILL.md");
        std::fs::write(
            &suggested_path,
            format!(
                "---\nname: openai-docs\ndescription: suggested skill\n---\n{}",
                "Use official OpenAI documentation.\n".repeat(120)
            ),
        )
        .unwrap();

        let plan = SkillPlan {
            required,
            suggested: vec![PlannedSkill {
                skill_id: "openai-docs".to_string(),
                name: "OpenAI Docs".to_string(),
                description: "Use official OpenAI docs".to_string(),
                source_path: Some(suggested_path.to_string_lossy().to_string()),
                origin: Some("community-openai".to_string()),
                score: Some(0.88),
                phase: "knowledge_index".to_string(),
                proposed_by: "global_knowledge_index".to_string(),
                reason: "Matched task and repository triage queries.".to_string(),
            }],
            skipped: Vec::new(),
            trace: Vec::new(),
            knowledge_status: SkillKnowledgeStatus::Ready,
            knowledge_detail: Some("Planned suggested knowledge.".to_string()),
        };

        let context = render_skill_instruction_context(&plan, Some(temp_dir.path()));

        assert_eq!(context.suggested_skills.len(), 1);
        assert!(context.block.contains("## Suggested Knowledge"));
        assert!(context.loaded_active_skills.len() < plan.required.len());
        assert!(context.render_budget.required_used <= REQUIRED_SKILL_BUDGET);
        assert!(context.render_budget.suggested_used <= SUGGESTED_SKILL_BUDGET);
        assert_eq!(context.render_budget.runtime_reserve, RUNTIME_SKILL_RESERVE);
    }

    #[test]
    fn detect_skill_file_extracts_skill_id_and_origin() {
        let path = PathBuf::from("/Users/test/.acpms/skills/community-openai/openai-docs/SKILL.md");

        let detected = detect_skill_file(&path).expect("skill file should be detected");

        assert_eq!(detected.skill_id, "openai-docs");
        assert_eq!(detected.origin, "community-openai");
        assert_eq!(detected.source_path, path.to_string_lossy());
    }

    #[test]
    fn detect_skill_file_resolves_symlinked_overlay_paths_to_real_source() {
        let temp_dir = tempfile::tempdir().unwrap();
        let platform_dir = temp_dir.path().join("platform-skills");
        let community_skill_dir = platform_dir.join("community-openai").join("openai-docs");
        let overlay_dir = temp_dir
            .path()
            .join("overlay")
            .join("skills")
            .join("openai-docs");

        std::fs::create_dir_all(&community_skill_dir).unwrap();
        std::fs::create_dir_all(overlay_dir.parent().unwrap()).unwrap();
        std::fs::write(
            community_skill_dir.join("SKILL.md"),
            "---\nname: openai-docs\ndescription: repo managed copy\n---\ncommunity copy",
        )
        .unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(&community_skill_dir, &overlay_dir).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&community_skill_dir, &overlay_dir).unwrap();

        let original_skills_dir = std::env::var("ACPMS_SKILLS_DIR").ok();
        std::env::set_var("ACPMS_SKILLS_DIR", &platform_dir);

        let detected =
            detect_skill_file(&overlay_dir.join("SKILL.md")).expect("skill file should resolve");

        if let Some(original) = original_skills_dir {
            std::env::set_var("ACPMS_SKILLS_DIR", original);
        } else {
            std::env::remove_var("ACPMS_SKILLS_DIR");
        }

        assert_eq!(detected.skill_id, "openai-docs");
        assert_eq!(detected.origin, "community-openai");
        assert_eq!(
            detected.source_path,
            std::fs::canonicalize(community_skill_dir)
                .unwrap()
                .join("SKILL.md")
                .to_string_lossy()
                .to_string()
        );
    }

    #[test]
    fn runtime_skill_attachment_prefers_repo_managed_duplicate_over_codex_home() {
        let temp_dir = tempfile::tempdir().unwrap();
        let platform_dir = temp_dir.path().join("platform-skills");
        let community_dir = platform_dir.join("community-openai");
        let codex_home_dir = temp_dir.path().join("codex-home").join("skills");

        let community_skill_dir = community_dir.join("openai-docs");
        let codex_skill_dir = codex_home_dir.join("openai-docs");

        std::fs::create_dir_all(&community_skill_dir).unwrap();
        std::fs::create_dir_all(&codex_skill_dir).unwrap();

        std::fs::write(
            community_skill_dir.join("SKILL.md"),
            "---\nname: openai-docs\ndescription: bundled copy\n---\ncommunity copy",
        )
        .unwrap();
        std::fs::write(
            codex_skill_dir.join("SKILL.md"),
            "---\nname: openai-docs\ndescription: user-managed copy\n---\ncodex home copy",
        )
        .unwrap();

        let roots = candidate_skill_roots_from_globals(
            None,
            vec![
                crate::knowledge_index::KnowledgeRoot {
                    path: platform_dir,
                    origin: "platform".to_string(),
                },
                crate::knowledge_index::KnowledgeRoot {
                    path: community_dir,
                    origin: "community-openai".to_string(),
                },
                crate::knowledge_index::KnowledgeRoot {
                    path: codex_home_dir,
                    origin: "codex-home".to_string(),
                },
            ],
        );

        let skill = load_skill_from_roots("openai-docs", &roots).expect("skill should load");

        assert_eq!(skill.origin.as_deref(), Some("community-openai"));
        assert!(skill.content.contains("community copy"));
        assert!(skill
            .source
            .as_ref()
            .expect("source path should exist")
            .ends_with(Path::new(
                "platform-skills/community-openai/openai-docs/SKILL.md"
            )));
    }
}
