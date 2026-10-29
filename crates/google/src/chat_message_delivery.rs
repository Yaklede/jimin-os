use super::{
    GoogleAuthError, GoogleChatAdapter, GoogleChatMessageReceipt, bounded_body,
    classify_chat_read_status, reminder_receipt, validate_access_token,
    validate_provider_free_text, validated_chat_space_id, validated_chat_thread_segments,
};
use secrecy::SecretString;

struct MessageRequest {
    url: reqwest::Url,
    lookup: reqwest::Url,
    space: String,
    thread: Option<String>,
    body: serde_json::Value,
}

fn message_request(
    space: &str,
    text: &str,
    id: &str,
    thread: Option<&str>,
) -> Result<MessageRequest, GoogleAuthError> {
    let space_id = validated_chat_space_id(space)?;
    if id.len() != 32
        || !id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(GoogleAuthError::InvalidRequest);
    }
    if let Some(thread) = thread {
        validated_chat_thread_segments(thread, space_id)
            .map_err(|_| GoogleAuthError::InvalidRequest)?;
    }
    let text = validate_provider_free_text(text.to_owned(), 32_000)
        .map_err(|_| GoogleAuthError::InvalidRequest)?;
    let mut url = reqwest::Url::parse("https://chat.googleapis.com/v1")
        .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
    url.path_segments_mut()
        .map_err(|()| GoogleAuthError::ProviderUnavailable)?
        .push("spaces")
        .push(space_id)
        .push("messages");
    let message_id = format!("client-jimin-{id}");
    let lookup = reqwest::Url::parse(&format!(
        "https://chat.googleapis.com/v1/{space}/messages/{message_id}"
    ))
    .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
    url.query_pairs_mut()
        .append_pair("requestId", id)
        .append_pair("messageId", &message_id);
    let mut body = serde_json::json!({"text":text});
    if let Some(thread) = thread {
        url.query_pairs_mut()
            .append_pair("messageReplyOption", "REPLY_MESSAGE_OR_FAIL");
        body["thread"] = serde_json::json!({"name":thread});
    }
    Ok(MessageRequest {
        url,
        lookup,
        space: space.into(),
        thread: thread.map(str::to_owned),
        body,
    })
}

impl GoogleChatAdapter {
    /// Creates one named message in the room; retries read back an existing provider message.
    /// # Errors
    /// Returns bounded validation, credential, transport or provider failures.
    pub async fn create_reminder_message(
        &self,
        access: &SecretString,
        space: &str,
        text: &str,
        id: &str,
    ) -> Result<GoogleChatMessageReceipt, GoogleAuthError> {
        self.send_named_message(access, message_request(space, text, id, None)?)
            .await
    }

    /// Replies only to the selected assignment thread with the same durable message identity.
    /// A provider/thread failure never creates a new room message as an implicit fallback.
    /// # Errors
    /// Returns bounded failures, including a conflicting receipt from a different thread.
    pub async fn create_completion_reply(
        &self,
        access: &SecretString,
        space: &str,
        thread: &str,
        text: &str,
        id: &str,
    ) -> Result<GoogleChatMessageReceipt, GoogleAuthError> {
        self.send_named_message(access, message_request(space, text, id, Some(thread))?)
            .await
    }

