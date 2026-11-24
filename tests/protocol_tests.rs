use discord_rs::protocol::*;
use gpui_kit::http_client::{AsyncBody, HttpClient, http};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn read_http_request_head(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
    }
    Ok(())
}

async fn spawn_one_shot_http_server(
    response: Option<String>,
) -> anyhow::Result<(String, tokio::task::JoinHandle<std::io::Result<()>>)> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        read_http_request_head(&mut stream).await?;
        if let Some(response) = response {
            stream.write_all(response.as_bytes()).await?;
            stream.flush().await?;
        }
        // Sem resposta: derruba a conexão para exercitar erro de rede real em loopback.
        Ok(())
    });
    Ok((format!("http://{}", address), handle))
}

#[tokio::test]
async fn test_mock_provider_guilds_and_channels() {
    let provider = MockDiscordProvider::new();
    let guilds = provider.list_guilds().await.expect("Failed to list guilds");

    assert_eq!(guilds.len(), 2);
    let rust_br = guilds.iter().find(|g| g.id == "guild-rust-br").expect("Rust Brasil missing");
    assert_eq!(rust_br.name, "Rust Brasil");
    assert_eq!(rust_br.categories.len(), 3);
    assert_eq!(rust_br.channels.len(), 6);

    let gpui_hub = guilds.iter().find(|g| g.id == "guild-gpui-hub").expect("GPUI Hub missing");
    assert_eq!(gpui_hub.name, "GPUI Hub");
    assert_eq!(gpui_hub.channels.len(), 6);

    let channels = provider.list_channels("guild-rust-br").await.expect("Failed to list channels");
    assert_eq!(channels.len(), 6);
    assert!(channels.iter().any(|c| c.name == "geral" && c.is_text()));
    assert!(channels.iter().any(|c| c.name == "Voz Geral" && c.is_voice()));
    assert!(channels.iter().any(|c| c.name == "boas-vindas" && c.is_announcement()));
}

#[tokio::test]
async fn test_mock_provider_messaging_and_reactions() {
    let provider = MockDiscordProvider::new();
    let mut rx = provider.subscribe_events();

    // Verify initial message history
    let messages = provider
        .get_messages("chan-br-general", 50)
        .await
        .expect("Failed to get messages");
    assert!(messages.len() >= 4);

    // Send a message
    let new_msg = provider
        .send_message("chan-br-general", "Test message from integration test")
        .await
        .expect("Failed to send message");
    assert_eq!(new_msg.content, "Test message from integration test");
    assert_eq!(new_msg.channel_id, "chan-br-general");

    // Event should be received
    let event = rx.try_recv().expect("Expected MessageCreate event");
    match event {
        TransportEvent::MessageCreate(m) => {
            assert_eq!(m.id, new_msg.id);
            assert_eq!(m.content, "Test message from integration test");
        }
        other => panic!("Unexpected event received: {:?}", other),
    }

    // Add reaction
    provider
        .add_reaction("chan-br-general", &new_msg.id, "🦀")
        .await
        .expect("Failed to add reaction");

    let rx_event = rx.try_recv().expect("Expected ReactionAdd event");
    match rx_event {
        TransportEvent::ReactionAdd { emoji, message_id, .. } => {
            assert_eq!(emoji, "🦀");
            assert_eq!(message_id, new_msg.id);
        }
        other => panic!("Unexpected event received: {:?}", other),
    }

    // Remove reaction
    provider
        .remove_reaction("chan-br-general", &new_msg.id, "🦀")
        .await
        .expect("Failed to remove reaction");

    let rx_rm_event = rx.try_recv().expect("Expected ReactionRemove event");
    match rx_rm_event {
        TransportEvent::ReactionRemove { emoji, message_id, .. } => {
            assert_eq!(emoji, "🦀");
            assert_eq!(message_id, new_msg.id);
        }
        other => panic!("Unexpected event received: {:?}", other),
    }

    // Send sticker
    let sticker_msg = provider
        .send_sticker("chan-br-general", "sticker-12345")
        .await
        .expect("Failed to send sticker");
    assert!(sticker_msg.is_sticker());
    assert_eq!(sticker_msg.sticker_id(), Some("sticker-12345".to_string()));
    assert_eq!(sticker_msg.sticker_name(), Some("mock-sticker".to_string()));

    let rx_sticker_event = rx.try_recv().expect("Expected MessageCreate event for sticker");
    match rx_sticker_event {
        TransportEvent::MessageCreate(m) => {
            assert_eq!(m.id, sticker_msg.id);
            assert!(m.is_sticker());
            assert_eq!(m.sticker_id(), Some("sticker-12345".to_string()));
        }
        other => panic!("Unexpected event received: {:?}", other),
    }
}

