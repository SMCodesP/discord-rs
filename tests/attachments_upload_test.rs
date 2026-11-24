use discord_rs::domain::settings::NitroTier;
use discord_rs::protocol::{
    guess_content_type, is_spoiler_filename, max_attachment_bytes, strip_spoiler_prefix,
    validate_attachment, validate_attachment_size, validate_attachments, AttachmentUploadError,
    AttachmentUploader, MultipartMessageBuilder, UploadAttachment, BYTES_PER_MIB,
    MAX_ATTACHMENTS_PER_MESSAGE, SPOILER_PREFIX,
};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
struct CapturedRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl CapturedRequest {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Debug, Clone)]
struct MultipartPart {
    name: String,
    filename: Option<String>,
    content_type: Option<String>,
    body: Vec<u8>,
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> std::io::Result<CapturedRequest> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end;
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed before headers",
            ));
        }
        buf.extend_from_slice(&chunk[..read]);
        if let Some(position) = find_subslice(&buf, b"\r\n\r\n") {
            header_end = position + 4;
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

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }

    let content_length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);

    Ok(CapturedRequest {
        method,
        path,
        headers,
        body,
    })
}

async fn spawn_http_server(
    status: &str,
    response_body: serde_json::Value,
) -> anyhow::Result<(
    String,
    Arc<Mutex<Vec<CapturedRequest>>>,
    tokio::task::JoinHandle<()>,
)> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);
    let body = response_body.to_string();
    let status = status.to_string();

    let handle = tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        let Ok(request) = read_http_request(&mut stream).await else {
            return;
        };
        captured_clone.lock().unwrap().push(request);

        let response = format!(
            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            body.as_bytes().len(),
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;
    });

    Ok((format!("http://{}", address), captured, handle))
}

fn boundary_from_content_type(content_type: &str) -> String {
    content_type
        .strip_prefix("multipart/form-data; boundary=")
        .unwrap_or_else(|| panic!("content type multipart inesperado: {content_type}"))
        .to_string()
}

fn parse_multipart(body: &[u8], boundary: &str) -> Vec<MultipartPart> {
    let delimiter = format!("--{boundary}");
    let mut parts = Vec::new();
    let mut position = 0;

    while let Some(offset) = find_subslice(&body[position..], delimiter.as_bytes()) {
        let segment_start = position + offset + delimiter.len();
        let segment_end = match find_subslice(&body[segment_start..], delimiter.as_bytes()) {
            Some(next) => segment_start + next,
            None => break,
        };
        let mut segment = &body[segment_start..segment_end];
        segment = segment.strip_prefix(b"\r\n").unwrap_or(segment);
        segment = segment.strip_suffix(b"\r\n").unwrap_or(segment);

        if segment.is_empty() || segment == b"--" {
            position = segment_end;
            continue;
        }

        let Some(header_end) = find_subslice(segment, b"\r\n\r\n") else {
            position = segment_end;
            continue;
        };
        let header_text = String::from_utf8_lossy(&segment[..header_end]).to_string();
        let part_body = segment[header_end + 4..].to_vec();

        let mut name = String::new();
        let mut filename = None;
        let mut content_type = None;
        for line in header_text.lines() {
            if let Some(disposition) = line.strip_prefix("Content-Disposition:") {
                name = disposition_attribute(disposition, "name").unwrap_or_default();
                filename = disposition_attribute(disposition, "filename");
            } else if let Some(value) = line.strip_prefix("Content-Type:") {
                content_type = Some(value.trim().to_string());
            }
        }

        parts.push(MultipartPart {
            name,
            filename,
            content_type,
            body: part_body,
        });
        position = segment_end;
    }

    parts
}

