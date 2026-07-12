use std::collections::VecDeque;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufRead, AsyncWrite};

use crate::codec::JsonLineTransport;
use crate::error::{Error, Result};

const MAX_QUEUED_NOTIFICATIONS: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Notification {
    pub(crate) method: String,
    pub(crate) params: Option<Value>,
}

#[derive(Serialize)]
struct Request<'a, P> {
    id: u64,
    method: &'a str,
    params: P,
}

#[derive(Serialize)]
struct ClientNotification<'a> {
    method: &'a str,
}

#[derive(Serialize)]
struct ServerResponse<'a> {
    id: &'a Value,
    result: &'a Value,
}

struct ServerRequest {
    id: Value,
    method: String,
}

pub(crate) struct RpcConnection<R, W> {
    transport: JsonLineTransport<R, W>,
    next_request_id: u64,
    queued_notifications: VecDeque<Notification>,
}

impl<R, W> RpcConnection<R, W>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub(crate) fn new(transport: JsonLineTransport<R, W>) -> Self {
        Self {
            transport,
            next_request_id: 1,
            queued_notifications: VecDeque::new(),
        }
    }

    pub(crate) async fn request<P, T>(&mut self, method: &'static str, params: P) -> Result<T>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        let request_id = self.next_request_id;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or(Error::InvalidProtocolMessage)?;

        self.transport
            .write(&Request {
                id: request_id,
                method,
                params,
            })
            .await?;

        loop {
            let value = self.transport.read_value().await?;
            match classify(&value)? {
                Incoming::Notification(notification) => {
                    if self.queued_notifications.len() >= MAX_QUEUED_NOTIFICATIONS {
                        return Err(Error::NotificationBackpressure);
                    }
                    self.queued_notifications.push_back(notification);
                }
                Incoming::ServerRequest(request) => {
                    self.decline_server_request(request).await?;
                }
                Incoming::Response {
                    id,
                    result,
                    error_code,
                } => {
                    if id != request_id {
                        return Err(Error::UnknownResponseId);
                    }
                    if let Some(code) = error_code {
                        return Err(Error::Rpc { code });
                    }
                    let result = result.ok_or(Error::InvalidProtocolMessage)?;
                    return serde_json::from_value(result)
                        .map_err(|_| Error::InvalidResponse { method });
                }
            }
        }
    }

    pub(crate) async fn notify(&mut self, method: &'static str) -> Result<()> {
        self.transport.write(&ClientNotification { method }).await
    }

    pub(crate) async fn next_notification(&mut self) -> Result<Notification> {
        loop {
            if let Some(notification) = self.queued_notifications.pop_front() {
                return Ok(notification);
            }

            let value = self.transport.read_value().await?;
            match classify(&value)? {
                Incoming::Notification(notification) => return Ok(notification),
                Incoming::ServerRequest(request) => {
                    self.decline_server_request(request).await?;
                }
                Incoming::Response { .. } => return Err(Error::UnknownResponseId),
            }
        }
    }

    async fn decline_server_request(&mut self, request: ServerRequest) -> Result<()> {
        let result = safe_decline_result(&request.method).ok_or(Error::UnexpectedServerRequest)?;
        self.transport
            .write(&ServerResponse {
                id: &request.id,
                result: &result,
            })
            .await
    }
}

enum Incoming {
    Notification(Notification),
    ServerRequest(ServerRequest),
    Response {
        id: u64,
        result: Option<Value>,
        error_code: Option<i64>,
    },
}

fn classify(value: &Value) -> Result<Incoming> {
    let object = value.as_object().ok_or(Error::InvalidProtocolMessage)?;
    let method = object.get("method").and_then(Value::as_str);
    let id = object.get("id");

    match (method, id) {
        (Some(method), None) => Ok(Incoming::Notification(Notification {
            method: method.to_owned(),
            params: object.get("params").cloned(),
        })),
        (Some(method), Some(id)) if valid_server_request_id(id) => {
            Ok(Incoming::ServerRequest(ServerRequest {
                id: id.clone(),
                method: method.to_owned(),
            }))
        }
        (Some(_), Some(_)) | (None, None) => Err(Error::InvalidProtocolMessage),
        (None, Some(id)) => classify_response(object, id),
    }
}

