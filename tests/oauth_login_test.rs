use discord_rs::protocol::auth::{
    OAuthCallback, OAuthConfig, OAuthPkceFlow, OAuthTokens, PkceChallenge,
};
use discord_rs::protocol::{AuthToken, TokenStorage, TokenType};
use std::fs;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn temp_storage(name: &str) -> TokenStorage {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "discord_rs_oauth_{name}_{}_{}",
        std::process::id(),
        unique
    ));
    fs::create_dir_all(&dir).unwrap();
    TokenStorage::with_path(dir.join("auth.json"))
}

fn config() -> OAuthConfig {
    OAuthConfig::new(
        "synthetic_client_id",
        "http://127.0.0.1:38291/oauth/callback",
    )
    .unwrap()
}

#[test]
fn pkce_matches_rfc7636_vector_and_hides_verifier() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let pkce = PkceChallenge::from_verifier(verifier).unwrap();
    assert_eq!(pkce.challenge(), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    assert_eq!(pkce.verifier(), verifier);

    let debug = format!("{pkce:?}");
    assert!(!debug.contains(verifier));
    assert!(debug.contains("***"));
}

#[test]
fn pkce_rejects_invalid_verifiers() {
    assert!(PkceChallenge::from_verifier("too-short").is_err());
    let invalid_char = "a".repeat(42) + "!";
    assert!(PkceChallenge::from_verifier(invalid_char).is_err());
}

#[test]
fn authorization_url_carries_state_nonce_and_pkce_but_not_verifier() {
    let flow = OAuthPkceFlow::new(config()).unwrap();
    let url = flow.authorization_url();

    assert!(url.starts_with("https://discord.com/oauth2/authorize?"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("client_id=synthetic_client_id"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains(&format!("state={}", flow.state())));
    assert!(url.contains(&format!("nonce={}", flow.nonce())));
    assert!(url.contains(&format!("code_challenge={}", flow.code_challenge())));
    assert!(!url.contains(flow.code_verifier()));
}

#[test]
fn callback_validation_enforces_state_cancellation_and_expiry() {
    let flow = OAuthPkceFlow::new(config()).unwrap();
    let state = flow.state().to_string();

    let valid = OAuthCallback {
        state: Some(state.clone()),
        code: Some("synthetic_authorization_code".to_string()),
        ..Default::default()
    };
    assert_eq!(
        flow.validate_callback(&valid).unwrap().secret(),
        "synthetic_authorization_code"
    );

    let wrong_state = OAuthCallback {
        state: Some("wrong".to_string()),
        code: Some("synthetic_authorization_code".to_string()),
        ..Default::default()
    };
    assert!(flow.validate_callback(&wrong_state).is_err());

    let denied = OAuthCallback {
        state: Some(state.clone()),
        error: Some("access_denied".to_string()),
        ..Default::default()
    };
    assert!(flow.validate_callback(&denied).is_err());

    let mut cancelled = OAuthPkceFlow::new(config()).unwrap();
    cancelled.cancel();
    let callback = OAuthCallback {
        state: Some(cancelled.state().to_string()),
        code: Some("synthetic_authorization_code".to_string()),
        ..Default::default()
    };
    assert!(cancelled.validate_callback(&callback).is_err());

    let expired = OAuthPkceFlow::with_lifetime(config(), Duration::ZERO).unwrap();
    let callback = OAuthCallback {
        state: Some(expired.state().to_string()),
        code: Some("synthetic_authorization_code".to_string()),
        ..Default::default()
    };
    assert!(expired.validate_callback(&callback).is_err());
}

#[tokio::test]
async fn code_exchange_posts_verifier_and_parses_tokens() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0u8; 8192];
        let read = stream.read(&mut buffer).await.unwrap();
        let request = String::from_utf8_lossy(&buffer[..read]).to_string();
        assert!(request.starts_with("POST /token HTTP/1.1"));
        assert!(request.contains("application/x-www-form-urlencoded"));
        assert!(request.contains("grant_type=authorization_code"));
        assert!(request.contains("code_verifier="));
        assert!(request.contains("code=synthetic_authorization_code"));

        let body = r#"{"access_token":"synthetic_access_token_1234567890","token_type":"Bearer","expires_in":3600,"refresh_token":"synthetic_refresh_token_1234567890","scope":"identify guilds"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
    });

    let mut cfg = config();
    cfg.token_endpoint = format!("http://{address}/token");
    let flow = OAuthPkceFlow::new(cfg).unwrap();
    let callback = OAuthCallback {
        state: Some(flow.state().to_string()),
        code: Some("synthetic_authorization_code".to_string()),
        ..Default::default()
    };
    let code = flow.validate_callback(&callback).unwrap();
    let tokens = flow.exchange_code(&code).await.unwrap();

    assert_eq!(tokens.access_token, "synthetic_access_token_1234567890");
    assert_eq!(tokens.token_type, "Bearer");
    assert_eq!(
        tokens.refresh_token.as_deref(),
        Some("synthetic_refresh_token_1234567890")
    );
    let auth = tokens.to_auth_token_at(1_700_000_000);
    assert_eq!(auth.token_type, TokenType::OAuth);
    assert_eq!(auth.authorization_header(), "Bearer synthetic_access_token_1234567890");
    assert_eq!(auth.refresh_token(), Some("synthetic_refresh_token_1234567890"));
    assert_eq!(auth.expires_at(), Some(1_700_003_600));
    assert!(!auth.is_expired_at(1_700_000_001));
    assert!(auth.is_expired_at(1_700_003_600));

    server.await.unwrap();
}

#[test]
fn token_endpoint_validation_rejects_missing_bearer() {
    let mut tokens = OAuthTokens {
        access_token: "synthetic_access_token_1234567890".to_string(),
        token_type: "mac".to_string(),
        expires_in: None,
        refresh_token: None,
        scope: None,
    };
    assert!(tokens.validate().is_err());
    tokens.token_type = "Bearer".to_string();
    assert!(tokens.validate().is_ok());
}

#[test]
fn invalid_token_never_overwrites_saved_credential() {
    let storage = temp_storage("invalid_overwrite");
    let valid = AuthToken::user("valid_synthetic_token_1234567890");
    storage.save(&valid).unwrap();
    let before = fs::read(storage.path()).unwrap();

    let invalid = AuthToken::user("short");
    assert!(storage.save(&invalid).is_err());
    assert_eq!(fs::read(storage.path()).unwrap(), before);
    assert_eq!(
        storage.load().unwrap().unwrap().secret(),
        "valid_synthetic_token_1234567890"
    );
}

#[cfg(unix)]
#[test]
fn overwriting_token_restores_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let storage = temp_storage("permission_overwrite");
    let token = AuthToken::user("valid_synthetic_token_1234567890");
    storage.save(&token).unwrap();
    fs::set_permissions(storage.path(), fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(fs::metadata(storage.path()).unwrap().permissions().mode() & 0o777, 0o644);

    storage.save(&token).unwrap();
    assert_eq!(fs::metadata(storage.path()).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn oauth_token_debug_does_not_leak_access_or_refresh_secret() {
    let auth = AuthToken::oauth_with_refresh(
        "access_secret_1234567890",
        Some("refresh_secret_1234567890".to_string()),
        None,
    );
    let debug = format!("{auth:?}");
    assert!(!debug.contains("access_secret_1234567890"));
    assert!(!debug.contains("refresh_secret_1234567890"));
    assert!(debug.contains("***"));
}
