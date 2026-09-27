//! Browser history projection over opaque workbench snapshots. No execution state.

use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use workbench_store::{Error, NativeState, Store};

use crate::App;
use crate::protocol::{
    ChatHistory, ChatSession, ContentPart, CreateSession, HistoryResponse, Role, SaveHistory,
    SavedRevision, SessionPage,
};

const FORMAT: &str = "ol-rht-chat-history";
const FORMAT_VERSION: &str = "1";
const RUNNER: &str = "ol-rht-chat";
const PAGE_SIZE: u32 = 50;
// A whole branch tree may be larger than the selected model context.
const MAX_HISTORY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct Storage {
    pub store: Store,
    pub project_id: Uuid,
}

pub(crate) fn routes(app: Arc<App>) -> Router<Arc<App>> {
    Router::new()
        .route("/v1/chat/sessions", get(list))
        .route("/v1/chat/sessions/{id}", axum::routing::put(create))
        .route("/v1/chat/sessions/{id}/history", get(load).put(save))
        .route_layer(axum::middleware::from_fn_with_state(app, limit_writes))
        .layer(DefaultBodyLimit::max(MAX_HISTORY_BYTES))
        .layer(axum::middleware::map_response(
            |mut response: Response| async {
                response.headers_mut().insert(
                    header::CACHE_CONTROL,
                    header::HeaderValue::from_static("no-store"),
                );
                response
            },
        ))
}

async fn limit_writes(
    State(app): State<Arc<App>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if request.method() == axum::http::Method::PUT
        && let Some(admission) = &app.admission
        && let Err(rejected) = admission.write()
    {
        return rejected.into_response();
    }
    next.run(request).await
}

struct ApiError(StatusCode, &'static str);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"message": self.1}))).into_response()
    }
}

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        match error {
            Error::InvalidInput(message) => Self(StatusCode::BAD_REQUEST, message),
            Error::NotFound(_) => Self(StatusCode::NOT_FOUND, "Conversation not found."),
            Error::IdConflict | Error::RevisionConflict { .. } => Self(
                StatusCode::CONFLICT,
                "This conversation changed elsewhere. Export unsaved history before reloading it.",
            ),
            Error::Database(_) => Self(
                StatusCode::SERVICE_UNAVAILABLE,
                "Chat storage is unavailable. Your changes have not been confirmed saved.",
            ),
        }
    }
}

fn storage(app: &App) -> Result<&Storage, ApiError> {
    app.storage.as_ref().ok_or(ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "Chat storage is not configured.",
    ))
}

fn uuid(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Expected a UUID."))
}

fn summary(session: workbench_store::Session) -> ChatSession {
    ChatSession {
        id: session.id.to_string(),
        title: session.title,
        created_at: session.created_at.to_rfc3339(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    after: Option<String>,
}

async fn list(
    State(app): State<Arc<App>>,
    Query(query): Query<Cursor>,
) -> Result<Json<SessionPage>, ApiError> {
    let storage = storage(&app)?;
    let after = query.after.as_deref().map(uuid).transpose()?;
    let mut sessions = storage
        .store
        .sessions(storage.project_id, after, PAGE_SIZE + 1)
        .await?;
    let more = sessions.len() > PAGE_SIZE as usize;
    sessions.truncate(PAGE_SIZE as usize);
    let next_cursor = more.then(|| sessions.last().unwrap().id.to_string());
    Ok(Json(SessionPage {
        sessions: sessions.into_iter().map(summary).collect(),
        next_cursor,
    }))
}

async fn create(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Json(request): Json<CreateSession>,
) -> Result<Json<ChatSession>, ApiError> {
    let storage = storage(&app)?;
    Ok(Json(summary(
        storage
            .store
            .create_session(storage.project_id, uuid(&id)?, &request.title)
            .await?,
    )))
}

async fn load(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<Json<HistoryResponse>, ApiError> {
    let storage = storage(&app)?;
    let id = uuid(&id)?;
    storage
        .store
        .session(storage.project_id, id)
        .await?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "Conversation not found."))?;
    let checkpoint = storage
        .store
        .latest_checkpoint(storage.project_id, id)
        .await?;
    let Some(checkpoint) = checkpoint else {
        return Ok(Json(HistoryResponse {
            revision: 0,
            history: ChatHistory {
                head_id: None,
                messages: vec![],
            },
        }));
    };
    check_format(&checkpoint, id)?;
    let history: ChatHistory = serde_json::from_slice(&checkpoint.payload).map_err(|_| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Stored chat history could not be read.",
        )
    })?;
    validate(&history).map_err(|_| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Stored chat history is invalid.",
        )
    })?;
    // Use the revision of these exact bytes, not a separately read session head.
    Ok(Json(HistoryResponse {
        revision: checkpoint.revision,
        history,
    }))
}

