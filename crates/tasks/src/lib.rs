//! Async task queue used to track the status of asynchronous operations
//! such as document ingestion, settings updates, and snapshot creation.

use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use errors::{AppError, AppResult};
use models::{CollectionId, TaskId, TaskInfo, TaskKind, TaskResult, TaskStatus};
use storage::{StorageBackend, get_json, put_json};
use utils::Stopwatch;

/// Storage table used for tasks.
pub const TABLE_TASKS: &str = "tasks";

/// A queued, in-flight, or completed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Task identifier.
    pub uid: TaskId,
    /// Task kind.
    #[serde(rename = "type")]
    pub kind: TaskKind,
    /// Current status.
    pub status: TaskStatus,
    /// Collection the task operates on, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_uid: Option<CollectionId>,
    /// Enqueue time.
    pub enqueued_at: chrono::DateTime<Utc>,
    /// Start time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<chrono::DateTime<Utc>>,
    /// Finish time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<chrono::DateTime<Utc>>,
    /// Duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Number of affected documents (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_documents: Option<u64>,
    /// Error details on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<models::TaskError>,
}

impl Task {
    /// Converts to the public [`TaskInfo`] view.
    #[must_use]
    pub fn to_info(&self) -> TaskInfo {
        TaskInfo {
            uid: self.uid,
            status: self.status,
            kind: self.kind,
            enqueued_at: self.enqueued_at,
            started_at: self.started_at,
            finished_at: self.finished_at,
            duration_ms: self.duration_ms,
            index_uid: self.index_uid.clone(),
            error: self.error.clone(),
            affected_documents: self.affected_documents,
        }
    }

    /// Returns the lightweight [`TaskResult`] view.
    #[must_use]
    pub fn to_result(&self) -> TaskResult {
        TaskResult {
            task_uid: self.uid,
            status: self.status,
            kind: self.kind,
            enqueued_at: self.enqueued_at,
            index_uid: self.index_uid.clone(),
        }
    }
}

/// Handler invoked by the worker loop to execute a task.
#[async_trait]
pub trait TaskHandler: Send + Sync + 'static {
    /// Runs the task, returning the number of affected documents.
    async fn run(&self, task: &Task) -> AppResult<u64>;
}

/// The task queue. Holds both a persistent store and an in-memory queue of
/// tasks ready to be processed.
pub struct TaskQueue {
    storage: Arc<dyn StorageBackend>,
    queue: Mutex<VecDeque<TaskId>>,
    /// Cached lookups of pending tasks to avoid hitting storage for every
    /// queue op.
    pending: dashmap::DashMap<TaskId, Task>,
}

impl TaskQueue {
    /// Creates a new task queue.
    #[must_use]
    pub fn new(storage: Arc<dyn StorageBackend>) -> Self {
        Self {
            storage,
            queue: Mutex::new(VecDeque::new()),
            pending: dashmap::DashMap::new(),
        }
    }

    /// Enqueues a new task and returns a lightweight result.
    pub async fn enqueue(
        &self,
        kind: TaskKind,
        index_uid: Option<CollectionId>,
    ) -> AppResult<TaskResult> {
        let task = Task {
            uid: TaskId::generate(),
            kind,
            status: TaskStatus::Enqueued,
            index_uid: index_uid.clone(),
            enqueued_at: Utc::now(),
            started_at: None,
            finished_at: None,
            duration_ms: None,
            affected_documents: None,
            error: None,
        };
        self.persist(&task).await?;
        self.pending.insert(task.uid, task.clone());
        self.queue.lock().push_back(task.uid);
        info!(task = %task.uid, kind = ?kind, "enqueued task");
        Ok(TaskResult {
            task_uid: task.uid,
            status: TaskStatus::Enqueued,
            kind,
            enqueued_at: task.enqueued_at,
            index_uid,
        })
    }