fn disposition_attribute(disposition: &str, attribute: &str) -> Option<String> {
    let needle = format!("{}=\"", attribute);
    let start = disposition.find(&needle)? + needle.len();
    let end = disposition[start..].find('"')? + start;
    Some(disposition[start..end].to_string())
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn small_attachment(filename: &str, contents: &[u8]) -> UploadAttachment {
    UploadAttachment::from_bytes(filename, contents.to_vec()).unwrap()
}

#[test]
fn builder_emits_payload_json_and_file_parts() {
    let attachment = UploadAttachment::new("cat.png", "image/png", vec![0u8, 1, 2, 255])
        .unwrap()
        .with_description("um gato");
    let message = MultipartMessageBuilder::new()
        .content("olá")
        .attachment(attachment)
        .build(NitroTier::None)
        .unwrap();

    assert!(message.content_type.starts_with("multipart/form-data; boundary="));
    let boundary = boundary_from_content_type(&message.content_type);
    assert_eq!(message.boundary, boundary);

    let parts = parse_multipart(&message.body, &boundary);
    assert_eq!(parts.len(), 2, "payload_json + um arquivo");

    assert_eq!(parts[0].name, "payload_json");
    assert_eq!(parts[1].name, "files[0]");
    assert_eq!(parts[1].filename.as_deref(), Some("cat.png"));
    assert_eq!(parts[1].content_type.as_deref(), Some("image/png"));
    assert_eq!(parts[1].body, vec![0, 1, 2, 255]);

    let payload: serde_json::Value = serde_json::from_slice(&parts[0].body).unwrap();
    assert_eq!(payload["content"], "olá");
    assert_eq!(payload["attachments"][0]["id"], 0);
    assert_eq!(payload["attachments"][0]["filename"], "cat.png");
    assert_eq!(payload["attachments"][0]["description"], "um gato");

    let closing = format!("--{boundary}--\r\n");
    assert!(message.body.ends_with(closing.as_bytes()));
}

#[test]
fn spoiler_prefix_is_applied_without_duplication() {
    let base = small_attachment("cat.png", &[1, 2, 3]);
    assert!(!base.is_spoiler());
    assert_eq!(base.display_filename(), "cat.png");

    let hidden = base.clone().with_spoiler(true).unwrap();
    assert_eq!(hidden.filename, format!("{SPOILER_PREFIX}cat.png"));
    assert!(hidden.is_spoiler());
    assert!(is_spoiler_filename(&hidden.filename));
    assert_eq!(hidden.display_filename(), "cat.png");
    assert_eq!(strip_spoiler_prefix(&hidden.filename), "cat.png");

    let hidden_again = hidden.clone().with_spoiler(true).unwrap();
    assert_eq!(
        hidden_again.filename,
        format!("{SPOILER_PREFIX}cat.png"),
        "não pode acumular prefixos"
    );

    let visible = hidden_again.clone().with_spoiler(false).unwrap();
    assert_eq!(visible.filename, "cat.png");
    assert!(!visible.is_spoiler());

    let message = MultipartMessageBuilder::new()
        .attachments(vec![hidden_again])
        .build(NitroTier::None)
        .unwrap();
    let boundary = boundary_from_content_type(&message.content_type);
    let parts = parse_multipart(&message.body, &boundary);
    let expected = format!("{SPOILER_PREFIX}cat.png");
    assert_eq!(parts[1].filename.as_deref(), Some(expected.as_str()));
    let payload: serde_json::Value = serde_json::from_slice(&parts[0].body).unwrap();
    assert_eq!(payload["attachments"][0]["filename"], expected);
    assert!(payload.get("content").is_none(), "conteúdo vazio não é enviado");
}

#[test]
fn size_limits_follow_nitro_tier() {
    assert_eq!(max_attachment_bytes(NitroTier::None), 25 * BYTES_PER_MIB);
    assert_eq!(max_attachment_bytes(NitroTier::Basic), 50 * BYTES_PER_MIB);
    assert_eq!(max_attachment_bytes(NitroTier::Full), 500 * BYTES_PER_MIB);

    for (tier, megabytes) in [
        (NitroTier::None, 25u64),
        (NitroTier::Basic, 50u64),
        (NitroTier::Full, 500u64),
    ] {
        let limit = megabytes * BYTES_PER_MIB;
        assert!(validate_attachment_size(tier, limit).is_ok());
        let error = validate_attachment_size(tier, limit + 1).unwrap_err();
        assert_eq!(
            error,
            AttachmentUploadError::AttachmentTooLarge {
                size: limit + 1,
                limit,
                tier,
            }
        );
    }
}

#[test]
fn rejects_more_than_ten_attachments() {
    let attachments = (0..=MAX_ATTACHMENTS_PER_MESSAGE)
        .map(|index| small_attachment(&format!("arquivo-{index}.txt"), &[index as u8]))
        .collect::<Vec<_>>();
    let error = validate_attachments(&attachments, NitroTier::Full).unwrap_err();
    assert_eq!(
        error,
        AttachmentUploadError::TooManyAttachments {
            count: MAX_ATTACHMENTS_PER_MESSAGE + 1,
            limit: MAX_ATTACHMENTS_PER_MESSAGE,
        }
    );

    let allowed = attachments[..MAX_ATTACHMENTS_PER_MESSAGE].to_vec();
    assert_eq!(validate_attachments(&allowed, NitroTier::Full), Ok(()));
}

#[test]
fn rejects_invalid_names_content_types_and_empty_files() {
    assert_eq!(
        UploadAttachment::new("", "image/png", vec![1]).unwrap_err(),
        AttachmentUploadError::EmptyFilename
    );
    assert_eq!(
        UploadAttachment::new("folder/cat.png", "image/png", vec![1]).unwrap_err(),
        AttachmentUploadError::InvalidFilename("folder/cat.png".to_string())
    );
    assert_eq!(
        UploadAttachment::new("cat\r\nX-Injected: 1.png", "image/png", vec![1]).unwrap_err(),
        AttachmentUploadError::InvalidFilename("cat\r\nX-Injected: 1.png".to_string())
    );
    assert_eq!(
        UploadAttachment::new("cat.png", "", vec![1]).unwrap_err(),
        AttachmentUploadError::EmptyContentType
    );
    assert_eq!(
        UploadAttachment::new("cat.png", "image/png\r\nX-Injected: 1", vec![1]).unwrap_err(),
        AttachmentUploadError::InvalidContentType
    );
    assert_eq!(
        UploadAttachment::new(SPOILER_PREFIX, "image/png", vec![1]).unwrap_err(),
        AttachmentUploadError::InvalidFilename(SPOILER_PREFIX.to_string())
    );

    let empty = UploadAttachment::new("vazio.txt", "text/plain", Vec::<u8>::new()).unwrap();
    assert_eq!(
        validate_attachment(&empty, NitroTier::Full).unwrap_err(),
        AttachmentUploadError::EmptyFile {
            filename: "vazio.txt".to_string()
        }
    );

    let error = MultipartMessageBuilder::new()
        .build(NitroTier::Full)
        .unwrap_err();
    assert_eq!(error, AttachmentUploadError::NoAttachments);
}

#[test]
fn infers_content_type_from_extension() {
    assert_eq!(guess_content_type("FOTO.PNG"), "image/png");
    assert_eq!(guess_content_type("foto.jpeg"), "image/jpeg");
    assert_eq!(guess_content_type("clip.webm"), "video/webm");
    assert_eq!(guess_content_type("voz.ogg"), "audio/ogg");
    assert_eq!(guess_content_type("notas.txt"), "text/plain");
    assert_eq!(
        guess_content_type("arquivo.sem-extensao"),
        "application/octet-stream"
    );
    assert_eq!(guess_content_type("animacao.gif"), "image/gif");

    let attachment = small_attachment("animacao.gif", &[0x47, 0x49, 0x46]);
    assert_eq!(attachment.content_type, "image/gif");
}

#[test]
fn rejects_invalid_boundary_and_uses_explicit_boundary() {
    let attachment = small_attachment("a.txt", b"a");
    let error = MultipartMessageBuilder::new()
        .attachment(attachment.clone())
        .with_boundary("invalido com espaço")
        .build(NitroTier::None)
        .unwrap_err();
    assert_eq!(
        error,
        AttachmentUploadError::InvalidBoundary("invalido com espaço".to_string())
    );

    let error = MultipartMessageBuilder::new()
        .attachment(attachment.clone())
        .with_boundary("linha\r\nquebrada")
        .build(NitroTier::None)
        .unwrap_err();
    assert_eq!(
        error,
        AttachmentUploadError::InvalidBoundary("linha\r\nquebrada".to_string())
    );

    let message = MultipartMessageBuilder::new()
        .content("ok")
        .attachment(attachment)
        .with_boundary("DiscordRsBoundary123")
        .build(NitroTier::None)
        .unwrap();
    assert_eq!(
        message.content_type,
        "multipart/form-data; boundary=DiscordRsBoundary123"
    );
    assert_eq!(message.boundary, "DiscordRsBoundary123");
}

#[tokio::test]
async fn uploader_posts_multipart_and_reads_confirmed_response() {
    let response = serde_json::json!({
        "id": "42",
        "channel_id": "99",
        "content": "hello",
        "attachments": [{
            "id": "7",
            "filename": "note.txt",
            "size": 5,
            "url": "https://cdn.discordapp.com/attachments/99/7/note.txt",
            "content_type": "text/plain"
        }]
    });
    let (base, captured, server) = spawn_http_server("200 OK", response).await.unwrap();
    let uploader = AttachmentUploader::new(reqwest::Client::new(), base, "Bot test-token");
    let attachment = small_attachment("note.txt", b"hello");

    let message = uploader
        .send_message("99", "hello", NitroTier::None, vec![attachment])
        .await
        .unwrap();
    server.abort();

    assert_eq!(message.id, "42");
    assert_eq!(message.channel_id, "99");
    assert_eq!(message.content, "hello");
    assert_eq!(message.attachments.len(), 1);
    assert_eq!(message.attachments[0].id, "7");
    assert_eq!(message.attachments[0].filename, "note.txt");
    assert_eq!(message.attachments[0].size, 5);
    assert_eq!(
        message.attachments[0].content_type.as_deref(),
        Some("text/plain")
    );

    let requests = captured.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/channels/99/messages");
    assert_eq!(request.header("authorization"), Some("Bot test-token"));
    let content_type = request.header("content-type").unwrap();
    let boundary = boundary_from_content_type(content_type);

    let parts = parse_multipart(&request.body, &boundary);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].name, "payload_json");
    assert_eq!(parts[1].name, "files[0]");
    assert_eq!(parts[1].filename.as_deref(), Some("note.txt"));
    assert_eq!(parts[1].body, b"hello");

    let payload: serde_json::Value = serde_json::from_slice(&parts[0].body).unwrap();
    assert_eq!(payload["content"], "hello");
    assert_eq!(payload["attachments"][0]["id"], 0);
    assert_eq!(payload["attachments"][0]["filename"], "note.txt");
}

