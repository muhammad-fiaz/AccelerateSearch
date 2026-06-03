//! Background job scheduler.

use std::sync::Arc;

use async_trait::async_trait;
use cron::Schedule;
use std::str::FromStr;
use tokio::sync::Notify;
use tracing::{error, info};

use errors::AppResult;

/// A job that runs on a schedule.
#[async_trait]
pub trait Job: Send + Sync + 'static {
    /// Returns the job name (for logging).
    fn name(&self) -> &str;
    /// Runs the job. Errors are logged but never propagated.
    async fn run(&self) -> AppResult<()>;
}

/// Runs a job on a cron schedule. The implementation sleeps until the next
/// scheduled tick, runs the job, and repeats.
pub async fn run_cron(job: Arc<dyn Job>, expression: &str, stop: Arc<Notify>) {
    let schedule = match Schedule::from_str(expression) {
        Ok(s) => s,
        Err(e) => {
            error!(job = %job.name(), error = %e, "invalid cron expression");
            return;
        }
    };
    info!(job = %job.name(), expression, "starting scheduled job");
    while let Some(next) = schedule.upcoming(chrono::Utc).next() {
        let now = chrono::Utc::now();
        let delay = (next - now)
            .to_std()
            .unwrap_or(std::time::Duration::from_secs(1));
        tokio::select! {
            _ = stop.notified() => break,
            _ = tokio::time::sleep(delay) => {
                if let Err(e) = job.run().await {
                    error!(job = %job.name(), error = %e, "job failed");
                }
            }
        }
    }
    info!(job = %job.name(), "scheduled job stopped");
}

/// Runs a job on a fixed interval.
pub async fn run_interval(job: Arc<dyn Job>, interval: std::time::Duration, stop: Arc<Notify>) {
    info!(job = %job.name(), interval_secs = interval.as_secs(), "starting interval job");
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = stop.notified() => break,
            _ = ticker.tick() => {
                if let Err(e) = job.run().await {
                    error!(job = %job.name(), error = %e, "job failed");
                }
            }
        }
    }
    info!(job = %job.name(), "interval job stopped");
}

/// Concrete job that auto-deletes old log files.
pub struct LogCleanupJob {
    pub log_dir: std::path::PathBuf,
    pub days: u64,
}

#[async_trait]
impl Job for LogCleanupJob {
    fn name(&self) -> &str {
        "log-cleanup"
    }

    async fn run(&self) -> AppResult<()> {
        let removed = telemetry::cleanup_old_logs(&self.log_dir, self.days).unwrap_or(0);
        info!(removed, "log cleanup complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopJob;
    #[async_trait]
    impl Job for NoopJob {
        fn name(&self) -> &str {
            "noop"
        }
        async fn run(&self) -> AppResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn run_cron_invalid_exits_immediately() {
        let stop = Arc::new(Notify::new());
        let job: Arc<dyn Job> = Arc::new(NoopJob);
        run_cron(job, "not a cron expression", stop).await;
    }

    #[tokio::test]
    async fn run_interval_runs_and_stops() {
        let stop = Arc::new(Notify::new());
        let job: Arc<dyn Job> = Arc::new(NoopJob);
        let stop_clone = stop.clone();
        let handle = tokio::spawn(async move {
            run_interval(job, std::time::Duration::from_millis(20), stop).await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(75)).await;
        stop_clone.notify_one();
        let _ = tokio::time::timeout(std::time::Duration::from_millis(200), handle).await;
    }
}
