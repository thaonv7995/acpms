use acpms_db::models::{
    CloseSprintRequest, CloseSprintResult, CreateSprintRequest, GenerateSprintsRequest, Sprint,
    SprintCarryOverMode, SprintOverview, SprintStatus, UpdateSprintRequest,
};
use anyhow::{bail, Context, Result};
use chrono::Duration;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct SprintService {
    pool: PgPool,
}

impl SprintService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn sprint_columns() -> &'static str {
        "id, project_id, sequence, name, description, goal, status, start_date, end_date, closed_at, closed_by, created_at, updated_at"
    }

    async fn next_sequence_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        project_id: Uuid,
    ) -> Result<i32> {
        let next: i32 = sqlx::query_scalar(
            r#"
            SELECT COALESCE(MAX(sequence), 0) + 1
            FROM sprints
            WHERE project_id = $1
            "#,
        )
        .bind(project_id)
        .fetch_one(&mut **tx)
        .await
        .context("Failed to compute next sprint sequence")?;

        Ok(next)
    }

    async fn get_project_sprint_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        project_id: Uuid,
        sprint_id: Uuid,
    ) -> Result<Sprint> {
        let query = format!(
            "SELECT {} FROM sprints WHERE id = $1 AND project_id = $2 FOR UPDATE",
            Self::sprint_columns()
        );
        let sprint = sqlx::query_as::<_, Sprint>(&query)
            .bind(sprint_id)
            .bind(project_id)
            .fetch_optional(&mut **tx)
            .await
            .context("Failed to fetch sprint")?
            .ok_or_else(|| anyhow::anyhow!("Sprint not found in this project"))?;

        Ok(sprint)
    }

    pub async fn create_sprint(&self, req: CreateSprintRequest) -> Result<Sprint> {
        let mut tx = self.pool.begin().await.context("Failed to start tx")?;

        let sequence = if let Some(sequence) = req.sequence {
            if sequence <= 0 {
                bail!("Sprint sequence must be greater than 0");
            }
            sequence
        } else {
            Self::next_sequence_tx(&mut tx, req.project_id).await?
        };

        let query = format!(
            r#"
            INSERT INTO sprints (project_id, sequence, name, description, goal, start_date, end_date, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, 'planning')
            RETURNING {}
            "#,
            Self::sprint_columns()
        );

        let sprint = sqlx::query_as::<_, Sprint>(&query)
            .bind(req.project_id)
            .bind(sequence)
            .bind(&req.name)
            .bind(&req.description)
            .bind(&req.goal)
            .bind(req.start_date)
            .bind(req.end_date)
            .fetch_one(&mut *tx)
            .await
            .context("Failed to create sprint")?;

        tx.commit().await.context("Failed to commit tx")?;
        Ok(sprint)
    }

    pub async fn generate_sprints(&self, req: GenerateSprintsRequest) -> Result<Vec<Sprint>> {
        let mut tx = self.pool.begin().await.context("Failed to start tx")?;

        let mut sprints = Vec::new();
        let mut current_start_date = req.start_date;
        let mut sequence = Self::next_sequence_tx(&mut tx, req.project_id).await?;

        for _ in 0..req.count {
            let end_date = current_start_date + Duration::weeks(req.duration_weeks as i64);
            let name = format!("Sprint {}", sequence);

            let query = format!(
                r#"
                INSERT INTO sprints (project_id, sequence, name, start_date, end_date, description, goal, status)
                VALUES ($1, $2, $3, $4, $5, NULL, NULL, 'planning')
                RETURNING {}
                "#,
                Self::sprint_columns()
            );

            let sprint = sqlx::query_as::<_, Sprint>(&query)
                .bind(req.project_id)
                .bind(sequence)
                .bind(&name)
                .bind(current_start_date)
                .bind(end_date)
                .fetch_one(&mut *tx)
                .await
                .context("Failed to generate sprint")?;

            sprints.push(sprint);
            current_start_date = end_date;
            sequence += 1;
        }

        tx.commit().await.context("Failed to commit tx")?;
        Ok(sprints)
    }

    pub async fn list_project_sprints(&self, project_id: Uuid) -> Result<Vec<Sprint>> {
        let query = format!(
            "SELECT {} FROM sprints WHERE project_id = $1 ORDER BY sequence ASC, created_at ASC",
            Self::sprint_columns()
        );
        let sprints = sqlx::query_as::<_, Sprint>(&query)
            .bind(project_id)
            .fetch_all(&self.pool)
            .await
            .context("Failed to list project sprints")?;

        Ok(sprints)
    }

    /// Get a single sprint by ID
    pub async fn get_sprint(&self, sprint_id: Uuid) -> Result<Option<Sprint>> {
        let query = format!(
            "SELECT {} FROM sprints WHERE id = $1",
            Self::sprint_columns()
        );
        let sprint = sqlx::query_as::<_, Sprint>(&query)
            .bind(sprint_id)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to get sprint")?;

        Ok(sprint)
    }

    /// Update a sprint
    pub async fn update_sprint(&self, sprint_id: Uuid, req: UpdateSprintRequest) -> Result<Sprint> {
        let mut tx = self.pool.begin().await.context("Failed to start tx")?;

        let existing = sqlx::query_as::<_, Sprint>(&format!(
            "SELECT {} FROM sprints WHERE id = $1 FOR UPDATE",
            Self::sprint_columns()
        ))
        .bind(sprint_id)
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to fetch sprint")?
        .ok_or_else(|| anyhow::anyhow!("Sprint not found"))?;

        if matches!(req.status, Some(SprintStatus::Active))
            && existing.status != SprintStatus::Active
        {
            // Activate via dedicated flow to enforce single-active constraint cleanly.
            let _ = self
                .activate_sprint_tx(&mut tx, existing.project_id, sprint_id, None)
                .await?;
        }

        let query = format!(
            r#"
            UPDATE sprints
            SET name = COALESCE($2, name),
                description = COALESCE($3, description),
                goal = COALESCE($4, goal),
                status = COALESCE($5, status),
                start_date = COALESCE($6, start_date),
                end_date = COALESCE($7, end_date),
                closed_at = CASE
                    WHEN $5::sprint_status = 'completed' THEN COALESCE(closed_at, NOW())
                    WHEN $5::sprint_status IN ('planning', 'active') THEN NULL
                    ELSE closed_at
                END,
                closed_by = CASE
                    WHEN $5::sprint_status IN ('planning', 'active') THEN NULL
                    ELSE closed_by
                END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING {}
            "#,
            Self::sprint_columns()
        );

        let sprint = sqlx::query_as::<_, Sprint>(&query)
            .bind(sprint_id)
            .bind(&req.name)
            .bind(&req.description)
            .bind(&req.goal)
            .bind(req.status)
            .bind(req.start_date)
            .bind(req.end_date)
            .fetch_one(&mut *tx)
            .await
            .context("Failed to update sprint")?;

        tx.commit().await.context("Failed to commit tx")?;
        Ok(sprint)
    }

    /// Delete a sprint
    pub async fn delete_sprint(&self, sprint_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM sprints
            WHERE id = $1
            "#,
        )
        .bind(sprint_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete sprint")?;

        Ok(())
    }

    /// Get the active sprint for a project (status = 'active')
    pub async fn get_active_sprint(&self, project_id: Uuid) -> Result<Option<Sprint>> {
        let query = format!(
            "SELECT {} FROM sprints WHERE project_id = $1 AND status = 'active' ORDER BY sequence ASC LIMIT 1",
            Self::sprint_columns()
        );
        let sprint = sqlx::query_as::<_, Sprint>(&query)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to get active sprint")?;

        Ok(sprint)
    }

    /// Get the current sprint based on date range (now between start_date and end_date)
    pub async fn get_current_sprint(&self, project_id: Uuid) -> Result<Option<Sprint>> {
        let query = format!(
            r#"
            SELECT {}
            FROM sprints
            WHERE project_id = $1
              AND start_date <= NOW()
              AND (end_date IS NULL OR end_date >= NOW())
            ORDER BY start_date DESC
            LIMIT 1
            "#,
            Self::sprint_columns()
        );
        let sprint = sqlx::query_as::<_, Sprint>(&query)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to get current sprint")?;

        Ok(sprint)
    }

    pub async fn activate_sprint(
        &self,
        project_id: Uuid,
        sprint_id: Uuid,
        activated_by: Option<Uuid>,
    ) -> Result<Sprint> {
        let mut tx = self.pool.begin().await.context("Failed to start tx")?;
        let sprint = self
            .activate_sprint_tx(&mut tx, project_id, sprint_id, activated_by)
            .await?;
        tx.commit().await.context("Failed to commit tx")?;
        Ok(sprint)
    }

    async fn activate_sprint_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        project_id: Uuid,
        sprint_id: Uuid,
        activated_by: Option<Uuid>,
    ) -> Result<Sprint> {
        let target = Self::get_project_sprint_tx(tx, project_id, sprint_id).await?;

        if matches!(target.status, SprintStatus::Closed | SprintStatus::Archived) {
            bail!("Cannot activate a closed or archived sprint");
        }

        // Close any currently active sprint in this project before activating target sprint.
        sqlx::query(
            r#"
            UPDATE sprints
            SET status = 'completed',
                closed_at = NOW(),
                closed_by = COALESCE($2, closed_by),
                updated_at = NOW()
            WHERE project_id = $1
              AND status = 'active'
              AND id <> $3
            "#,
        )
        .bind(project_id)
        .bind(activated_by)
        .bind(sprint_id)
        .execute(&mut **tx)
        .await
        .context("Failed to close current active sprint")?;

        let query = format!(
            r#"
            UPDATE sprints
            SET status = 'active',
                closed_at = NULL,
                closed_by = NULL,
                updated_at = NOW()
            WHERE id = $1 AND project_id = $2
            RETURNING {}
            "#,
            Self::sprint_columns()
        );

        let activated = sqlx::query_as::<_, Sprint>(&query)
            .bind(sprint_id)
            .bind(project_id)
            .fetch_one(&mut **tx)
            .await
            .context("Failed to activate sprint")?;

        Ok(activated)
    }

    pub async fn close_sprint(
        &self,
        project_id: Uuid,
        sprint_id: Uuid,
        actor_id: Uuid,
        req: CloseSprintRequest,
    ) -> Result<CloseSprintResult> {
        let mut tx = self.pool.begin().await.context("Failed to start tx")?;

        let closing_sprint = Self::get_project_sprint_tx(&mut tx, project_id, sprint_id).await?;
        if closing_sprint.status != SprintStatus::Active {
            bail!("Only active sprints can be closed");
        }

        let mut moved_to_sprint_id: Option<Uuid> = None;

        if req.carry_over_mode == SprintCarryOverMode::MoveToNext {
            moved_to_sprint_id = Some(
                self.resolve_next_sprint_for_carry_over(&mut tx, project_id, &closing_sprint, &req)
                    .await?,
            );
        }

        let moved_task_count = self
            .carry_over_tasks(
                &mut tx,
                project_id,
                sprint_id,
                moved_to_sprint_id,
                actor_id,
                req.carry_over_mode,
                req.reason.as_deref(),
            )
            .await?;

        // Close sprint
        sqlx::query(
            r#"
            UPDATE sprints
            SET status = 'completed',
                closed_at = NOW(),
                closed_by = $3,
                updated_at = NOW()
            WHERE id = $1 AND project_id = $2
            "#,
        )
        .bind(sprint_id)
        .bind(project_id)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("Failed to close sprint")?;

        // Activate target sprint if requested.
        if let Some(target_sprint_id) = moved_to_sprint_id {
            self.activate_sprint_tx(&mut tx, project_id, target_sprint_id, Some(actor_id))
                .await?;
        }

        tx.commit().await.context("Failed to commit tx")?;

        Ok(CloseSprintResult {
            closed_sprint_id: sprint_id,
            moved_task_count,
            moved_to_sprint_id,
            carry_over_mode: req.carry_over_mode,
        })
    }

    async fn resolve_next_sprint_for_carry_over(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        project_id: Uuid,
        closing_sprint: &Sprint,
        req: &CloseSprintRequest,
    ) -> Result<Uuid> {
        if let Some(next_sprint_id) = req.next_sprint_id {
            let next = Self::get_project_sprint_tx(tx, project_id, next_sprint_id).await?;
            if matches!(next.status, SprintStatus::Closed | SprintStatus::Archived) {
                bail!("Cannot move tasks to a closed or archived sprint");
            }
            if next.id == closing_sprint.id {
                bail!("Cannot move tasks to the same sprint being closed");
            }
            return Ok(next.id);
        }

        if let Some(create_req) = &req.create_next_sprint {
            let sequence = Self::next_sequence_tx(tx, project_id).await?;
            let name = create_req
                .name
                .clone()
                .unwrap_or_else(|| format!("Sprint {}", sequence));

            let query = format!(
                r#"
                INSERT INTO sprints (project_id, sequence, name, description, goal, start_date, end_date, status)
                VALUES ($1, $2, $3, NULL, $4, $5, $6, 'planning')
                RETURNING {}
                "#,
                Self::sprint_columns()
            );

            let created = sqlx::query_as::<_, Sprint>(&query)
                .bind(project_id)
                .bind(sequence)
                .bind(&name)
                .bind(&create_req.goal)
                .bind(create_req.start_date)
                .bind(create_req.end_date)
                .fetch_one(&mut **tx)
                .await
                .context("Failed to create next sprint")?;

            return Ok(created.id);
        }

        // Auto-pick nearest planned sprint after current sequence.
        let next_existing = sqlx::query_as::<_, Sprint>(&format!(
            "SELECT {} FROM sprints WHERE project_id = $1 AND sequence > $2 AND status = 'planning' ORDER BY sequence ASC LIMIT 1 FOR UPDATE",
            Self::sprint_columns()
        ))
        .bind(project_id)
        .bind(closing_sprint.sequence)
        .fetch_optional(&mut **tx)
        .await
        .context("Failed to resolve next sprint")?;

        if let Some(next) = next_existing {
            return Ok(next.id);
        }

        // No planned sprint exists -> create one automatically.
        let sequence = Self::next_sequence_tx(tx, project_id).await?;
        let query = format!(
            r#"
            INSERT INTO sprints (project_id, sequence, name, description, goal, start_date, end_date, status)
            VALUES ($1, $2, $3, NULL, NULL, NULL, NULL, 'planning')
            RETURNING {}
            "#,
            Self::sprint_columns()
        );
        let created = sqlx::query_as::<_, Sprint>(&query)
            .bind(project_id)
            .bind(sequence)
            .bind(format!("Sprint {}", sequence))
            .fetch_one(&mut **tx)
            .await
            .context("Failed to auto-create next sprint")?;

        Ok(created.id)
    }

    async fn carry_over_tasks(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        project_id: Uuid,
        from_sprint_id: Uuid,
        to_sprint_id: Option<Uuid>,
        actor_id: Uuid,
        mode: SprintCarryOverMode,
        reason: Option<&str>,
    ) -> Result<i64> {
        if mode == SprintCarryOverMode::KeepInClosed {
            return Ok(0);
        }

        let moved_count: i64 = match mode {
            SprintCarryOverMode::MoveToNext => {
                let target_sprint = to_sprint_id.ok_or_else(|| anyhow::anyhow!("Missing target sprint"))?;
                sqlx::query_scalar(
                    r#"
                    WITH moved AS (
                        UPDATE tasks
                        SET sprint_id = $4,
                            updated_at = NOW()
                        WHERE project_id = $1
                          AND sprint_id = $2
                          AND LOWER(status::text) NOT IN ('done', 'archived', 'cancelled', 'canceled')
                        RETURNING id
                    ), inserted AS (
                        INSERT INTO sprint_task_movements (
                            project_id, task_id, from_sprint_id, to_sprint_id, moved_by, reason
                        )
                        SELECT $1, id, $2, $4, $3, $5
                        FROM moved
                        RETURNING 1
                    )
                    SELECT COUNT(*)::bigint FROM inserted
                    "#,
                )
                .bind(project_id)
                .bind(from_sprint_id)
                .bind(actor_id)
                .bind(target_sprint)
                .bind(reason)
                .fetch_one(&mut **tx)
                .await
                .context("Failed to move unfinished tasks to next sprint")?
            }
            SprintCarryOverMode::MoveToBacklog => {
                sqlx::query_scalar(
                    r#"
                    WITH moved AS (
                        UPDATE tasks
                        SET sprint_id = NULL,
                            updated_at = NOW()
                        WHERE project_id = $1
                          AND sprint_id = $2
                          AND LOWER(status::text) NOT IN ('done', 'archived', 'cancelled', 'canceled')
                        RETURNING id
                    ), inserted AS (
                        INSERT INTO sprint_task_movements (
                            project_id, task_id, from_sprint_id, to_sprint_id, moved_by, reason
                        )
                        SELECT $1, id, $2, NULL, $3, $4
                        FROM moved
                        RETURNING 1
                    )
                    SELECT COUNT(*)::bigint FROM inserted
                    "#,
                )
                .bind(project_id)
                .bind(from_sprint_id)
                .bind(actor_id)
                .bind(reason)
                .fetch_one(&mut **tx)
                .await
                .context("Failed to move unfinished tasks to backlog")?
            }
            SprintCarryOverMode::KeepInClosed => 0,
        };

        Ok(moved_count)
    }

    pub async fn get_sprint_overview(
        &self,
        project_id: Uuid,
        sprint_id: Uuid,
    ) -> Result<SprintOverview> {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM sprints WHERE id = $1 AND project_id = $2
            )
            "#,
        )
        .bind(sprint_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check sprint existence")?;

        if !exists {
            bail!("Sprint not found in this project");
        }

        let overview = sqlx::query_as::<_, SprintOverview>(
            r#"
            WITH task_stats AS (
                SELECT
                    COUNT(*)::bigint AS total_tasks,
                    COUNT(*) FILTER (WHERE LOWER(status::text) IN ('done', 'archived', 'cancelled', 'canceled'))::bigint AS done_tasks,
                    COUNT(*) FILTER (WHERE LOWER(status::text) IN ('cancelled', 'canceled'))::bigint AS canceled_tasks
                FROM tasks
                WHERE project_id = $1
                  AND sprint_id = $2
            ), movement_stats AS (
                SELECT
                    COUNT(*) FILTER (WHERE to_sprint_id = $2)::bigint AS moved_in_count,
                    COUNT(*) FILTER (WHERE from_sprint_id = $2)::bigint AS moved_out_count
                FROM sprint_task_movements
                WHERE project_id = $1
                  AND (from_sprint_id = $2 OR to_sprint_id = $2)
            )
            SELECT
                $2::uuid AS sprint_id,
                $1::uuid AS project_id,
                ts.total_tasks,
                ts.done_tasks,
                ts.canceled_tasks,
                GREATEST(ts.total_tasks - ts.done_tasks, 0)::bigint AS remaining_tasks,
                CASE
                    WHEN ts.total_tasks = 0 THEN 0
                    ELSE ROUND((ts.done_tasks::numeric / ts.total_tasks::numeric) * 100)::int
                END AS completion_rate,
                ms.moved_in_count,
                ms.moved_out_count
            FROM task_stats ts
            CROSS JOIN movement_stats ms
            "#,
        )
        .bind(project_id)
        .bind(sprint_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to build sprint overview")?;

        Ok(overview)
    }
}
