use acpms_db::models::AttemptStatus;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SubagentRelationship {
    pub id: Uuid,
    pub parent_attempt_id: Uuid,
    pub child_attempt_id: Uuid,
    pub spawned_at: DateTime<Utc>,
    pub spawn_tool_use_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SubagentTreeNode {
    pub attempt_id: Uuid,
    pub status: AttemptStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub depth: i32,
    #[sqlx(skip)]
    pub children: Vec<SubagentTreeNode>,
}

pub struct SubagentService {
    pool: PgPool,
}

impl SubagentService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Store a parent-child relationship
    pub async fn store_relationship(
        &self,
        parent_attempt_id: Uuid,
        child_attempt_id: Uuid,
        spawn_tool_use_id: Option<String>,
    ) -> Result<Uuid> {
        let id = sqlx::query_scalar(
            r#"
            INSERT INTO subagent_relationships (parent_attempt_id, child_attempt_id, spawn_tool_use_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (parent_attempt_id, child_attempt_id) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(parent_attempt_id)
        .bind(child_attempt_id)
        .bind(spawn_tool_use_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Get hierarchical tree of subagents (max depth 3)
    pub async fn get_subagent_tree(
        &self,
        parent_attempt_id: Uuid,
    ) -> Result<Vec<SubagentTreeNode>> {
        let rows = sqlx::query_as::<_, SubagentTreeNode>(
            r#"
            WITH RECURSIVE subagent_tree AS (
                -- Base case: direct children
                SELECT
                    sr.child_attempt_id as attempt_id,
                    ta.status as "status",
                    ta.started_at,
                    ta.completed_at,
                    1 as depth,
                    sr.parent_attempt_id
                FROM subagent_relationships sr
                JOIN task_attempts ta ON sr.child_attempt_id = ta.id
                WHERE sr.parent_attempt_id = $1

                UNION ALL

                -- Recursive case: children of children
                SELECT
                    sr.child_attempt_id as attempt_id,
                    ta.status as "status",
                    ta.started_at,
                    ta.completed_at,
                    st.depth + 1 as depth,
                    sr.parent_attempt_id
                FROM subagent_relationships sr
                JOIN task_attempts ta ON sr.child_attempt_id = ta.id
                JOIN subagent_tree st ON sr.parent_attempt_id = st.attempt_id
                WHERE st.depth < 3  -- Max depth limit
            )
            SELECT * FROM subagent_tree
            ORDER BY depth, started_at
            "#,
        )
        .bind(parent_attempt_id)
        .fetch_all(&self.pool)
        .await?;

        // Build tree structure from flat rows
        let mut nodes: Vec<SubagentTreeNode> = rows;

        // Build hierarchy (simple approach - can be optimized)
        Self::build_tree_hierarchy(&mut nodes);

        Ok(nodes)
    }

    fn build_tree_hierarchy(_nodes: &mut Vec<SubagentTreeNode>) {
        // TODO: Implement tree building logic
        // For now, return flat list (frontend can handle hierarchy)
    }

    /// Get all attempt IDs in a subagent tree (parent + all descendants)
    pub async fn get_all_attempt_ids(&self, parent_attempt_id: Uuid) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            WITH RECURSIVE subagent_tree AS (
                SELECT $1::uuid as attempt_id

                UNION

                SELECT sr.child_attempt_id
                FROM subagent_relationships sr
                JOIN subagent_tree st ON sr.parent_attempt_id = st.attempt_id
            )
            SELECT attempt_id FROM subagent_tree
            "#,
        )
        .bind(parent_attempt_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get direct children of a parent attempt
    pub async fn get_direct_children(
        &self,
        parent_attempt_id: Uuid,
    ) -> Result<Vec<SubagentRelationship>> {
        let rows = sqlx::query_as::<_, SubagentRelationship>(
            r#"
            SELECT id, parent_attempt_id, child_attempt_id, spawned_at, spawn_tool_use_id
            FROM subagent_relationships
            WHERE parent_attempt_id = $1
            ORDER BY spawned_at
            "#,
        )
        .bind(parent_attempt_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| SubagentRelationship {
                id: row.id,
                parent_attempt_id: row.parent_attempt_id,
                child_attempt_id: row.child_attempt_id,
                spawned_at: row.spawned_at,
                spawn_tool_use_id: row.spawn_tool_use_id,
            })
            .collect())
    }

    /// Get parent of a child attempt (if any)
    pub async fn get_parent(&self, child_attempt_id: Uuid) -> Result<Option<SubagentRelationship>> {
        let row = sqlx::query_as::<_, SubagentRelationship>(
            r#"
            SELECT id, parent_attempt_id, child_attempt_id, spawned_at, spawn_tool_use_id
            FROM subagent_relationships
            WHERE child_attempt_id = $1
            LIMIT 1
            "#,
        )
        .bind(child_attempt_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| SubagentRelationship {
            id: row.id,
            parent_attempt_id: row.parent_attempt_id,
            child_attempt_id: row.child_attempt_id,
            spawned_at: row.spawned_at,
            spawn_tool_use_id: row.spawn_tool_use_id,
        }))
    }

    /// Check if an attempt has any subagents
    pub async fn has_subagents(&self, parent_attempt_id: Uuid) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) as "count!"
            FROM subagent_relationships
            WHERE parent_attempt_id = $1
            "#,
        )
        .bind(parent_attempt_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count > 0)
    }

    /// Get statistics about subagent tree
    pub async fn get_tree_stats(&self, parent_attempt_id: Uuid) -> Result<SubagentTreeStats> {
        let row = sqlx::query_as::<_, SubagentTreeStats>(
            r#"
            WITH RECURSIVE subagent_tree AS (
                SELECT $1::uuid as attempt_id, 0 as depth

                UNION

                SELECT sr.child_attempt_id, st.depth + 1
                FROM subagent_relationships sr
                JOIN subagent_tree st ON sr.parent_attempt_id = st.attempt_id
            )
            SELECT
                COUNT(*) as "total_subagents",
                MAX(depth) as "max_depth"
            FROM subagent_tree
            WHERE depth > 0
            "#,
        )
        .bind(parent_attempt_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SubagentTreeStats {
    pub total_subagents: i64,
    pub max_depth: i32,
}