#[test]
fn test_auth_token_masking_and_headers() {
    let bot_token = AuthToken::bot("MTA2MjAwMTIx.G2x...SECRET_12345678");
    assert_eq!(
        bot_token.authorization_header(),
        "Bot MTA2MjAwMTIx.G2x...SECRET_12345678"
    );

    let debug_repr = format!("{:?}", bot_token);
    assert!(!debug_repr.contains("SECRET_12345678"));
    assert!(debug_repr.contains("***"));

    let user_token = AuthToken::user("USER_SECRET_TOKEN_XYZ_12345678");
    assert_eq!(
        user_token.authorization_header(),
        "USER_SECRET_TOKEN_XYZ_12345678"
    );
    assert!(user_token.is_valid_format());
}

#[tokio::test]
async fn test_http_adapter_surfaces_connection_drop_without_fake_response() -> anyhow::Result<()> {
    let (base_url, server) = spawn_one_shot_http_server(None).await?;
    let reqwest_client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;
    let client = ReqwestHttpClient::with_client(reqwest_client);
    let request = http::Request::builder()
        .method("GET")
        .uri(format!("{}/users/@me", base_url))
        .body(AsyncBody::from_bytes(Vec::new().into()))?;

    let result = tokio::time::timeout(std::time::Duration::from_secs(3), client.send(request))
        .await
        .expect("o adapter HTTP deve responder com erro após a conexão ser derrubada");
    assert!(
        result.is_err(),
        "queda de rede não pode virar resposta de sucesso"
    );
    tokio::time::timeout(std::time::Duration::from_secs(1), server)
        .await?
        .expect("servidor local deve concluir sem erro")?;
    Ok(())
}

#[tokio::test]
async fn test_http_adapter_preserves_429_headers_for_rate_limiter() -> anyhow::Result<()> {
    let response = concat!(
        "HTTP/1.1 429 Too Many Requests\r\n",
        "Content-Length: 0\r\n",
        "Retry-After: 0.08\r\n",
        "X-RateLimit-Global: true\r\n",
        "X-RateLimit-Limit: 5\r\n",
        "X-RateLimit-Remaining: 0\r\n",
        "X-RateLimit-Reset-After: 0.08\r\n",
        "Connection: close\r\n\r\n",
    );
    let (base_url, server) = spawn_one_shot_http_server(Some(response.to_string())).await?;
    let reqwest_client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;
    let client = ReqwestHttpClient::with_client(reqwest_client);
    let request = http::Request::builder()
        .method("GET")
        .uri(format!("{}/users/@me", base_url))
        .body(AsyncBody::from_bytes(Vec::new().into()))?;

    let response = client.send(request).await?;
    assert_eq!(response.status().as_u16(), 429);
    assert_eq!(
        response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok()),
        Some("0.08")
    );

    // O header real do socket local alimenta o coordenador de produção.
    let manager = RateLimitManager::new();
    manager
        .update_from_headers("/users/@me", response.headers())
        .await;
    let started = std::time::Instant::now();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        manager.acquire_permit("/guilds/999999"),
    )
    .await
    .expect("o limite global vindo do HTTP 429 deve expirar");
    assert!(started.elapsed() >= std::time::Duration::from_millis(50));

    tokio::time::timeout(std::time::Duration::from_secs(1), server)
        .await?
        .expect("servidor local deve concluir sem erro")?;
    Ok(())
}
