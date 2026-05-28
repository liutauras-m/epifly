use common::error::HttpError;
use common::memory::WorkspaceStoreError;

pub(super) fn map_err(e: WorkspaceStoreError) -> HttpError {
    match e {
        WorkspaceStoreError::Validation(msg) => HttpError::validation("body", msg),
        WorkspaceStoreError::NotFound => HttpError::not_found("workspace node not found"),
        WorkspaceStoreError::Forbidden => HttpError::forbidden("access denied"),
        WorkspaceStoreError::Conflict => HttpError::bad_request("concurrent modification"),
        WorkspaceStoreError::Storage(msg) => HttpError::internal(msg, None),
    }
}

pub(super) fn map_content_err(e: anyhow::Error) -> HttpError {
    HttpError::agent(e.to_string())
}
