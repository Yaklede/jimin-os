use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufRead, AsyncWrite};

use crate::codec::{DEFAULT_MAX_LINE_BYTES, JsonLineTransport};
use crate::error::{Error, Result};
use crate::protocol::{Notification, RpcConnection};

const CLIENT_NAME: &str = "jimin-agent";
const CLIENT_TITLE: &str = "Jimin OS Agent";
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_LOGIN_FIELD_BYTES: usize = 4 * 1024;
const MAX_AGENT_RESPONSE_BYTES: usize = 512 * 1024;
const PERSONAL_ASSISTANT_INSTRUCTIONS: &str = "You are Jimin, a private personal AI assistant. Respond directly and helpfully to the user's request using only the conversation context. Do not execute commands, inspect or change files, access the network, or call tools. If the request requires personal data that was not supplied, explain which information is needed instead of attempting a tool call.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub authenticated: bool,
    pub account_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<&'static str>,
    pub requires_openai_auth: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSummary {
    pub status: &'static str,
    pub delta_notifications: u64,
    pub delta_bytes: u64,
    pub retry_notifications: u64,
    pub agent_message_items: u64,
    pub unknown_notifications: u64,
    pub response_bytes: u64,
    pub response_sha256: String,
}

/// A model currently exposed by the managed Codex runtime. The model ID is
/// passed back to `thread/start` or `thread/resume`; no provider model names
/// are compiled into Jimin OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessingModel {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub is_default: bool,
    pub default_reasoning_effort: String,
    pub supported_reasoning_efforts: Vec<ProcessingReasoningEffort>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessingReasoningEffort {
    pub id: String,
    pub description: String,
}

/// Device-code details that are safe to present to the personal app. The
/// managed Codex runtime owns the `ChatGPT` OAuth tokens and refresh lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatgptDeviceCode {
    pub login_id: String,
    pub verification_url: String,
    pub user_code: String,
}

/// A completed personal-agent turn. The answer is intentionally available only
/// to the authenticated caller; logs and health probes use [`TurnSummary`]
/// instead and never retain the response content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedTurn {
    pub response: String,
    pub summary: TurnSummary,
}

pub struct AppServerClient<R, W> {
    connection: RpcConnection<R, W>,
    initialized: bool,
}