fn safe_decline_result(method: &str) -> Option<Value> {
    match method {
        "item/commandExecution/requestApproval"
        | "item/fileChange/requestApproval"
        | "applyPatchApproval"
        | "execCommandApproval" => Some(json!({ "decision": "decline" })),
        "item/permissions/requestApproval" => Some(json!({ "permissions": {} })),
        "item/tool/requestUserInput" => Some(json!({ "answers": {} })),
        "mcpServer/elicitation/request" => Some(json!({ "action": "decline", "content": null })),
        "item/tool/call" => Some(json!({ "success": false, "contentItems": [] })),
        _ => None,
    }
}

fn valid_server_request_id(value: &Value) -> bool {
    value.as_i64().is_some()
        || value.as_u64().is_some()
        || value.as_str().is_some_and(|id| {
            !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
        })
}

fn classify_response(object: &Map<String, Value>, id: &Value) -> Result<Incoming> {
    let id = id.as_u64().ok_or(Error::UnknownResponseId)?;
    let result = object.get("result").cloned();
    let error_code = object
        .get("error")
        .and_then(Value::as_object)
        .and_then(|error| error.get("code"))
        .and_then(Value::as_i64);

    if result.is_some() == error_code.is_some() {
        return Err(Error::InvalidProtocolMessage);
    }

    Ok(Incoming::Response {
        id,
        result,
        error_code,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, split};

    use super::RpcConnection;
    use crate::codec::JsonLineTransport;
    use crate::error::Error;

    #[tokio::test]
    async fn correlates_response_and_queues_earlier_notification() {
        let (client, server) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client);
        let transport = JsonLineTransport::new(BufReader::new(client_reader), client_writer, 4096);
        let mut connection = RpcConnection::new(transport);

        let server_task = tokio::spawn(async move {
            let (server_reader, mut server_writer) = split(server);
            let mut server_reader = BufReader::new(server_reader);
            let mut request = String::new();
            server_reader
                .read_line(&mut request)
                .await
                .expect("request");
            server_writer
                .write_all(b"{\"method\":\"future/event\",\"params\":{\"ignored\":true}}\n")
                .await
                .expect("notification");
            server_writer
                .write_all(b"{\"id\":1,\"result\":{\"ok\":true}}\n")
                .await
                .expect("response");
        });

        let result: Value = connection
            .request("test/read", json!({}))
            .await
            .expect("correlated response");
        assert_eq!(result, json!({"ok": true}));
        let notification = connection
            .next_notification()
            .await
            .expect("queued notification");
        assert_eq!(notification.method, "future/event");
        server_task.await.expect("server task");
    }

    #[tokio::test]
    async fn declines_a_command_approval_before_returning_the_original_response() {
        let (client, server) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client);
        let transport = JsonLineTransport::new(BufReader::new(client_reader), client_writer, 4096);
        let mut connection = RpcConnection::new(transport);

        let server_task = tokio::spawn(async move {
            let (server_reader, mut server_writer) = split(server);
            let mut server_reader = BufReader::new(server_reader);
            let mut request = String::new();
            server_reader
                .read_line(&mut request)
                .await
                .expect("client request");
            server_writer
                .write_all(
                    b"{\"id\":\"approval-1\",\"method\":\"item/commandExecution/requestApproval\",\"params\":{}}\n",
                )
                .await
                .expect("server request");
            let mut decision = String::new();
            server_reader
                .read_line(&mut decision)
                .await
                .expect("client decision");
            assert_eq!(
                serde_json::from_str::<Value>(&decision).expect("decision json"),
                json!({"id": "approval-1", "result": {"decision": "decline"}})
            );
            server_writer
                .write_all(b"{\"id\":1,\"result\":{\"ok\":true}}\n")
                .await
                .expect("response");
        });

        let result: Value = connection
            .request("test/read", json!({}))
            .await
            .expect("response after declined approval");
        assert_eq!(result, json!({"ok": true}));
        server_task.await.expect("server task");
    }

    #[tokio::test]
    async fn rejects_unknown_response_id() {
        let (client, server) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client);
        let transport = JsonLineTransport::new(BufReader::new(client_reader), client_writer, 4096);
        let mut connection = RpcConnection::new(transport);

        let server_task = tokio::spawn(async move {
            let (server_reader, mut server_writer) = split(server);
            let mut server_reader = BufReader::new(server_reader);
            let mut request = String::new();
            server_reader
                .read_line(&mut request)
                .await
                .expect("request");
            server_writer
                .write_all(b"{\"id\":99,\"result\":{}}\n")
                .await
                .expect("response");
        });

        let result: Result<Value, Error> = connection.request("test/read", json!({})).await;
        assert!(matches!(result, Err(Error::UnknownResponseId)));
        server_task.await.expect("server task");
    }
}
