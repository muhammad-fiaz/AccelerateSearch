//! Tasks REST endpoints.

use actix_web::{HttpResponse, ResponseError, delete, get, post, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use errors::{AppError, AppResult};
use models::CollectionId;
use models::{TaskInfo, TaskKind, TaskStatus};
use tasks::TaskQueue;

use crate::state::AppState;

/// Tasks list query.
#[derive(Debug, Clone, Deserialize)]
pub struct TasksQuery {
    /// Limit on the number of returned tasks.
    pub limit: Option<usize>,
    /// Filter by status.
    pub statuses: Option<String>,
    /// Filter by task kind.
    #[serde(rename = "types")]
    pub kinds: Option<String>,
    /// Filter by collection UID.
    pub index_uids: Option<String>,
    /// Return tasks enqueued after this timestamp.
    pub after_enqueued_at: Option<DateTime<Utc>>,
    /// Return tasks enqueued before this timestamp.
    pub before_enqueued_at: Option<DateTime<Utc>>,
}

/// Lists tasks.
#[utoipa::path(
    get,
    path = "/api/v1/tasks",
    params(
        ("limit" = Option<usize>, Query,),
        ("statuses" = Option<String>, Query,)
    ),
    responses(
        (status = 200, description = "List of tasks", body = TasksResponse)
    ),
    tag = "tasks"
)]
#[get("/tasks")]
pub async fn list_tasks(state: web::Data<AppState>, query: web::Query<TasksQuery>) -> HttpResponse {
    let status = query.statuses.as_deref().and_then(parse_status);
    let kind = query.kinds.as_deref().and_then(parse_kind);
    let index_uid = query.index_uids.as_deref().map(CollectionId::new);
    let limit = query.limit.unwrap_or(20);
    match state.tasks.list(status, kind, index_uid, limit).await {
        Ok(items) => HttpResponse::Ok().json(TasksResponse {
            results: items,
            limit,
            from: None,
            next: None,
        }),
        Err(e) => e.error_response(),
    }
}

/// Response body for the tasks list.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct TasksResponse {
    /// Returned tasks.
    pub results: Vec<TaskInfo>,
    /// Limit used.
    pub limit: usize,
    /// Next cursor (unused).
    pub from: Option<String>,
    /// Next cursor (unused).
    pub next: Option<String>,
}

/// Returns a single task.
#[utoipa::path(
    get,
    path = "/api/v1/tasks/{taskUid}",
    params(("taskUid" = String, Path,)),
    responses(
        (status = 200, description = "Task", body = TaskInfo),
        (status = 404, description = "Task not found")
    ),
    tag = "tasks"
)]
#[get("/tasks/{taskUid}")]
pub async fn get_task(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let id = match uuid::Uuid::parse_str(&path) {
        Ok(u) => models::TaskId::from_uuid(u),
        Err(_) => return AppError::bad_request("invalid task uid").error_response(),
    };
    match state.tasks.get(id).await {
        Ok(Some(t)) => HttpResponse::Ok().json(t.to_info()),
        Ok(None) => AppError::not_found(format!("task {id} not found")).error_response(),
        Err(e) => e.error_response(),
    }
}

/// Cancels all tasks matching the filter.
#[utoipa::path(
    delete,
    path = "/api/v1/tasks",
    responses(
        (status = 200, description = "Tasks cancelled", body = TasksResponse)
    ),
    tag = "tasks"
)]
#[delete("/tasks")]
pub async fn cancel_all_tasks(state: web::Data<AppState>) -> HttpResponse {
    match state.tasks.cancel_filtered(|_| true).await {
        Ok(items) => HttpResponse::Ok().json(TasksResponse {
            results: items,
            limit: 0,
            from: None,
            next: None,
        }),
        Err(e) => e.error_response(),
    }
}

/// Cancels tasks matching a filter.
#[utoipa::path(
    post,
    path = "/api/v1/tasks/cancel",
    request_body = CancelTasksRequest,
    responses(
        (status = 200, description = "Cancelled", body = TasksResponse)
    ),
    tag = "tasks"
)]
#[post("/tasks/cancel")]
pub async fn cancel_tasks(
    state: web::Data<AppState>,
    body: web::Json<CancelTasksRequest>,
) -> HttpResponse {
    let status = body.statuses.as_deref().and_then(parse_status);
    let kind = body.kinds.as_deref().and_then(parse_kind);
    let index_uid = body.index_uids.as_deref().map(CollectionId::new);
    let filter = move |t: &tasks::Task| {
        if let Some(s) = status
            && t.status != s
        {
            return false;
        }
        if let Some(k) = kind
            && t.kind != k
        {
            return false;
        }
        if let Some(ref c) = index_uid
            && t.index_uid.as_ref() != Some(c)
        {
            return false;
        }
        true
    };
    match state.tasks.cancel_filtered(filter).await {
        Ok(items) => HttpResponse::Ok().json(TasksResponse {
            results: items,
            limit: 0,
            from: None,
            next: None,
        }),
        Err(e) => e.error_response(),
    }
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct CancelTasksRequest {
    pub statuses: Option<String>,
    pub kinds: Option<String>,
    pub index_uids: Option<String>,
}

fn parse_status(s: &str) -> Option<TaskStatus> {
    match s {
        "enqueued" => Some(TaskStatus::Enqueued),
        "processing" => Some(TaskStatus::Processing),
        "succeeded" => Some(TaskStatus::Succeeded),
        "failed" => Some(TaskStatus::Failed),
        "cancelled" => Some(TaskStatus::Cancelled),
        _ => None,
    }
}

fn parse_kind(s: &str) -> Option<TaskKind> {
    match s {
        "documentAdditionOrUpdate" => Some(TaskKind::DocumentAdditionOrUpdate),
        "documentDeletion" => Some(TaskKind::DocumentDeletion),
        "collectionCreation" => Some(TaskKind::CollectionCreation),
        "collectionDeletion" => Some(TaskKind::CollectionDeletion),
        "settingsUpdate" => Some(TaskKind::SettingsUpdate),
        "settingsReset" => Some(TaskKind::SettingsReset),
        "snapshotCreation" => Some(TaskKind::SnapshotCreation),
        "snapshotRestoration" => Some(TaskKind::SnapshotRestoration),
        _ => None,
    }
}

#[allow(dead_code)]
fn _suppress_unused(_a: &AppState, _q: &TaskQueue) -> AppResult<()> {
    Ok(())
}