impl<R, W> AppServerClient<R, W>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn new(reader: R, writer: W) -> Self {
        Self::with_max_line_bytes(reader, writer, DEFAULT_MAX_LINE_BYTES)
    }

    pub fn with_max_line_bytes(reader: R, writer: W, max_line_bytes: usize) -> Self {
        let transport = JsonLineTransport::new(reader, writer, max_line_bytes);
        Self {
            connection: RpcConnection::new(transport),
            initialized: false,
        }
    }

    /// Performs the required `initialize` request and `initialized` notification handshake.
    ///
    /// # Errors
    ///
    /// Returns a typed transport or protocol error when the handshake fails, or
    /// [`Error::AlreadyInitialized`] when called more than once.
    pub async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Err(Error::AlreadyInitialized);
        }

        let params = InitializeParams {
            client_info: ClientInfo {
                name: CLIENT_NAME,
                title: CLIENT_TITLE,
                version: CLIENT_VERSION,
            },
            capabilities: ClientCapabilities {
                experimental_api: false,
                request_attestation: false,
            },
        };

        let _: InitializeResponse = self.connection.request("initialize", params).await?;
        self.connection.notify("initialized").await?;
        self.initialized = true;
        Ok(())
    }

    /// Reads a credential-free summary of the currently authenticated account.
    ///
    /// # Errors
    ///
    /// Returns a typed transport or protocol error when the request fails, or
    /// [`Error::NotInitialized`] before a successful handshake.
    pub async fn read_account(&mut self) -> Result<AccountSummary> {
        self.require_initialized()?;
        let response: AccountResponse = self
            .connection
            .request("account/read", json!({ "refreshToken": false }))
            .await?;

        Ok(summarize_account(response))
    }

    /// Lists the visible processing models exposed by the current Codex
    /// runtime. Pagination is exhausted so settings never show a partial list.
    ///
    /// # Errors
    ///
    /// Returns a typed transport or protocol error when the response is
    /// malformed, or [`Error::NotInitialized`] before a successful handshake.
    pub async fn list_processing_models(&mut self) -> Result<Vec<ProcessingModel>> {
        self.require_initialized()?;
        let mut models = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let response: ModelListResponse = self
                .connection
                .request(
                    "model/list",
                    json!({
                        "cursor": cursor,
                        "limit": 100,
                        "includeHidden": false
                    }),
                )
                .await?;
            models.extend(response.data.into_iter().map(ProcessingModel::from));
            let Some(next_cursor) = response.next_cursor else {
                break;
            };
            if next_cursor.is_empty() {
                return Err(Error::InvalidResponse {
                    method: "model/list",
                });
            }
            cursor = Some(next_cursor);
        }
        if models.is_empty() {
            return Err(Error::InvalidResponse {
                method: "model/list",
            });
        }
        Ok(models)
    }

    /// Starts Codex-managed `ChatGPT` device-code login. The caller shows the
    /// returned URL and code, while Codex persists and refreshes the resulting
    /// tokens inside `CODEX_HOME`.
    ///
    /// # Errors
    ///
    /// Returns a typed protocol error without returning any OAuth token.
    pub async fn start_chatgpt_device_code_login(&mut self) -> Result<ChatgptDeviceCode> {
        self.require_initialized()?;
        let response: DeviceCodeLoginResponse = self
            .connection
            .request(
                "account/login/start",
                json!({ "type": "chatgptDeviceCode" }),
            )
            .await?;
        if response.login_type != "chatgptDeviceCode"
            || !valid_login_field(&response.login_id)
            || !valid_login_field(&response.verification_url)
            || !valid_login_field(&response.user_code)
        {
            return Err(Error::InvalidResponse {
                method: "account/login/start",
            });
        }
        Ok(ChatgptDeviceCode {
            login_id: response.login_id,
            verification_url: response.verification_url,
            user_code: response.user_code,
        })
    }

    pub(crate) async fn discard_next_notification(&mut self) -> Result<()> {
        self.require_initialized()?;
        let _notification = self.connection.next_notification().await?;
        Ok(())
    }

    /// Starts an ephemeral thread with a read-only sandbox and no approvals.
    ///
    /// # Errors
    ///
    /// Returns a typed transport or protocol error when the request fails, or
    /// [`Error::NotInitialized`] before a successful handshake.
    pub async fn start_ephemeral_thread(&mut self) -> Result<String> {
        self.start_ephemeral_thread_with_options(None, None).await
    }

    /// Starts a safe ephemeral thread rooted at an explicit trusted workspace.
    ///
    /// # Errors
    ///
    /// Returns a typed transport, workspace, or protocol error when the request fails, or
    /// [`Error::NotInitialized`] before a successful handshake.
    pub async fn start_ephemeral_thread_in(
        &mut self,
        cwd: &Path,
        model: Option<&str>,
    ) -> Result<String> {
        let cwd = cwd.to_str().ok_or(Error::InvalidWorkspace)?;
        self.start_ephemeral_thread_with_options(Some(cwd), model)
            .await
    }

    /// Starts a persistent, read-only conversation thread in the trusted agent
    /// workspace. The returned ID can be used for future turns after a client
    /// reconnects to the same Codex-managed `CODEX_HOME`.
    ///
    /// # Errors
    ///
    /// Returns a typed protocol or workspace error without executing tools.
    pub async fn start_persistent_thread_in(
        &mut self,
        cwd: &Path,
        model: Option<&str>,
    ) -> Result<String> {
        self.require_initialized()?;
        let cwd = cwd.to_str().ok_or(Error::InvalidWorkspace)?;
        let response: ThreadStartResponse = self
            .connection
            .request(
                "thread/start",
                json!({
                    "approvalPolicy": "never",
                    "sandbox": "read-only",
                    "cwd": cwd,
                    "developerInstructions": PERSONAL_ASSISTANT_INSTRUCTIONS,
                    "model": model,
                    "serviceTier": null
                }),
            )
            .await?;
        if response.thread.id.is_empty() {
            return Err(Error::InvalidResponse {
                method: "thread/start",
            });
        }
        Ok(response.thread.id)
    }

    /// Rejoins a previously persisted, read-only conversation thread in the
    /// trusted agent workspace.
    ///
    /// # Errors
    ///
    /// Returns a typed protocol or workspace error without executing tools.
    pub async fn resume_persistent_thread_in(
        &mut self,
        thread_id: &str,
        cwd: &Path,
        model: Option<&str>,
    ) -> Result<String> {
        self.require_initialized()?;
        if thread_id.is_empty() || thread_id.len() > MAX_LOGIN_FIELD_BYTES {
            return Err(Error::InvalidProtocolMessage);
        }
        let cwd = cwd.to_str().ok_or(Error::InvalidWorkspace)?;
        let response: ThreadStartResponse = self
            .connection
            .request(
                "thread/resume",
                json!({
                    "threadId": thread_id,
                    "approvalPolicy": "never",
                    "sandbox": "read-only",
                    "cwd": cwd,
                    "developerInstructions": PERSONAL_ASSISTANT_INSTRUCTIONS,
                    "model": model,
                    "serviceTier": null
                }),
            )
            .await?;
        if response.thread.id.is_empty() || response.thread.id != thread_id {
            return Err(Error::InvalidResponse {
                method: "thread/resume",
            });
        }
        Ok(response.thread.id)
    }

    async fn start_ephemeral_thread_with_options(
        &mut self,
        cwd: Option<&str>,
        model: Option<&str>,
    ) -> Result<String> {
        self.require_initialized()?;
        let response: ThreadStartResponse = self
            .connection
            .request(
                "thread/start",
                json!({
                    "ephemeral": true,
                    "approvalPolicy": "never",
                    "sandbox": "read-only",
                    "cwd": cwd,
                    "developerInstructions": PERSONAL_ASSISTANT_INSTRUCTIONS,
                    "model": model,
                    "serviceTier": null
                }),
            )
            .await?;

        if response.thread.id.is_empty() {
            return Err(Error::InvalidResponse {
                method: "thread/start",
            });
        }
        Ok(response.thread.id)
    }

    /// Runs a text turn and returns only content length, counts, status, and digest metadata.
    ///
    /// # Errors
    ///
    /// Returns a typed transport or protocol error when streaming fails, when
    /// the turn does not complete, or when no authoritative final agent message arrives.
    pub async fn run_turn(&mut self, thread_id: &str, prompt: &str) -> Result<TurnSummary> {
        Ok(self
            .run_turn_with_response(thread_id, prompt)
            .await?
            .summary)
    }

    /// Runs a conversation turn and returns its authoritative final message to
    /// the caller. Tool execution remains disabled by the thread's read-only,
    /// no-approval policy.
    ///
    /// # Errors
    ///
    /// Returns a typed error for protocol failures, failed turns, or a final
    /// answer exceeding the bounded private-response payload.
    pub async fn run_turn_with_response(
        &mut self,
        thread_id: &str,
        prompt: &str,
    ) -> Result<CompletedTurn> {
        self.run_turn_with_response_streaming(thread_id, prompt, |_| {})
            .await
    }

    /// Runs a conversation turn and invokes `on_delta` for each matching
    /// agent-message fragment before the authoritative final response arrives.
    ///
    /// # Errors
    ///
    /// Returns a typed error for protocol failures, failed turns, or a final
    /// answer exceeding the bounded private-response payload.
    pub async fn run_turn_with_response_streaming<F>(
        &mut self,
        thread_id: &str,
        prompt: &str,
        on_delta: F,
    ) -> Result<CompletedTurn>
    where
        F: FnMut(&str),
    {
        self.run_turn_with_response_streaming_with_options(thread_id, prompt, None, None, on_delta)
            .await
    }

    /// Runs a conversation turn with model and reasoning overrides that apply
    /// to this and subsequent turns in the Codex thread.
    ///
    /// # Errors
    ///
    /// Returns a typed error for protocol failures, failed turns, or a final
    /// answer exceeding the bounded private-response payload.
    pub async fn run_turn_with_response_streaming_with_options<F>(
        &mut self,
        thread_id: &str,
        prompt: &str,
        model: Option<&str>,
        reasoning_effort: Option<&str>,
        mut on_delta: F,
    ) -> Result<CompletedTurn>
    where
        F: FnMut(&str),
    {
        self.require_initialized()?;
        if thread_id.is_empty() || prompt.is_empty() {
            return Err(Error::InvalidProtocolMessage);
        }

        let response: TurnStartResponse = self
            .connection
            .request(
                "turn/start",
                json!({
                    "threadId": thread_id,
                    "input": [{
                        "type": "text",
                        "text": prompt,
                        "text_elements": []
                    }],
                    "model": model,
                    "effort": reasoning_effort
                }),
            )
            .await?;

        if response.turn.id.is_empty() {
            return Err(Error::InvalidResponse {
                method: "turn/start",
            });
        }

        self.collect_turn_with_deltas(thread_id, &response.turn.id, &mut on_delta)
            .await
    }

    fn require_initialized(&self) -> Result<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(Error::NotInitialized)
        }
    }

    async fn collect_turn_with_deltas<F>(
        &mut self,
        thread_id: &str,
        turn_id: &str,
        on_delta: &mut F,
    ) -> Result<CompletedTurn>
    where
        F: FnMut(&str),
    {
        let mut delta_notifications = 0_u64;
        let mut delta_bytes = 0_u64;
        let mut retry_notifications = 0_u64;
        let mut agent_message_items = 0_u64;
        let mut unknown_notifications = 0_u64;
        let mut final_response: Option<Vec<u8>> = None;
        let mut last_retry_reason: Option<&'static str> = None;

        loop {
            let notification = self.connection.next_notification().await?;
            match notification.method.as_str() {
                "item/agentMessage/delta" => {
                    let params: AgentMessageDelta = parse_params(notification)?;
                    if params.thread_id == thread_id && params.turn_id == turn_id {
                        delta_notifications = delta_notifications.saturating_add(1);
                        delta_bytes = delta_bytes.saturating_add(params.delta.len() as u64);
                        on_delta(&params.delta);
                    }
                }
                "item/completed" => {
                    let params: ItemCompleted = parse_params(notification)?;
                    if params.thread_id == thread_id
                        && params.turn_id == turn_id
                        && let ThreadItem::AgentMessage { text } = params.item
                    {
                        agent_message_items = agent_message_items.saturating_add(1);
                        final_response = Some(text.into_bytes());
                    }
                }
                "turn/completed" => {
                    let params: TurnCompleted = parse_params(notification)?;
                    if params.thread_id != thread_id || params.turn.id != turn_id {
                        unknown_notifications = unknown_notifications.saturating_add(1);
                        continue;
                    }
                    if params.turn.status != "completed" {
                        let mapped_reason = safe_turn_error_reason(params.turn.error.as_ref());
                        let reason = if params.turn.status == "interrupted" {
                            "turn_interrupted"
                        } else if mapped_reason == "turn_failed_other" {
                            last_retry_reason.unwrap_or(mapped_reason)
                        } else {
                            mapped_reason
                        };
                        return Err(Error::TurnFailed { reason });
                    }
                    let response = final_response.ok_or(Error::MissingFinalAgentMessage)?;
                    if response.len() > MAX_AGENT_RESPONSE_BYTES {
                        return Err(Error::AgentResponseTooLarge {
                            max_bytes: MAX_AGENT_RESPONSE_BYTES,
                        });
                    }
                    let response_bytes = response.len() as u64;
                    let response_sha256 = sha256_hex(&response);
                    let response = String::from_utf8(response).map_err(|_| Error::InvalidUtf8)?;
                    return Ok(CompletedTurn {
                        response,
                        summary: TurnSummary {
                            status: "completed",
                            delta_notifications,
                            delta_bytes,
                            retry_notifications,
                            agent_message_items,
                            unknown_notifications,
                            response_bytes,
                            response_sha256,
                        },
                    });
                }
                "error" => {
                    let params: ErrorNotification = parse_params(notification)?;
                    if params.thread_id != thread_id || params.turn_id != turn_id {
                        unknown_notifications = unknown_notifications.saturating_add(1);
                    } else if params.will_retry {
                        retry_notifications = retry_notifications.saturating_add(1);
                        last_retry_reason = Some(safe_turn_error_reason(Some(&params.error)));
                    } else {
                        return Err(Error::TurnFailed {
                            reason: safe_turn_error_reason(Some(&params.error)),
                        });
                    }
                }
                _ => {
                    unknown_notifications = unknown_notifications.saturating_add(1);
                }
            }
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn parse_params<T>(notification: Notification) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let params = notification.params.ok_or(Error::InvalidProtocolMessage)?;
    serde_json::from_value(params).map_err(|_| Error::InvalidProtocolMessage)
}

fn summarize_account(response: AccountResponse) -> AccountSummary {
    let Some(account) = response.account else {
        return AccountSummary {
            authenticated: false,
            account_type: "none",
            plan_type: None,
            requires_openai_auth: response.requires_openai_auth,
        };
    };

    let account_type = account
        .get("type")
        .and_then(Value::as_str)
        .map_or("unknown", safe_account_type);
    let plan_type = account
        .get("planType")
        .and_then(Value::as_str)
        .map(safe_plan_type);

    AccountSummary {
        authenticated: true,
        account_type,
        plan_type,
        requires_openai_auth: response.requires_openai_auth,
    }
}

fn safe_account_type(value: &str) -> &'static str {
    match value {
        "apiKey" => "apiKey",
        "chatgpt" => "chatgpt",
        "amazonBedrock" => "amazonBedrock",
        _ => "unknown",
    }
}

