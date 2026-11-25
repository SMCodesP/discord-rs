use discord_rs::domain::message::{Author, Message, MessageReference};
use discord_rs::protocol::{AuthToken, DiscordApiClient, DiscordTransport};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug)]
struct CapturedRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> std::io::Result<CapturedRequest> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end;
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "closed"));
        }
        buf.extend_from_slice(&chunk[..read]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }
    let header_text = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = header_text.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/");
    let path = target.split('?').next().unwrap_or(target).to_string();
    let mut content_length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 { break; }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);
    Ok(CapturedRequest { method, path, body })
}

async fn spawn_server(response_body: Value) -> anyhow::Result<(String, tokio::task::JoinHandle<CapturedRequest>)> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    let body = response_body.to_string();
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let req = read_http_request(&mut stream).await.expect("read");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await.expect("write");
        stream.flush().await.expect("flush");
        req
    });
    Ok((format!("http://{}", addr), handle))
}

#[tokio::test]
async fn send_message_with_reference_posts_message_reference_and_allowed_mentions() {
    let response = serde_json::json!({
        "id": "2001",
        "channel_id": "1001",
        "content": "reply body",
        "timestamp": "2026-01-01T00:00:00Z",
        "author": {"id": "u1", "username": "tester"},
        "message_reference": {"message_id": "1999", "channel_id": "1001"},
        "referenced_message": {
            "id": "1999",
            "channel_id": "1001",
            "content": "original",
            "timestamp": "2026-01-01T00:00:00Z",
            "author": {"id": "u2", "username": "orig"}
        }
    });
    let (base, handle) = spawn_server(response).await.expect("server");
    let client = DiscordApiClient::with_base_url(AuthToken::user("test-token"), base);
    let reference = MessageReference::reply("1999");

    let sent = client
        .send_message_with_reference("1001", "reply body", Some(&reference), false)
        .await
        .expect("send");

    let req = handle.await.expect("join");
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/channels/1001/messages");
    let body: Value = serde_json::from_slice(&req.body).expect("json");
    assert_eq!(body["content"], "reply body");
    assert_eq!(body["message_reference"]["message_id"], "1999");
    assert_eq!(body["message_reference"]["fail_if_not_exists"], false);
    assert_eq!(body["allowed_mentions"]["replied_user"], false);
    assert_eq!(body["allowed_mentions"]["parse"], serde_json::json!([]));

    assert_eq!(sent.message_reference.as_ref().and_then(|r| r.message_id.as_deref()), Some("1999"));
    assert_eq!(sent.referenced_message.as_ref().map(|m| m.id.as_str()), Some("1999"));
}

#[test]
fn parse_message_from_json_preserves_reference_fields() {
    let raw = serde_json::json!({
        "id": "2002",
        "channel_id": "1001",
        "content": "reply",
        "timestamp": "2026-01-01T00:00:00Z",
        "author": {"id": "u1", "username": "tester"},
        "message_reference": {"message_id": "1998", "channel_id": "1001", "type": 0},
        "referenced_message": {
            "id": "1998",
            "channel_id": "1001",
            "content": "orig",
            "timestamp": "2026-01-01T00:00:00Z",
            "author": {"id": "u2", "username": "orig"}
        }
    });
    let msg = discord_rs::protocol::discord_api::parse_message_from_json_str(&raw.to_string()).expect("parse");
    assert_eq!(msg.message_reference.as_ref().and_then(|r| r.message_id.as_deref()), Some("1998"));
    assert_eq!(msg.referenced_message.as_ref().map(|m| m.content.as_str()), Some("orig"));
}

#[test]
fn begin_message_send_with_reference_attaches_reply_preview() {
    let mut state = discord_rs::app::state::AppState::new();
    let (cm, _rx) = discord_rs::protocol::ClientManager::new();
    state.client_manager = Some(cm);
    let channel_id = "chan-1".to_string();
    let original = Message::new("orig-1", channel_id.clone(), Author::new("u2", "orig"), "original");
    state.messages_by_channel.insert(channel_id.clone(), vec![original]);

    let temp_id = state
        .begin_message_send_with_reference(&channel_id, "reply", Some(MessageReference::reply("orig-1")))
        .expect("begin");
    let msg = state.messages_by_channel.get(&channel_id).and_then(|msgs| msgs.iter().find(|m| m.id == temp_id)).expect("msg");
    assert_eq!(msg.message_reference.as_ref().and_then(|r| r.message_id.as_deref()), Some("orig-1"));
    assert_eq!(msg.referenced_message.as_ref().map(|m| m.content.as_str()), Some("original"));
}
