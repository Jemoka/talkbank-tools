//! Update and delete operations on the `jobs` and `file_statuses` tables.

use crate::scheduling::{AttemptOutcome, FailureCategory, RetryDisposition};

use crate::error::ServerError;

use super::JobDB;

impl JobDB {
    /// Update job-level status fields in the `jobs` table.
    ///
    /// Uses `COALESCE` so that `None` parameters leave the existing column
    /// value unchanged.  Called by the runner at each job state transition
    /// (Queued -> Running -> Completed/Failed/Cancelled).
    pub async fn update_job_status(
        &self,
        job_id: &str,
        status: &str,
        error: Option<&str>,
        completed_at: Option<f64>,
        num_workers: Option<i32>,
        next_eligible_at: Option<f64>,
    ) -> Result<(), ServerError> {
        sqlx::query(
            "UPDATE jobs
             SET status = ?,
                 error = COALESCE(?, error),
                 completed_at = COALESCE(?, completed_at),
                 num_workers = COALESCE(?, num_workers),
                 next_eligible_at = ?
             WHERE job_id = ?",
        )
        .bind(status)
        .bind(error)
        .bind(completed_at)
        .bind(num_workers)
        .bind(next_eligible_at)
        .bind(job_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update job-level lease ownership fields in the `jobs` table.
    pub async fn update_job_lease(
        &self,
        job_id: &str,
        leased_by_node: Option<&str>,
        lease_expires_at: Option<f64>,
        lease_heartbeat_at: Option<f64>,
    ) -> Result<(), ServerError> {
        sqlx::query(
            "UPDATE jobs
             SET leased_by_node = ?,
                 lease_expires_at = ?,
                 lease_heartbeat_at = ?
             WHERE job_id = ?",
        )
        .bind(leased_by_node)
        .bind(lease_expires_at)
        .bind(lease_heartbeat_at)
        .bind(job_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update a single row in the `file_statuses` table.
    ///
    /// Uses `COALESCE` so that `None` parameters preserve existing column
    /// values.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_file_status(
        &self,
        job_id: &str,
        filename: &str,
        status: &str,
        error: Option<&str>,
        error_category: Option<&str>,
        bug_report_id: Option<&str>,
        content_type: Option<&str>,
        started_at: Option<f64>,
        finished_at: Option<f64>,
        next_eligible_at: Option<f64>,
    ) -> Result<(), ServerError> {
        sqlx::query(
            "UPDATE file_statuses
             SET status = ?,
                 error = COALESCE(?, error),
                 error_category = COALESCE(?, error_category),
                 bug_report_id = COALESCE(?, bug_report_id),
                 content_type = COALESCE(?, content_type),
                 started_at = COALESCE(?, started_at),
                 finished_at = COALESCE(?, finished_at),
                 next_eligible_at = ?
             WHERE job_id = ? AND filename = ?",
        )
        .bind(status)
        .bind(error)
        .bind(error_category)
        .bind(bug_report_id)
        .bind(content_type)
        .bind(started_at)
        .bind(finished_at)
        .bind(next_eligible_at)
        .bind(job_id)
        .bind(filename)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Reset one recovered file back to a clean queued state.
    ///
    /// This is used during startup recovery after an interrupted job is
    /// reconciled back to `Queued`. Unlike [`Self::update_file_status`], this
    /// method clears stale timestamps instead of preserving them.
    pub async fn reset_recovered_file_to_queued(
        &self,
        job_id: &str,
        filename: &str,
    ) -> Result<(), ServerError> {
        sqlx::query(
            "UPDATE file_statuses
             SET status = 'queued',
                 error = NULL,
                 error_category = NULL,
                 bug_report_id = NULL,
                 started_at = NULL,
                 finished_at = NULL,
                 next_eligible_at = NULL
             WHERE job_id = ? AND filename = ?",
        )
        .bind(job_id)
        .bind(filename)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete a job row and its associated `file_statuses` rows.
    ///
    /// Relies on `ON DELETE CASCADE` in the `file_statuses` foreign key.
    pub async fn delete_job(&self, job_id: &crate::api::JobId) -> Result<(), ServerError> {
        sqlx::query("DELETE FROM jobs WHERE job_id = ?")
            .bind(job_id.as_ref())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Finalize a previously inserted attempt row.
    pub async fn finish_attempt(
        &self,
        attempt_id: &str,
        outcome: AttemptOutcome,
        failure_category: Option<FailureCategory>,
        disposition: RetryDisposition,
        finished_at: f64,
    ) -> Result<(), ServerError> {
        sqlx::query(
            "UPDATE attempts
             SET finished_at = ?,
                 outcome = ?,
                 failure_category = ?,
                 disposition = ?
             WHERE attempt_id = ?",
        )
        .bind(finished_at)
        .bind(outcome.to_string())
        .bind(failure_category.map(|category| category.to_string()))
        .bind(disposition.to_string())
        .bind(attempt_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