    async fn send_named_message(
        &self,
        access: &SecretString,
        request: MessageRequest,
    ) -> Result<GoogleChatMessageReceipt, GoogleAuthError> {
        let token = validate_access_token(access)?;
        let response = self
            .client
            .post(request.url)
            .bearer_auth(token)
            .json(&request.body)
            .send()
            .await
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        let response = if response.status().as_u16() == 409 {
            self.client
                .get(request.lookup)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|_| GoogleAuthError::ProviderUnavailable)?
        } else {
            response
        };
        if !response.status().is_success() {
            return Err(classify_chat_read_status(response.status().as_u16()));
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bounded_body(response, 64 * 1024).await?)
                .map_err(|_| GoogleAuthError::ProviderDataInvalid)?;
        let receipt = reminder_receipt(&request.space, &value)?;
        if request
            .thread
            .as_ref()
            .is_some_and(|thread| *thread != receipt.thread_name)
        {
            return Err(GoogleAuthError::ProviderDataInvalid);
        }
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };

    const ID: &str = "019f0000000070008000000000000001";
    const THREAD: &str = "spaces/test/threads/assignment";

    fn server(
        replies: Vec<Option<(u16, serde_json::Value)>>,
    ) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for reply in replies {
                let deadline = Instant::now() + Duration::from_secs(4);
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(value) => break value,
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(e) => panic!("fixture accept: {e}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut part = [0; 2048];
                    let n = stream.read(&mut part).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&part[..n]);
                    assert!(bytes.len() < 65_536);
                    if let Some(end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|s| s.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                if let Some((status, body)) = reply {
                    let body = body.to_string();
                    write!(stream,"HTTP/1.1 {status} fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
                }
            }
            requests
        });
        (origin, handle)
    }

    fn request(origin: &str, thread: Option<&str>) -> MessageRequest {
        let mut request =
            message_request("spaces/test", "일감 · 완료되었어요.", ID, thread).unwrap();
        request.url = reqwest::Url::parse(
            &request
                .url
                .as_str()
                .replace("https://chat.googleapis.com", origin),
        )
        .unwrap();
        request.lookup = reqwest::Url::parse(
            &request
                .lookup
                .as_str()
                .replace("https://chat.googleapis.com", origin),
        )
        .unwrap();
        request
    }
    fn adapter() -> GoogleChatAdapter {
        GoogleChatAdapter {
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            client_id: "fixture".into(),
            client_secret: SecretString::from("fixture-not-a-credential"),
        }
    }
    fn receipt(thread: &str) -> serde_json::Value {
        serde_json::json!({"name":"spaces/test/messages/canonical","thread":{"name":thread}})
    }

    #[tokio::test]
    async fn accepted_but_lost_response_retry_reads_same_message_without_second_creation() {
        // First POST was accepted but its response was lost; retry conflicts and GET recovers it.
        let (origin, handle) = server(vec![
            None,
            Some((409, serde_json::json!({}))),
            Some((200, receipt(THREAD))),
        ]);
        let client = adapter();
        let access = SecretString::from("fixture-access-token");
        assert!(
            client
                .send_named_message(&access, request(&origin, Some(THREAD)))
                .await
                .is_err()
        );
        assert_eq!(
            client
                .send_named_message(&access, request(&origin, Some(THREAD)))
                .await
                .unwrap()
                .thread_name,
            THREAD
        );
        let requests = handle.join().unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests[0], requests[1],
            "retry must retain request ID, message ID, thread and title-only body"
        );
        assert!(requests[0].contains("messageReplyOption=REPLY_MESSAGE_OR_FAIL"));
        assert!(requests[0].contains(&format!("messageId=client-jimin-{ID}")));
        let body: serde_json::Value =
            serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(
            body,
            serde_json::json!({"text":"일감 · 완료되었어요.","thread":{"name":THREAD}})
        );
        assert!(
            requests[2].starts_with(&format!("GET /v1/spaces/test/messages/client-jimin-{ID} "))
        );
    }

    #[tokio::test]
    async fn missing_selected_thread_fails_without_new_room_post() {
        let (origin, handle) = server(vec![Some((404, serde_json::json!({})))]);
        assert!(
            adapter()
                .send_named_message(
                    &SecretString::from("fixture-access-token"),
                    request(&origin, Some(THREAD))
                )
                .await
                .is_err()
        );
        let requests = handle.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].contains("REPLY_MESSAGE_OR_FAIL"));
    }

    #[tokio::test]
    async fn conflicting_receipt_in_another_thread_is_not_acknowledged() {
        let (origin, handle) = server(vec![
            Some((409, serde_json::json!({}))),
            Some((200, receipt("spaces/test/threads/other"))),
        ]);
        assert!(matches!(
            adapter()
                .send_named_message(
                    &SecretString::from("fixture-access-token"),
                    request(&origin, Some(THREAD))
                )
                .await,
            Err(GoogleAuthError::ProviderDataInvalid)
        ));
        assert_eq!(handle.join().unwrap().len(), 2);
    }

    #[test]
    fn room_fallback_has_no_thread_and_validation_cannot_cross_spaces() {
        let request = message_request("spaces/test", "제목 · 완료되었어요.", ID, None).unwrap();
        assert!(request.body.get("thread").is_none());
        assert!(
            !request
                .url
                .query_pairs()
                .any(|(key, _)| key == "messageReplyOption")
        );
        assert!(
            message_request(
                "spaces/test",
                "제목",
                ID,
                Some("spaces/foreign/threads/wrong")
            )
            .is_err()
        );
        assert!(message_request("spaces/test", "제목", "invalid", None).is_err());
    }
}
