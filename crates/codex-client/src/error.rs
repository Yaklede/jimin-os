use std::io;

use thiserror::Error;

/// Errors exposed by the Codex App Server adapter.
///
/// Variants intentionally avoid carrying server-provided text. This keeps RPC
/// payloads, model output, and credentials out of normal application logs.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Codex App Server I/O failed during {operation}")]
    Io {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("Codex App Server closed the JSONL stream")]
    UnexpectedEof,
    #[error("Codex App Server emitted a JSONL frame larger than {max_bytes} bytes")]
    LineTooLong { max_bytes: usize },
    #[error("Codex App Server emitted non-UTF-8 JSONL data")]
    InvalidUtf8,
    #[error("Codex App Server emitted malformed JSON")]
    MalformedJson,
    #[error("Codex App Server emitted an invalid protocol message")]
    InvalidProtocolMessage,
    #[error("Codex App Server returned an unknown response id")]
    UnknownResponseId,
    #[error("Codex App Server notification queue reached its safe bound")]
    NotificationBackpressure,
    #[error("Codex App Server sent an unsupported server request")]
    UnexpectedServerRequest,
    #[error("Codex App Server returned RPC error code {code}")]
    Rpc { code: i64 },
    #[error("Codex App Server response did not match {method}")]
    InvalidResponse { method: &'static str },
    #[error("Codex App Server client has already been initialized")]
    AlreadyInitialized,
    #[error("Codex App Server client must be initialized first")]
    NotInitialized,
    #[error("Codex App Server turn failed: {reason}")]
    TurnFailed { reason: &'static str },
    #[error("Codex App Server turn completed without a final agent message")]
    MissingFinalAgentMessage,
    #[error("Codex App Server final response exceeded {max_bytes} bytes")]
    AgentResponseTooLarge { max_bytes: usize },
    #[error("Codex CLI could not be started")]
    Spawn(#[source] io::Error),
    #[error("Codex CLI version check failed")]
    VersionCheck(#[source] io::Error),
    #[error("Codex CLI version output was malformed")]
    MalformedVersionOutput,
    #[error("Codex CLI {actual} is incompatible; expected {expected}")]
    IncompatibleVersion {
        expected: &'static str,
        actual: String,
    },
    #[error("The authenticated Codex account is not a ChatGPT account")]
    UnsupportedAccountType,
    #[error("Codex CLI version check exited unsuccessfully")]
    VersionCommandFailed,
    #[error("Agent workspace could not be resolved")]
    Workspace(#[source] io::Error),
    #[error("Agent workspace is not a directory")]
    InvalidWorkspace,
    #[error("Probe model identifier is invalid")]
    InvalidModel,
    #[error("Codex App Server process could not be stopped")]
    Shutdown(#[source] io::Error),
    #[error("Codex App Server shutdown signal handler failed")]
    Signal(#[source] io::Error),
    #[error("Codex App Server child process exited")]
    AppServerExited,
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "app_server_io",
            Self::UnexpectedEof => "app_server_eof",
            Self::LineTooLong { .. } => "app_server_frame_too_large",
            Self::InvalidUtf8 => "app_server_invalid_utf8",
            Self::MalformedJson => "app_server_malformed_json",
            Self::InvalidProtocolMessage => "app_server_invalid_protocol",
            Self::UnknownResponseId => "app_server_unknown_response_id",
            Self::NotificationBackpressure => "app_server_notification_backpressure",
            Self::UnexpectedServerRequest => "app_server_unexpected_request",
            Self::Rpc { .. } => "app_server_rpc_error",
            Self::InvalidResponse { .. } => "app_server_invalid_response",
            Self::AlreadyInitialized => "app_server_already_initialized",
            Self::NotInitialized => "app_server_not_initialized",
            Self::TurnFailed { reason } => reason,
            Self::MissingFinalAgentMessage => "app_server_missing_final_message",
            Self::AgentResponseTooLarge { .. } => "app_server_response_too_large",
            Self::Spawn(_) => "codex_spawn_failed",
            Self::VersionCheck(_) => "codex_version_check_failed",
            Self::MalformedVersionOutput => "codex_version_output_malformed",
            Self::IncompatibleVersion { .. } => "codex_version_incompatible",
            Self::UnsupportedAccountType => "codex_account_type_unsupported",
            Self::VersionCommandFailed => "codex_version_command_failed",
            Self::Workspace(_) => "agent_workspace_unavailable",
            Self::InvalidWorkspace => "agent_workspace_invalid",
            Self::InvalidModel => "invalid_probe_model",
            Self::Shutdown(_) => "codex_shutdown_failed",
            Self::Signal(_) => "agent_signal_failed",
            Self::AppServerExited => "app_server_exited",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