fn safe_plan_type(value: &str) -> &'static str {
    match value {
        "free" => "free",
        "go" => "go",
        "plus" => "plus",
        "pro" => "pro",
        "prolite" => "prolite",
        "team" => "team",
        "self_serve_business_usage_based" => "self_serve_business_usage_based",
        "business" => "business",
        "enterprise_cbp_usage_based" => "enterprise_cbp_usage_based",
        "enterprise" => "enterprise",
        "edu" => "edu",
        _ => "unknown",
    }
}

fn valid_login_field(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_LOGIN_FIELD_BYTES
        && !value.chars().any(char::is_control)
}

fn safe_turn_error_reason(error: Option<&TurnError>) -> &'static str {
    let Some(error) = error else {
        return "turn_failed_other";
    };
    safe_codex_error_reason(error.codex_error_info.as_ref())
        .unwrap_or_else(|| safe_message_error_reason(&error.message))
}

fn safe_codex_error_reason(codex_error_info: Option<&Value>) -> Option<&'static str> {
    match codex_error_info {
        Some(Value::String(value)) => match value.as_str() {
            "contextWindowExceeded" => Some("turn_context_window_exceeded"),
            "usageLimitExceeded" => Some("turn_usage_limit_exceeded"),
            "serverOverloaded" => Some("turn_server_overloaded"),
            "cyberPolicy" => Some("turn_cyber_policy"),
            "internalServerError" => Some("turn_internal_server_error"),
            "unauthorized" => Some("turn_unauthorized"),
            "badRequest" => Some("turn_bad_request"),
            "threadRollbackFailed" => Some("turn_thread_rollback_failed"),
            "sandboxError" => Some("turn_sandbox_error"),
            _ => None,
        },
        Some(Value::Object(value)) if value.contains_key("httpConnectionFailed") => {
            Some("turn_http_connection_failed")
        }
        Some(Value::Object(value)) if value.contains_key("responseStreamConnectionFailed") => {
            Some("turn_response_stream_connection_failed")
        }
        Some(Value::Object(value)) if value.contains_key("responseStreamDisconnected") => {
            Some("turn_response_stream_disconnected")
        }
        Some(Value::Object(value)) if value.contains_key("responseTooManyFailedAttempts") => {
            Some("turn_response_too_many_failed_attempts")
        }
        Some(Value::Object(value)) if value.contains_key("activeTurnNotSteerable") => {
            Some("turn_active_not_steerable")
        }
        _ => None,
    }
}