#[tokio::test]
async fn uploader_does_not_report_success_on_http_error() {
    let response = serde_json::json!({
        "message": "Missing Access",
        "code": 50001
    });
    let (base, captured, server) =
        spawn_http_server("403 Forbidden", response).await.unwrap();
    let uploader = AttachmentUploader::new(reqwest::Client::new(), base, "Bot test-token");
    let attachment = small_attachment("negado.txt", b"blocked");

    let error = uploader
        .send_message("99", "", NitroTier::None, vec![attachment])
        .await
        .unwrap_err();
    server.abort();

    let description = error.to_string();
    assert!(description.contains("403"), "{description}");
    assert!(description.contains("Missing Access"), "{description}");

    let requests = captured.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
}

#[tokio::test]
async fn uploader_rejects_oversized_file_before_network() {
    let oversized = vec![0u8; (25 * BYTES_PER_MIB + 1) as usize];
    let attachment = UploadAttachment::new("grande.bin", "application/octet-stream", oversized)
        .unwrap();
    let uploader = AttachmentUploader::new(
        reqwest::Client::new(),
        "http://127.0.0.1:1",
        "Bot test-token",
    );

    let error = uploader
        .send_message("99", "", NitroTier::None, vec![attachment])
        .await
        .unwrap_err();
    assert!(error.to_string().contains("excede o limite"), "{error}");
}