fn check_format(checkpoint: &workbench_store::Checkpoint, id: Uuid) -> Result<(), ApiError> {
    if checkpoint.runner != RUNNER
        || checkpoint.format != FORMAT
        || checkpoint.format_version != FORMAT_VERSION
        || checkpoint.native_session_id != id.to_string()
    {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "This session uses an unsupported history format. It has not been changed.",
        ));
    }
    Ok(())
}

async fn save(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Json(request): Json<SaveHistory>,
) -> Result<Json<SavedRevision>, ApiError> {
    let storage = storage(&app)?;
    let id = uuid(&id)?;
    validate(&request.history)?;
    if let Some(checkpoint) = storage
        .store
        .latest_checkpoint(storage.project_id, id)
        .await?
    {
        check_format(&checkpoint, id)?;
    }
    let payload = serde_json::to_vec(&request.history)
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid history."))?;
    if payload.len() > MAX_HISTORY_BYTES {
        return Err(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Chat history exceeds 2 MiB.",
        ));
    }
    let state = NativeState {
        runner: RUNNER.to_owned(),
        runner_version: env!("CARGO_PKG_VERSION").to_owned(),
        format: FORMAT.to_owned(),
        format_version: FORMAT_VERSION.to_owned(),
        native_session_id: id.to_string(),
        payload,
    };
    let checkpoint = storage
        .store
        .save_checkpoint(
            storage.project_id,
            id,
            uuid(&request.checkpoint_id)?,
            request.expected_revision,
            &state,
        )
        .await?;
    Ok(Json(SavedRevision {
        revision: checkpoint.revision,
    }))
}

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 128 && !id.contains('\0')
}

fn validate(history: &ChatHistory) -> Result<(), ApiError> {
    let invalid = || {
        ApiError(
            StatusCode::BAD_REQUEST,
            "Invalid chat history tree or message.",
        )
    };
    if history.messages.len() > 2048 {
        return Err(invalid());
    }
    let mut seen = HashSet::new();
    let mut parents = HashSet::new();
    for item in &history.messages {
        if !valid_id(&item.message.id) || seen.contains(&item.message.id) {
            return Err(invalid());
        }
        if let Some(parent) = &item.parent_id {
            if !seen.contains(parent) {
                return Err(invalid());
            }
            parents.insert(parent);
        }
        seen.insert(&item.message.id);
        if item.created_at.len() > 64
            || chrono::DateTime::parse_from_rfc3339(&item.created_at).is_err()
            || item.message.content.len() > 128
            || (item.message.role == Role::Assistant) != item.status.is_some()
            || item
                .model_message_id
                .as_deref()
                .is_some_and(|id| !valid_id(id))
            || (!item.part_ids.is_empty() && item.part_ids.len() != item.message.content.len())
            || item.part_ids.iter().any(|id| !valid_id(id))
            || item.part_ids.iter().collect::<HashSet<_>>().len() != item.part_ids.len()
        {
            return Err(invalid());
        }
        let mut tool_ids = HashSet::new();
        for part in &item.message.content {
            match part {
                ContentPart::Text { .. } => {}
                ContentPart::Reasoning { .. } if item.message.role == Role::Assistant => {}
                ContentPart::ToolCall {
                    tool_call_id,
                    call,
                    result,
                } if item.message.role == Role::Assistant => {
                    if !valid_id(tool_call_id)
                        || !tool_ids.insert(tool_call_id)
                        || call.validate().is_err()
                        || result.as_ref().is_some_and(|result| !call.matches(result))
                    {
                        return Err(invalid());
                    }
                }
                _ => return Err(invalid()),
            }
        }
        if item.message.role == Role::User
            && !item
                .message
                .content
                .iter()
                .any(|part| matches!(part, ContentPart::Text { text } if !text.trim().is_empty()))
        {
            return Err(invalid());
        }
    }
    match &history.head_id {
        None if history.messages.is_empty() => Ok(()),
        Some(head) if seen.contains(head) && !parents.contains(head) => Ok(()),
        _ => Err(invalid()),
    }
}