    /// Returns a task by id.
    pub async fn get(&self, uid: TaskId) -> AppResult<Option<Task>> {
        if let Some(t) = self.pending.get(&uid) {
            return Ok(Some(t.clone()));
        }
        get_json::<Task>(self.storage.as_ref(), TABLE_TASKS, &uid.to_string()).await
    }

    /// Lists tasks with optional filters.
    pub async fn list(
        &self,
        status: Option<TaskStatus>,
        kind: Option<TaskKind>,
        index_uid: Option<CollectionId>,
        limit: usize,
    ) -> AppResult<Vec<TaskInfo>> {
        let keys = self.storage.list(TABLE_TASKS, "").await?;
        let mut out = Vec::new();
        for k in keys {
            if let Some(bytes) = self.storage.get(TABLE_TASKS, &k).await? {
                let t: Task = serde_json::from_slice(&bytes)?;
                if let Some(s) = status
                    && t.status != s
                {
                    continue;
                }
                if let Some(kk) = kind
                    && t.kind != kk
                {
                    continue;
                }
                if let Some(ref col) = index_uid
                    && t.index_uid.as_ref() != Some(col)
                {
                    continue;
                }
                out.push(t.to_info());
                if out.len() >= limit {
                    break;
                }
            }
        }
        out.sort_by_key(|b| std::cmp::Reverse(b.enqueued_at));
        Ok(out)
    }

    /// Pops the next task from the queue and marks it as processing.
    pub async fn next(&self) -> AppResult<Option<Task>> {
        let uid = {
            let mut q = self.queue.lock();
            q.pop_front()
        };
        let Some(uid) = uid else { return Ok(None) };
        let mut task = self
            .get(uid)
            .await?
            .ok_or_else(|| AppError::not_found(format!("task {uid} not found")))?;
        task.status = TaskStatus::Processing;
        task.started_at = Some(Utc::now());
        self.persist(&task).await?;
        self.pending.insert(uid, task.clone());
        Ok(Some(task))
    }

    /// Marks a task as completed.
    pub async fn complete(&self, uid: TaskId, affected_documents: Option<u64>) -> AppResult<()> {
        let mut task = self
            .get(uid)
            .await?
            .ok_or_else(|| AppError::not_found(format!("task {uid} not found")))?;
        task.status = TaskStatus::Succeeded;
        task.finished_at = Some(Utc::now());
        task.duration_ms = Some(task_duration_ms(&task));
        task.affected_documents = affected_documents;
        self.persist(&task).await?;
        self.pending.insert(uid, task);
        accelerate_metrics::TASKS_TOTAL
            .with_label_values(&["task", "succeeded"])
            .inc();
        Ok(())
    }

    /// Marks a task as failed.
    pub async fn fail(&self, uid: TaskId, message: &str) -> AppResult<()> {
        let mut task = self
            .get(uid)
            .await?
            .ok_or_else(|| AppError::not_found(format!("task {uid} not found")))?;
        task.status = TaskStatus::Failed;
        task.finished_at = Some(Utc::now());
        task.duration_ms = Some(task_duration_ms(&task));
        task.error = Some(models::TaskError {
            code: "task_failed".into(),
            message: message.to_string(),
        });
        self.persist(&task).await?;
        self.pending.insert(uid, task);
        accelerate_metrics::TASKS_TOTAL
            .with_label_values(&["task", "failed"])
            .inc();
        Ok(())
    }

    /// Cancels a task. Returns the cancelled task if it was still pending.
    pub async fn cancel(&self, uid: TaskId) -> AppResult<Option<Task>> {
        let Some(mut task) = self.get(uid).await? else {
            return Ok(None);
        };
        if task.status.is_terminal() {
            return Ok(Some(task));
        }
        task.status = TaskStatus::Cancelled;
        task.finished_at = Some(Utc::now());
        self.persist(&task).await?;
        self.pending.insert(uid, task.clone());
        // Remove from the in-memory queue if present.
        self.queue.lock().retain(|id| *id != uid);
        Ok(Some(task))
    }

