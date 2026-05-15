//! Startup recovery and TTL pruning operations.

use sqlx::Row;
use tracing::info;

use crate::error::ServerError;

use super::{JobDB, unix_now};

impl JobDB {
    /// Mark queued/running jobs as interrupted on startup.
    ///
    /// Returns the list of job_ids that were marked interrupted.
    pub async fn recover_interrupted(&self) -> Result<Vec<String>, ServerError> {
        let rows = sqlx::query("SELECT job_id FROM jobs WHERE status IN ('queued', 'running')")
            .fetch_all(&self.pool)
            .await?;

        let mut ids = Vec::with_capacity(rows.len());
        for row in &rows {
            ids.push(row.try_get("job_id")?);
        }

        if !ids.is_empty() {
            let now = unix_now();

            // Update jobs
            for id in &ids {
                sqlx::query(
                    "UPDATE jobs SET status = 'interrupted', completed_at = ? WHERE job_id = ?",
                )
                .bind(now)
                .bind(id)
                .execute(&self.pool)
                .await?;
            }

            // Update file statuses
            for id in &ids {
                sqlx::query(
                    "UPDATE file_statuses SET status = 'interrupted' \
                     WHERE job_id = ? AND status IN ('queued', 'processing')",
                )
                .bind(id)
                .execute(&self.pool)
                .await?;
            }

            info!("Marked {} interrupted jobs: {:?}", ids.len(), ids);
        }

        Ok(ids)
    }

    /// Delete jobs whose `submitted_at` is older than `ttl_days` ago.
    ///
    /// Returns the `staging_dir` paths of the pruned jobs so the caller can
    /// clean up the associated files on disk.
    pub async fn prune_expired(&self, ttl_days: i32) -> Result<Vec<String>, ServerError> {
        let cutoff = unix_now() - (ttl_days as f64 * 86400.0);

        let rows = sqlx::query("SELECT job_id, staging_dir FROM jobs WHERE submitted_at < ?")
            .bind(cutoff)
            .fetch_all(&self.pool)
            .await?;

        let mut staging_dirs = Vec::new();
        let mut job_ids = Vec::new();
        for row in &rows {
            let id: String = row.try_get("job_id")?;
            let dir: String = row.try_get("staging_dir")?;
            job_ids.push(id);
            staging_dirs.push(dir);
        }

        if !job_ids.is_empty() {
            for id in &job_ids {
                sqlx::query("DELETE FROM jobs WHERE job_id = ?")
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
            info!("Pruned {} expired jobs", job_ids.len());
        }

        Ok(staging_dirs)
    }
}