fn safe_message_error_reason(message: &str) -> &'static str {
    let message = message.to_ascii_lowercase();
    if message.contains("trusted directory") || message.contains("git repo") {
        "turn_workspace_not_trusted"
    } else if message.contains("model") && message.contains("support") {
        "turn_model_not_supported"
    } else if message.contains("authentication") || message.contains("unauthorized") {
        "turn_unauthorized"
    } else if message.contains("rate limit") || message.contains("usage limit") {
        "turn_usage_limit_exceeded"
    } else {
        "turn_failed_other"
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    client_info: ClientInfo,
    capabilities: ClientCapabilities,
}

#[derive(Serialize)]
struct ClientInfo {
    name: &'static str,
    title: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientCapabilities {
    experimental_api: bool,
    request_attestation: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResponse {
    #[serde(rename = "userAgent")]
    _user_agent: String,
    #[serde(rename = "codexHome")]
    _codex_home: String,
    #[serde(rename = "platformFamily")]
    _platform_family: String,
    #[serde(rename = "platformOs")]
    _platform_os: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountResponse {
    account: Option<Value>,
    requires_openai_auth: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelListResponse {
    data: Vec<ModelListItem>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelListItem {
    id: String,
    display_name: String,
    description: String,
    is_default: bool,
    default_reasoning_effort: String,
    supported_reasoning_efforts: Vec<ModelReasoningEffortItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelReasoningEffortItem {
    reasoning_effort: String,
    description: String,
}

impl From<ModelListItem> for ProcessingModel {
    fn from(model: ModelListItem) -> Self {
        Self {
            id: model.id,
            display_name: model.display_name,
            description: model.description,
            is_default: model.is_default,
            default_reasoning_effort: model.default_reasoning_effort,
            supported_reasoning_efforts: model
                .supported_reasoning_efforts
                .into_iter()
                .map(|effort| ProcessingReasoningEffort {
                    id: effort.reasoning_effort,
                    description: effort.description,
                })
                .collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceCodeLoginResponse {
    #[serde(rename = "type")]
    login_type: String,
    login_id: String,
    verification_url: String,
    user_code: String,
}

#[derive(Deserialize)]
struct ThreadStartResponse {
    thread: ThreadReference,
}

#[derive(Deserialize)]
struct ThreadReference {
    id: String,
}

#[derive(Deserialize)]
struct TurnStartResponse {
    turn: TurnReference,
}

#[derive(Deserialize)]
struct TurnReference {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentMessageDelta {
    thread_id: String,
    turn_id: String,
    delta: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemCompleted {
    thread_id: String,
    turn_id: String,
    item: ThreadItem,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ThreadItem {
    #[serde(rename = "agentMessage")]
    AgentMessage { text: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnCompleted {
    thread_id: String,
    turn: AppServerCompletedTurn,
}

#[derive(Deserialize)]
struct AppServerCompletedTurn {
    id: String,
    status: String,
    error: Option<TurnError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorNotification {
    error: TurnError,
    thread_id: String,
    turn_id: String,
    will_retry: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnError {
    message: String,
    codex_error_info: Option<Value>,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, split};

    use super::{AppServerClient, TurnError, safe_turn_error_reason};

    async fn read_json_line<R>(reader: &mut BufReader<R>) -> Value
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let mut line = String::new();
        reader.read_line(&mut line).await.expect("fixture line");
        serde_json::from_str(&line).expect("fixture request json")
    }

    #[tokio::test]
    async fn sends_initialized_only_after_initialize_response() {
        let (client, server) = tokio::io::duplex(16 * 1024);
        let (client_reader, client_writer) = split(client);
        let mut client = AppServerClient::new(BufReader::new(client_reader), client_writer);

        let server_task = tokio::spawn(async move {
            let (server_reader, mut server_writer) = split(server);
            let mut server_reader = BufReader::new(server_reader);
            let request = read_json_line(&mut server_reader).await;
            assert_eq!(request["method"], "initialize");
            server_writer
                .write_all(b"{\"id\":1,\"result\":{\"userAgent\":\"fixture\",\"codexHome\":\"/tmp\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}\n")
                .await
                .expect("initialize response");
            let initialized = read_json_line(&mut server_reader).await;
            assert_eq!(initialized, json!({"method": "initialized"}));
        });

        client.initialize().await.expect("initialize");
        server_task.await.expect("server task");
    }

    #[tokio::test]
    async fn account_summary_never_exposes_email() {
        let (client, server) = tokio::io::duplex(16 * 1024);
        let (client_reader, client_writer) = split(client);
        let mut client = AppServerClient::new(BufReader::new(client_reader), client_writer);

        let server_task = tokio::spawn(async move {
            let (server_reader, mut server_writer) = split(server);
            let mut server_reader = BufReader::new(server_reader);
            let _initialize = read_json_line(&mut server_reader).await;
            server_writer
                .write_all(b"{\"id\":1,\"result\":{\"userAgent\":\"fixture\",\"codexHome\":\"/tmp\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}\n")
                .await
                .expect("initialize response");
            let _initialized = read_json_line(&mut server_reader).await;
            let account_request = read_json_line(&mut server_reader).await;
            assert_eq!(account_request["method"], "account/read");
            server_writer
                .write_all(b"{\"id\":2,\"result\":{\"account\":{\"type\":\"chatgpt\",\"email\":\"private@example.com\",\"planType\":\"plus\"},\"requiresOpenaiAuth\":true}}\n")
                .await
                .expect("account response");
        });

        client.initialize().await.expect("initialize");
        let summary = client.read_account().await.expect("account");
        let encoded = serde_json::to_string(&summary).expect("summary json");
        assert!(summary.authenticated);
        assert_eq!(summary.account_type, "chatgpt");
        assert_eq!(summary.plan_type, Some("plus"));
        assert!(!encoded.contains("private@example.com"));
        server_task.await.expect("server task");
    }

    #[tokio::test]
    async fn model_list_is_paginated_without_hardcoded_provider_ids() {
        let (client, server) = tokio::io::duplex(32 * 1024);
        let (client_reader, client_writer) = split(client);
        let mut client = AppServerClient::new(BufReader::new(client_reader), client_writer);

        let server_task = tokio::spawn(async move {
            let (server_reader, mut server_writer) = split(server);
            let mut server_reader = BufReader::new(server_reader);
            let _initialize = read_json_line(&mut server_reader).await;
            server_writer
                .write_all(b"{\"id\":1,\"result\":{\"userAgent\":\"fixture\",\"codexHome\":\"/tmp\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}\n")
                .await
                .expect("initialize response");
            let _initialized = read_json_line(&mut server_reader).await;

            let first = read_json_line(&mut server_reader).await;
            assert_eq!(first["method"], "model/list");
            assert_eq!(first["params"]["cursor"], Value::Null);
            assert_eq!(first["params"]["includeHidden"], false);
            server_writer
                .write_all(b"{\"id\":2,\"result\":{\"data\":[{\"id\":\"provider-default\",\"displayName\":\"Provider Default\",\"description\":\"Default model\",\"isDefault\":true,\"defaultReasoningEffort\":\"medium\",\"supportedReasoningEfforts\":[{\"reasoningEffort\":\"low\",\"description\":\"Fast\"},{\"reasoningEffort\":\"medium\",\"description\":\"Balanced\"}]}],\"nextCursor\":\"page-2\"}}\n")
                .await
                .expect("first model page");

            let second = read_json_line(&mut server_reader).await;
            assert_eq!(second["method"], "model/list");
            assert_eq!(second["params"]["cursor"], "page-2");
            server_writer
                .write_all(b"{\"id\":3,\"result\":{\"data\":[{\"id\":\"provider-fast\",\"displayName\":\"Provider Fast\",\"description\":\"Fast model\",\"isDefault\":false,\"defaultReasoningEffort\":\"low\",\"supportedReasoningEfforts\":[{\"reasoningEffort\":\"low\",\"description\":\"Fast\"}]}],\"nextCursor\":null}}\n")
                .await
                .expect("second model page");
        });

        client.initialize().await.expect("initialize");
        let models = client
            .list_processing_models()
            .await
            .expect("models should load");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "provider-default");
        assert!(models[0].is_default);
        assert_eq!(models[1].display_name, "Provider Fast");
        assert_eq!(models[0].default_reasoning_effort, "medium");
        assert_eq!(models[0].supported_reasoning_efforts.len(), 2);
        server_task.await.expect("server task");
    }

    #[tokio::test]
    async fn device_code_login_returns_only_presentable_login_fields() {
        let (client, server) = tokio::io::duplex(16 * 1024);
        let (client_reader, client_writer) = split(client);
        let mut client = AppServerClient::new(BufReader::new(client_reader), client_writer);

        let server_task = tokio::spawn(async move {
            let (server_reader, mut server_writer) = split(server);
            let mut server_reader = BufReader::new(server_reader);
            let _initialize = read_json_line(&mut server_reader).await;
            server_writer
                .write_all(b"{\"id\":1,\"result\":{\"userAgent\":\"fixture\",\"codexHome\":\"/tmp\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}\n")
                .await
                .expect("initialize response");
            let _initialized = read_json_line(&mut server_reader).await;
            let login_request = read_json_line(&mut server_reader).await;
            assert_eq!(login_request["method"], "account/login/start");
            assert_eq!(
                login_request["params"],
                json!({"type": "chatgptDeviceCode"})
            );
            server_writer
                .write_all(b"{\"id\":2,\"result\":{\"type\":\"chatgptDeviceCode\",\"loginId\":\"login-1\",\"verificationUrl\":\"https://auth.openai.com/codex/device\",\"userCode\":\"ABCD-1234\"}}\n")
                .await
                .expect("login response");
        });

        client.initialize().await.expect("initialize");
        let login = client
            .start_chatgpt_device_code_login()
            .await
            .expect("device login should start");
        assert_eq!(login.login_id, "login-1");
        assert_eq!(login.user_code, "ABCD-1234");
        assert_eq!(
            login.verification_url,
            "https://auth.openai.com/codex/device"
        );
        server_task.await.expect("server task");
    }

    #[tokio::test]
    async fn idle_notification_is_drained_without_retaining_content() {
        let (client, server) = tokio::io::duplex(16 * 1024);
        let (client_reader, client_writer) = split(client);
        let mut client = AppServerClient::new(BufReader::new(client_reader), client_writer);

        let server_task = tokio::spawn(async move {
            let (server_reader, mut server_writer) = split(server);
            let mut server_reader = BufReader::new(server_reader);
            let _initialize = read_json_line(&mut server_reader).await;
            server_writer
                .write_all(b"{\"id\":1,\"result\":{\"userAgent\":\"fixture\",\"codexHome\":\"/tmp\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}\n")
                .await
                .expect("initialize response");
            let _initialized = read_json_line(&mut server_reader).await;
            server_writer
                .write_all(b"{\"method\":\"account/rateLimits/updated\",\"params\":{\"private\":\"must-be-dropped\"}}\n")
                .await
                .expect("idle notification");
        });

        client.initialize().await.expect("initialize");
        client
            .discard_next_notification()
            .await
            .expect("notification drain");
        server_task.await.expect("server task");
    }

    #[tokio::test]
    async fn resumes_a_persisted_thread_with_the_read_only_contract() {
        let (client, server) = tokio::io::duplex(16 * 1024);
        let (client_reader, client_writer) = split(client);
        let mut client = AppServerClient::new(BufReader::new(client_reader), client_writer);

        let server_task = tokio::spawn(async move {
            let (server_reader, mut server_writer) = split(server);
            let mut server_reader = BufReader::new(server_reader);
            let _initialize = read_json_line(&mut server_reader).await;
            server_writer
                .write_all(b"{\"id\":1,\"result\":{\"userAgent\":\"fixture\",\"codexHome\":\"/tmp\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}\n")
                .await
                .expect("initialize response");
            let _initialized = read_json_line(&mut server_reader).await;
            let request = read_json_line(&mut server_reader).await;
            assert_eq!(request["method"], "thread/resume");
            assert_eq!(request["params"]["threadId"], "thread-1");
            assert_eq!(request["params"]["sandbox"], "read-only");
            assert_eq!(request["params"]["approvalPolicy"], "never");
            assert_eq!(request["params"]["cwd"], "/tmp/fixture-workspace");
            server_writer
                .write_all(b"{\"id\":2,\"result\":{\"thread\":{\"id\":\"thread-1\"}}}\n")
                .await
                .expect("thread response");
        });

        client.initialize().await.expect("initialize");
        let thread_id = client
            .resume_persistent_thread_in("thread-1", Path::new("/tmp/fixture-workspace"), None)
            .await
            .expect("thread should resume");
        assert_eq!(thread_id, "thread-1");
        server_task.await.expect("server task");
    }

    #[tokio::test]
    async fn summarizes_turn_without_returning_response_content() {
        let (client, server) = tokio::io::duplex(64 * 1024);
        let (client_reader, client_writer) = split(client);
        let mut client = AppServerClient::new(BufReader::new(client_reader), client_writer);

        let server_task = tokio::spawn(async move {
            let (server_reader, mut server_writer) = split(server);
            let mut server_reader = BufReader::new(server_reader);
            let _initialize = read_json_line(&mut server_reader).await;
            server_writer
                .write_all(b"{\"id\":1,\"result\":{\"userAgent\":\"fixture\",\"codexHome\":\"/tmp\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}\n")
                .await
                .expect("initialize response");
            let _initialized = read_json_line(&mut server_reader).await;
            let thread_request = read_json_line(&mut server_reader).await;
            assert_eq!(thread_request["params"]["ephemeral"], true);
            assert_eq!(thread_request["params"]["sandbox"], "read-only");
            assert_eq!(thread_request["params"]["approvalPolicy"], "never");
            assert_eq!(thread_request["params"]["cwd"], "/tmp/fixture-workspace");
            assert_eq!(thread_request["params"]["model"], "gpt-fixture");
            assert!(thread_request["params"]["serviceTier"].is_null());
            server_writer
                .write_all(b"{\"id\":2,\"result\":{\"thread\":{\"id\":\"thread-1\"}}}\n")
                .await
                .expect("thread response");
            let turn_request = read_json_line(&mut server_reader).await;
            assert_eq!(turn_request["method"], "turn/start");
            assert_eq!(turn_request["params"]["model"], "gpt-fixture");
            assert_eq!(turn_request["params"]["effort"], "high");
            assert_eq!(
                turn_request["params"]["input"][0]["text_elements"],
                json!([])
            );
            server_writer
                .write_all(b"{\"method\":\"future/notification\",\"params\":{\"content\":\"do-not-log\"}}\n")
                .await
                .expect("unknown notification");
            server_writer
                .write_all(b"{\"id\":3,\"result\":{\"turn\":{\"id\":\"turn-1\",\"status\":\"inProgress\"}}}\n")
                .await
                .expect("turn response");
            server_writer
                .write_all(b"{\"method\":\"error\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"willRetry\":true,\"error\":{\"message\":\"private upstream message\",\"additionalDetails\":\"private details\",\"codexErrorInfo\":{\"responseStreamDisconnected\":{\"httpStatusCode\":503}}}}}\n")
                .await
                .expect("retry notification");
            server_writer
                .write_all(b"{\"method\":\"item/agentMessage/delta\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"itemId\":\"item-1\",\"delta\":\"stream draft\"}}\n")
                .await
                .expect("delta");
            server_writer
                .write_all(b"{\"method\":\"item/completed\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"completedAtMs\":1,\"item\":{\"type\":\"agentMessage\",\"id\":\"item-1\",\"text\":\"authoritative final\",\"phase\":\"final_answer\",\"memoryCitation\":null}}}\n")
                .await
                .expect("completed item");
            server_writer
                .write_all(b"{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread-1\",\"turn\":{\"id\":\"turn-1\",\"status\":\"completed\",\"items\":[],\"error\":null}}}\n")
                .await
                .expect("turn completed");
        });

        client.initialize().await.expect("initialize");
        let thread_id = client
            .start_ephemeral_thread_in(Path::new("/tmp/fixture-workspace"), Some("gpt-fixture"))
            .await
            .expect("thread");
        let mut received_deltas = Vec::new();
        let completed = client
            .run_turn_with_response_streaming_with_options(
                &thread_id,
                "Summarize a generic fixture.",
                Some("gpt-fixture"),
                Some("high"),
                |delta| {
                    received_deltas.push(delta.to_owned());
                },
            )
            .await
            .expect("turn");
        let summary = completed.summary;
        let encoded = serde_json::to_string(&summary).expect("summary json");
        assert_eq!(summary.status, "completed");
        assert_eq!(summary.delta_notifications, 1);
        assert_eq!(summary.retry_notifications, 1);
        assert_eq!(summary.unknown_notifications, 1);
        assert_eq!(summary.response_bytes, "authoritative final".len() as u64);
        assert_eq!(received_deltas, vec!["stream draft"]);
        assert_eq!(completed.response, "authoritative final");
        assert!(!encoded.contains("authoritative final"));
        assert!(!encoded.contains("stream draft"));
        assert!(!encoded.contains("private upstream message"));
        server_task.await.expect("server task");
    }

    #[test]
    fn turn_error_mapping_only_returns_allowlisted_codes() {
        let usage_limit = TurnError {
            message: "private raw message".to_owned(),
            codex_error_info: Some(json!("usageLimitExceeded")),
        };
        assert_eq!(
            safe_turn_error_reason(Some(&usage_limit)),
            "turn_usage_limit_exceeded"
        );
        let connection = TurnError {
            message: "must never escape".to_owned(),
            codex_error_info: Some(json!({
                "httpConnectionFailed": { "httpStatusCode": 401 }
            })),
        };
        assert_eq!(
            safe_turn_error_reason(Some(&connection)),
            "turn_http_connection_failed"
        );
        let workspace = TurnError {
            message: "Not inside a trusted directory".to_owned(),
            codex_error_info: None,
        };
        assert_eq!(
            safe_turn_error_reason(Some(&workspace)),
            "turn_workspace_not_trusted"
        );
        let unknown = TurnError {
            message: "private-unrecognized-error".to_owned(),
            codex_error_info: None,
        };
        assert_eq!(safe_turn_error_reason(Some(&unknown)), "turn_failed_other");
    }
}
