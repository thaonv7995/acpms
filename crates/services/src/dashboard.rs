use acpms_db::{models::*, PgPool};
use acpms_executors;
use anyhow::Result;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use serde::Serialize;
use sqlx::FromRow;
use std::collections::HashMap;
use std::sync::Arc;
use ts_rs::TS;
use uuid::Uuid;

// --- Internal Query Result Structs ---
#[derive(FromRow)]
struct ProjectQueryResult {
    id: Uuid,
    name: String,
    description: Option<String>,
    #[allow(dead_code)]
    created_at: DateTime<Utc>,
    #[allow(dead_code)]
    updated_at: DateTime<Utc>,
    progress: Option<i64>,
    latest_attempt_status: Option<String>,
}

#[derive(FromRow)]
struct TaskQueryResult {
    id: Uuid,
    project_id: Uuid,
    project_name: String,
    title: String,
    description: Option<String>,
    task_type: TaskType,
    task_status: TaskStatus,
    created_at: DateTime<Utc>,
    assigned_to: Option<Uuid>,
}

#[derive(FromRow)]
struct ProjectAgentRow {
    project_id: Uuid,
    user_id: Uuid,
    user_name: String,
    rank_in_project: i64,
}

// --- Response Structs (matching Frontend types) ---

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DashboardStats {
    #[serde(rename = "activeProjects")]
    pub active_projects: StatsMetric,
    #[serde(rename = "agentsOnline")]
    pub agents_online: AgentStats,
    #[serde(rename = "systemLoad")]
    pub system_load: SystemLoad,
    #[serde(rename = "pendingPRs")]
    pub pending_prs: PrStats,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StatsMetric {
    pub count: i64,
    pub trend: String,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentStats {
    pub online: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemLoad {
    pub percentage: i64,
    pub status: String, // 'low' | 'medium' | 'high'
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrStats {
    pub count: i64,
    pub requires_review: bool,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DashboardProject {
    pub id: Uuid,
    pub name: String,
    pub subtitle: String,
    pub status: String, // 'building' | 'testing' | 'deploying' | 'completed'
    pub progress: i64,
    pub agents: Vec<AgentAvatar>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentAvatar {
    pub id: String,
    pub initial: String,
    pub color: String,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DashboardAgentLog {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "agentName")]
    pub agent_name: String,
    #[serde(rename = "agentColor")]
    pub agent_color: String,
    pub message: String,
    pub highlight: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DashboardHumanTask {
    pub id: Uuid,
    #[serde(rename = "projectId")]
    pub project_id: Uuid,
    #[serde(rename = "projectName")]
    pub project_name: String,
    #[serde(rename = "type")]
    pub type_: String, // 'blocker' | 'approval' | 'qa' | 'review'
    pub title: String,
    pub description: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    pub assignee: Option<UserAvatar>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserAvatar {
    pub id: Uuid,
    pub avatar: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DashboardData {
    pub stats: DashboardStats,
    pub projects: Vec<DashboardProject>,
    #[serde(rename = "agentLogs")]
    pub agent_logs: Vec<DashboardAgentLog>,
    #[serde(rename = "humanTasks")]
    pub human_tasks: Vec<DashboardHumanTask>,
}

// --- Service ---

pub struct DashboardService {
    pool: PgPool,
}

impl DashboardService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn is_system_admin(&self, user_id: Uuid) -> bool {
        sqlx::query_scalar::<_, bool>(
            r#"
            SELECT COALESCE('admin'::system_role = ANY(global_roles), false)
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
    }

    fn derive_project_status(latest_attempt_status: Option<&str>, progress: i64) -> String {
        match latest_attempt_status.map(|s| s.to_ascii_lowercase()) {
            Some(status) if status == "running" || status == "queued" => "building".to_string(),
            Some(status) if status == "failed" || status == "cancelled" => "testing".to_string(),
            Some(status) if status == "success" && progress >= 100 => "completed".to_string(),
            Some(status) if status == "success" => "deploying".to_string(),
            _ if progress >= 100 => "completed".to_string(),
            _ => "building".to_string(),
        }
    }

    fn avatar_color_for_rank(rank_in_project: i64) -> &'static str {
        match (rank_in_project - 1).rem_euclid(5) {
            0 => "bg-blue-500",
            1 => "bg-emerald-500",
            2 => "bg-amber-500",
            3 => "bg-rose-500",
            _ => "bg-indigo-500",
        }
    }

    fn initial_from_name(name: &str) -> String {
        name.chars()
            .find(|c| c.is_alphanumeric())
            .map(|c| c.to_ascii_uppercase().to_string())
            .unwrap_or_else(|| "U".to_string())
    }

    async fn load_dashboard_agent_logs(
        &self,
        user_id: Uuid,
        is_admin: bool,
        storage: Option<Arc<crate::StorageService>>,
    ) -> Result<Vec<DashboardAgentLog>> {
        let attempts: Vec<(Uuid, Option<String>)> = sqlx::query_as(
            r#"
            SELECT ta.id, ta.s3_log_key
            FROM task_attempts ta
            JOIN tasks t ON t.id = ta.task_id
            WHERE $1 OR EXISTS (SELECT 1 FROM project_members pm WHERE pm.project_id = t.project_id AND pm.user_id = $2)
            ORDER BY ta.created_at DESC
            LIMIT 12
            "#,
        )
        .bind(is_admin)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        // Fetch log bytes in parallel (was sequential - major bottleneck)
        let fetch_futures: Vec<_> = attempts
            .into_iter()
            .map(|(attempt_id, s3_log_key)| {
                let storage = storage.clone();
                async move {
                    let bytes = if let Some(ref st) = storage {
                        if let Some(key) = s3_log_key {
                            st.get_log_bytes(&key).await.unwrap_or_default()
                        } else {
                            acpms_executors::read_attempt_log_file(attempt_id)
                                .await
                                .unwrap_or_default()
                        }
                    } else {
                        acpms_executors::read_attempt_log_file(attempt_id)
                            .await
                            .unwrap_or_default()
                    };
                    (attempt_id, bytes)
                }
            })
            .collect();

        let results = join_all(fetch_futures).await;

        let mut all_logs: Vec<(Uuid, String, DateTime<Utc>)> = Vec::new();
        for (_attempt_id, bytes) in results {
            let logs = acpms_executors::parse_jsonl_to_agent_logs(&bytes);
            for l in logs {
                if !matches!(
                    l.log_type.as_str(),
                    "normalized" | "stdout" | "stderr" | "user"
                ) {
                    continue;
                }
                if l.content.contains("codex_otel::traces::otel_manager")
                    || l.content.starts_with("DEBUG codex_exec: Received event:")
                {
                    continue;
                }
                all_logs.push((l.id, l.content, l.created_at));
            }
        }
        all_logs.sort_by(|a, b| b.2.cmp(&a.2));
        all_logs.truncate(10);

        Ok(all_logs
            .into_iter()
            .map(|(id, content, timestamp)| DashboardAgentLog {
                id,
                timestamp,
                agent_name: "Agent".to_string(),
                agent_color: "bg-blue-500".to_string(),
                message: content.chars().take(120).collect(),
                highlight: None,
            })
            .collect())
    }

    async fn load_stats(&self, is_admin: bool, user_id: Uuid) -> Result<DashboardStats> {
        let pool = self.pool.clone();
        let (
            active_projects_count,
            agents_online,
            queued_attempts,
            pending_prs,
            projects_last_week,
            agents_total,
        ) = tokio::join!(
            async {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT COUNT(*) FROM projects p WHERE $1 OR EXISTS (
                        SELECT 1 FROM project_members pm WHERE pm.project_id = p.id AND pm.user_id = $2
                    )"#,
                )
                .bind(is_admin)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap_or(0)
            },
            async {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT COUNT(*) FROM task_attempts ta JOIN tasks t ON t.id = ta.task_id
                    WHERE ta.status = 'running' AND ($1 OR EXISTS (
                        SELECT 1 FROM project_members pm WHERE pm.project_id = t.project_id AND pm.user_id = $2
                    ))"#,
                )
                .bind(is_admin)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap_or(0)
            },
            async {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT COUNT(*) FROM task_attempts ta JOIN tasks t ON t.id = ta.task_id
                    WHERE ta.status = 'queued' AND ($1 OR EXISTS (
                        SELECT 1 FROM project_members pm WHERE pm.project_id = t.project_id AND pm.user_id = $2
                    ))"#,
                )
                .bind(is_admin)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap_or(0)
            },
            async {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT COUNT(*) FROM tasks t WHERE t.status = 'in_review' AND ($1 OR EXISTS (
                        SELECT 1 FROM project_members pm WHERE pm.project_id = t.project_id AND pm.user_id = $2
                    ))"#,
                )
                .bind(is_admin)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap_or(0)
            },
            async {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT COUNT(*) FROM projects p WHERE p.created_at > NOW() - INTERVAL '7 days'
                    AND ($1 OR EXISTS (
                        SELECT 1 FROM project_members pm WHERE pm.project_id = p.id AND pm.user_id = $2
                    ))"#,
                )
                .bind(is_admin)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap_or(0)
            },
            async {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT COUNT(DISTINCT pm.user_id) FROM project_members pm
                    WHERE $1 OR EXISTS (
                        SELECT 1 FROM project_members my_pm WHERE my_pm.project_id = pm.project_id AND my_pm.user_id = $2
                    )"#,
                )
                .bind(is_admin)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap_or(0)
            },
        );

        let agent_capacity = agents_total.max(1);
        let system_load_percentage =
            (((agents_online + queued_attempts) * 100) / agent_capacity).clamp(0, 100);
        let system_load_status = if system_load_percentage >= 75 {
            "high"
        } else if system_load_percentage >= 40 {
            "medium"
        } else {
            "low"
        };

        Ok(DashboardStats {
            active_projects: StatsMetric {
                count: active_projects_count,
                trend: if projects_last_week > 0 {
                    format!("+{} this week", projects_last_week)
                } else {
                    "No changes".to_string()
                },
            },
            agents_online: AgentStats {
                online: agents_online,
                total: agents_total.max(agents_online),
            },
            system_load: SystemLoad {
                percentage: system_load_percentage,
                status: system_load_status.to_string(),
            },
            pending_prs: PrStats {
                count: pending_prs,
                requires_review: pending_prs > 0,
            },
        })
    }

    async fn load_projects(&self, is_admin: bool, user_id: Uuid) -> Result<Vec<DashboardProject>> {
        // Return top 5 most recently created/updated with calculated progress
        let projects_raw = sqlx::query_as::<_, ProjectQueryResult>(
            r#"
            WITH accessible_projects AS (
                SELECT p.id, p.name, p.description, p.created_at, p.updated_at
                FROM projects p
                WHERE $1
                   OR EXISTS (
                        SELECT 1
                        FROM project_members pm
                        WHERE pm.project_id = p.id
                          AND pm.user_id = $2
                    )
            ),
            task_stats AS (
                SELECT 
                    t.project_id,
                    COUNT(*) as total,
                    COUNT(*) FILTER (
                        WHERE LOWER(t.status::text) IN ('done', 'archived', 'cancelled', 'canceled')
                    ) as completed
                FROM tasks t
                JOIN accessible_projects ap ON ap.id = t.project_id
                WHERE t.sprint_id IS NOT NULL
                GROUP BY t.project_id
            )
            SELECT 
                ap.id, 
                ap.name, 
                ap.description, 
                ap.created_at, 
                ap.updated_at,
                CASE
                    WHEN ts.total IS NULL OR ts.total = 0 THEN 0
                    ELSE ROUND((ts.completed::numeric / ts.total::numeric) * 100)::bigint
                END as progress,
                (
                    SELECT ta.status::text
                    FROM task_attempts ta
                    JOIN tasks t2 ON t2.id = ta.task_id
                    WHERE t2.project_id = ap.id
                    ORDER BY COALESCE(ta.started_at, ta.created_at) DESC, ta.id DESC
                    LIMIT 1
                ) as latest_attempt_status
            FROM accessible_projects ap
            LEFT JOIN task_stats ts ON ts.project_id = ap.id
            ORDER BY ap.updated_at DESC
            LIMIT 5
            "#,
        )
        .bind(is_admin)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let project_ids: Vec<Uuid> = projects_raw.iter().map(|p| p.id).collect();
        let mut agents_by_project: HashMap<Uuid, Vec<AgentAvatar>> = HashMap::new();

        if !project_ids.is_empty() {
            let project_agents = sqlx::query_as::<_, ProjectAgentRow>(
                r#"
                WITH ranked_members AS (
                    SELECT
                        pm.project_id,
                        pm.user_id,
                        COALESCE(u.name, u.email, 'User') AS user_name,
                        ROW_NUMBER() OVER (
                            PARTITION BY pm.project_id
                            ORDER BY pm.created_at ASC, pm.user_id ASC
                        ) AS rank_in_project
                    FROM project_members pm
                    JOIN users u ON u.id = pm.user_id
                    WHERE pm.project_id = ANY($1)
                )
                SELECT project_id, user_id, user_name, rank_in_project
                FROM ranked_members
                WHERE rank_in_project <= 3
                ORDER BY project_id ASC, rank_in_project ASC
                "#,
            )
            .bind(&project_ids)
            .fetch_all(&self.pool)
            .await?;

            for row in project_agents {
                agents_by_project
                    .entry(row.project_id)
                    .or_default()
                    .push(AgentAvatar {
                        id: row.user_id.to_string(),
                        initial: Self::initial_from_name(&row.user_name),
                        color: Self::avatar_color_for_rank(row.rank_in_project).to_string(),
                    });
            }
        }

        let projects: Vec<DashboardProject> = projects_raw
            .into_iter()
            .map(|p| {
                let progress = p.progress.unwrap_or(0).clamp(0, 100);
                DashboardProject {
                    id: p.id,
                    name: p.name,
                    subtitle: p.description.unwrap_or_default(),
                    status: Self::derive_project_status(
                        p.latest_attempt_status.as_deref(),
                        progress,
                    ),
                    progress,
                    agents: agents_by_project.remove(&p.id).unwrap_or_default(),
                }
            })
            .collect();

        Ok(projects)
    }

    async fn load_human_tasks(
        &self,
        is_admin: bool,
        user_id: Uuid,
    ) -> Result<Vec<DashboardHumanTask>> {
        let tasks_raw = sqlx::query_as::<_, TaskQueryResult>(
            r#"
            SELECT
                t.id,
                t.project_id,
                p.name as project_name,
                t.title,
                t.description,
                t.task_type,
                t.status as task_status,
                t.created_at,
                t.assigned_to
            FROM tasks t
            JOIN projects p ON p.id = t.project_id
            WHERE t.status::text IN ('todo', 'in_review')
              AND t.created_at >= NOW() - INTERVAL '30 days'
              AND NOT (
                  t.title ILIKE '[breakdown]%'
                  OR COALESCE(t.metadata->>'breakdown_mode', '') = 'ai_support'
                  OR COALESCE(t.metadata->>'breakdown_kind', '') = 'analysis_session'
              )
              AND (t.assigned_to = $3 OR t.assigned_to IS NULL)
              AND (
                    $1
                    OR EXISTS (
                        SELECT 1
                        FROM project_members pm
                        WHERE pm.project_id = t.project_id
                          AND pm.user_id = $2
                    )
                )
            ORDER BY 
                CASE WHEN t.status::text = 'in_review' THEN 0 ELSE 1 END ASC,
                t.created_at DESC
            LIMIT 5
            "#,
        )
        .bind(is_admin)
        .bind(user_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(tasks_raw
            .into_iter()
            .map(|t| DashboardHumanTask {
                id: t.id,
                project_id: t.project_id,
                project_name: t.project_name,
                type_: if matches!(t.task_status, TaskStatus::InReview) {
                    "review".to_string()
                } else {
                    match t.task_type {
                        TaskType::Bug | TaskType::Hotfix => "blocker".to_string(),
                        TaskType::Feature => "feature".to_string(),
                        TaskType::Refactor => "refactor".to_string(),
                        TaskType::Docs => "docs".to_string(),
                        TaskType::Test => "test".to_string(),
                        TaskType::Init => "init".to_string(),
                        TaskType::Chore => "chore".to_string(),
                        TaskType::Spike => "spike".to_string(),
                        TaskType::SmallTask => "small_task".to_string(),
                        TaskType::Deploy => "deploy".to_string(),
                    }
                },
                title: t.title,
                description: t.description.unwrap_or_default(),
                created_at: t.created_at,
                assignee: t.assigned_to.map(|uid| UserAvatar {
                    id: uid,
                    avatar: None,
                }),
            })
            .collect())
    }

    pub async fn get_dashboard_data(
        &self,
        user_id: Uuid,
        storage: Option<Arc<crate::StorageService>>,
    ) -> Result<DashboardData> {
        let is_admin = self.is_system_admin(user_id).await;

        // Run stats, projects, human_tasks, agent_logs in parallel
        let (stats_res, projects_res, human_tasks_res, agent_logs_res) = tokio::join!(
            self.load_stats(is_admin, user_id),
            self.load_projects(is_admin, user_id),
            self.load_human_tasks(is_admin, user_id),
            self.load_dashboard_agent_logs(user_id, is_admin, storage),
        );

        let stats = stats_res?;
        let projects = projects_res?;
        let human_tasks = human_tasks_res?;
        let agent_logs = agent_logs_res?;

        Ok(DashboardData {
            stats,
            projects,
            agent_logs,
            human_tasks,
        })
    }
}