    /// Cancels all enqueued tasks matching the given predicate.
    pub async fn cancel_filtered<F>(&self, mut pred: F) -> AppResult<Vec<TaskInfo>>
    where
        F: FnMut(&Task) -> bool,
    {
        let mut cancelled = Vec::new();
        let to_cancel: Vec<TaskId> = self
            .pending
            .iter()
            .filter(|kv| {
                let t = kv.value();
                !t.status.is_terminal() && pred(t)
            })
            .map(|kv| *kv.key())
            .collect();
        for id in to_cancel {
            if let Some(t) = self.cancel(id).await? {
                cancelled.push(t.to_info());
            }
        }
        Ok(cancelled)
    }

    /// Returns the number of pending tasks in the in-memory queue.
    #[must_use]
    pub fn queue_len(&self) -> usize {
        self.queue.lock().len()
    }

    /// Spawns a worker loop that processes tasks forever. Returns when
    /// `stop_rx` fires.
    pub async fn run_worker<H: TaskHandler>(
        self: Arc<Self>,
        mut stop_rx: tokio::sync::watch::Receiver<bool>,
        handler: Arc<H>,
    ) {
        loop {
            if *stop_rx.borrow() {
                break;
            }
            match self.next().await {
                Ok(Some(task)) => {
                    let sw = Stopwatch::new();
                    let res = handler.run(&task).await;
                    accelerate_metrics::TASK_PROCESSING_DURATION_SECONDS
                        .observe(sw.elapsed().as_secs_f64());
                    match res {
                        Ok(n) => {
                            if let Err(e) = self.complete(task.uid, Some(n)).await {
                                tracing::error!(error = %e, "failed to mark task complete");
                            }
                        }
                        Err(e) => {
                            if let Err(e2) = self.fail(task.uid, &e.to_string()).await {
                                tracing::error!(error = %e2, "failed to mark task failed");
                            }
                        }
                    }
                }
                Ok(None) => {
                    tokio::select! {
                        _ = stop_rx.changed() => break,
                        _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
                    }
                }
                Err(e) => {
                    debug!(error = %e, "task fetch error");
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }
    }

    async fn persist(&self, task: &Task) -> AppResult<()> {
        put_json(
            self.storage.as_ref(),
            TABLE_TASKS,
            &task.uid.to_string(),
            task,
        )
        .await
    }
}

fn task_duration_ms(task: &Task) -> u64 {
    match (task.started_at, task.finished_at) {
        (Some(s), Some(f)) => (f - s).num_milliseconds().max(0) as u64,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::RedbStorage;

    #[tokio::test]
    async fn enqueue_and_complete() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let q = TaskQueue::new(backend);
        let res = q
            .enqueue(TaskKind::DocumentAdditionOrUpdate, None)
            .await
            .unwrap();
        let task = q.get(res.task_uid).await.unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Enqueued);
        q.complete(res.task_uid, Some(10)).await.unwrap();
        let task = q.get(res.task_uid).await.unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Succeeded);
        assert_eq!(task.affected_documents, Some(10));
    }

    #[tokio::test]
    async fn cancel_pending_task() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let q = TaskQueue::new(backend);
        let res = q
            .enqueue(TaskKind::DocumentAdditionOrUpdate, None)
            .await
            .unwrap();
        let t = q.cancel(res.task_uid).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Cancelled);
    }

    #[tokio::test]
    async fn list_filters() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let q = TaskQueue::new(backend);
        q.enqueue(TaskKind::CollectionCreation, None).await.unwrap();
        q.enqueue(TaskKind::DocumentAdditionOrUpdate, None)
            .await
            .unwrap();
        let docs = q
            .list(None, Some(TaskKind::DocumentAdditionOrUpdate), None, 10)
            .await
            .unwrap();
        assert_eq!(docs.len(), 1);
    }
}
