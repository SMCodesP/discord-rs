//! Discord Remote Auth v2 (QR Code Login) — `wss://remote-auth-gateway.discord.gg/?v=2`.
//!
//! Generates an ephemeral RSA-2048 keypair locally, performs the official Discord
//! Remote Auth v2 handshake (`hello` -> `init` -> `nonce_proof` -> `pending_remote_init`
//! -> `pending_ticket` -> `pending_login`), exchanges the ticket at
//! `POST /api/v9/users/@me/remote-auth/login`, and decrypts the freshly minted
//! user token locally with RSA-OAEP-SHA256. Zero mocks, zero shared browser sessions.

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD as B64_STD, URL_SAFE_NO_PAD as B64_URL};
use futures::{SinkExt, StreamExt};
use qrcode::{Color, QrCode};
use rand::rngs::OsRng;
use rsa::pkcs8::EncodePublicKey;
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::protocol::discord_api::{DISCORD_DESKTOP_USER_AGENT, build_super_properties_b64};

const REMOTE_AUTH_WS_URL: &str = "wss://remote-auth-gateway.discord.gg/?v=2";
const REMOTE_AUTH_LOGIN_URL: &str = "https://discord.com/api/v9/users/@me/remote-auth/login";

/// Precomputed boolean QR code matrix ready for native GPUI rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrMatrix {
    pub width: usize,
    pub modules: Vec<bool>,
}

impl QrMatrix {
    pub fn from_url(url: &str) -> Result<Self> {
        let code = QrCode::with_error_correction_level(url.as_bytes(), qrcode::EcLevel::H)
            .context("Falha ao gerar matriz do QR Code")?;
        let width = code.width();
        let modules = code
            .into_colors()
            .into_iter()
            .map(|c| c == Color::Dark)
            .collect();
        Ok(Self { width, modules })
    }

    #[inline]
    pub fn is_dark(&self, x: usize, y: usize) -> bool {
        self.modules.get(y * self.width + x).copied().unwrap_or(false)
    }
}

/// Real-time events emitted by a Remote Auth v2 QR Code login session.
#[derive(Debug, Clone)]
pub enum RemoteAuthEvent {
    Connecting,
    QrReady {
        qr_url: String,
        matrix: QrMatrix,
    },
    Scanned {
        user_id: String,
        username: String,
        avatar_url: Option<String>,
    },
    Authenticated {
        token: String,
    },
    Cancelled,
    Expired,
    Error(String),
}

/// Parse Discord's decrypted `pending_ticket` payload:
/// `<user_id>:<discriminator>:<avatar_hash>:<username>`.
pub fn parse_remote_auth_user_payload(payload: &str) -> Option<(String, String, Option<String>)> {
    let mut parts = payload.splitn(4, ':');
    let user_id = parts.next()?.trim().to_string();
    let _discriminator = parts.next()?.trim();
    let avatar_hash = parts.next()?.trim();
    let username = parts.next()?.trim().to_string();
    if user_id.is_empty() || username.is_empty() {
        return None;
    }
    let avatar_url = if avatar_hash.is_empty() || avatar_hash == "0" {
        None
    } else {
        let ext = if avatar_hash.starts_with("a_") { "gif" } else { "png" };
        Some(format!(
            "https://cdn.discordapp.com/avatars/{}/{}.{}?size=128",
            user_id, avatar_hash, ext
        ))
    };
    Some((user_id, username, avatar_url))
}

/// Compute Discord Remote Auth v2 `nonce_proof` from decrypted nonce bytes:
/// `base64url_nopad(SHA256(nonce))`.
pub fn compute_nonce_proof(decrypted_nonce: &[u8]) -> String {
    let digest = Sha256::digest(decrypted_nonce);
    B64_URL.encode(digest)
}

/// Spawn a live Remote Auth v2 session on the Tokio runtime.
/// Dropping or cancelling the returned `tokio::task::JoinHandle` aborts the session cleanly.
pub fn start_remote_auth_session() -> (mpsc::UnboundedReceiver<RemoteAuthEvent>, tokio::task::JoinHandle<()>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let handle = tokio::spawn(async move {
        let _ = tx.send(RemoteAuthEvent::Connecting);
        if let Err(err) = run_remote_auth_loop(&tx).await {
            log::warn!("[RemoteAuth] Sessão QR encerrada com erro: {:#}", err);
            let _ = tx.send(RemoteAuthEvent::Error(err.to_string()));
        }
    });
    (rx, handle)
}

async fn run_remote_auth_loop(tx: &mpsc::UnboundedSender<RemoteAuthEvent>) -> Result<()> {
    // Generate RSA-2048 keypair on a blocking thread so we never stall async workers.
    let (private_key, encoded_public_key) = tokio::task::spawn_blocking(|| -> Result<(RsaPrivateKey, String)> {
        let mut rng = OsRng;
        let priv_key = RsaPrivateKey::new(&mut rng, 2048)
            .context("Falha ao gerar chave RSA-2048 para login QR")?;
        let pub_key = RsaPublicKey::from(&priv_key);
        let spki_der = pub_key
            .to_public_key_der()
            .context("Falha ao serializar chave pública SPKI")?;
        Ok((priv_key, B64_STD.encode(spki_der.as_bytes())))
    })
    .await
    .context("Falha na thread de geração RSA")??;

    let mut request = REMOTE_AUTH_WS_URL
        .into_client_request()
        .context("URL inválida do Remote Auth Gateway")?;
    request
        .headers_mut()
        .insert("Origin", "https://discord.com".parse()?);
    request
        .headers_mut()
        .insert("User-Agent", DISCORD_DESKTOP_USER_AGENT.parse()?);

    let (ws_stream, _) = connect_async(request)
        .await
        .context("Não foi possível conectar ao Remote Auth do Discord")?;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<WsMessage>();
    let writer_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_write.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut heartbeat_task: Option<tokio::task::JoinHandle<()>> = None;

    while let Some(frame) = ws_read.next().await {
        let msg = match frame {
            Ok(m) => m,
            Err(e) => {
                if let Some(hb) = heartbeat_task.take() {
                    hb.abort();
                }
                writer_task.abort();
                return Err(anyhow!("Erro de conexão Remote Auth: {}", e));
            }
        };

        let text = match msg {
            WsMessage::Text(t) => t,
            WsMessage::Close(_) => {
                let _ = tx.send(RemoteAuthEvent::Expired);
                break;
            }
            _ => continue,
        };

        let payload: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("");
        match op {
            "hello" => {
                let interval_ms = payload
                    .get("heartbeat_interval")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(41_250);
                let hb_tx = out_tx.clone();
                heartbeat_task = Some(tokio::spawn(async move {
                    let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms));
                    ticker.tick().await; // skip immediate tick
                    loop {
                        ticker.tick().await;
                        let hb = json!({ "op": "heartbeat" }).to_string();
                        if hb_tx.send(WsMessage::Text(hb.into())).is_err() {
                            break;
                        }
                    }
                }));

                let init_msg = json!({
                    "op": "init",
                    "encoded_public_key": encoded_public_key,
                })
                .to_string();
                let _ = out_tx.send(WsMessage::Text(init_msg.into()));
            }
            "nonce_proof" => {
                let enc_b64 = payload
                    .get("encrypted_nonce")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("nonce_proof sem encrypted_nonce"))?;
                let enc_bytes = B64_STD
                    .decode(enc_b64)
                    .context("encrypted_nonce inválido")?;
                let decrypted = private_key
                    .decrypt(Oaep::new::<Sha256>(), &enc_bytes)
                    .context("Falha ao descriptografar nonce_proof")?;
                let proof = compute_nonce_proof(&decrypted);
                let reply = json!({
                    "op": "nonce_proof",
                    "proof": proof,
                })
                .to_string();
                let _ = out_tx.send(WsMessage::Text(reply.into()));
            }
            "pending_remote_init" => {
                if let Some(fingerprint) = payload.get("fingerprint").and_then(|v| v.as_str()) {
                    let qr_url = format!("https://discord.com/ra/{}", fingerprint);
                    let matrix = QrMatrix::from_url(&qr_url)?;
                    let _ = tx.send(RemoteAuthEvent::QrReady { qr_url, matrix });
                }
            }
            "pending_ticket" => {
                if let Some(enc_user) = payload.get("encrypted_user_payload").and_then(|v| v.as_str()) {
                    if let Ok(enc_bytes) = B64_STD.decode(enc_user) {
                        if let Ok(plain) = private_key.decrypt(Oaep::new::<Sha256>(), &enc_bytes) {
                            if let Ok(plain_str) = String::from_utf8(plain) {
                                if let Some((user_id, username, avatar_url)) =
                                    parse_remote_auth_user_payload(&plain_str)
                                {
                                    let _ = tx.send(RemoteAuthEvent::Scanned {
                                        user_id,
                                        username,
                                        avatar_url,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            "pending_login" => {
                let ticket = payload
                    .get("ticket")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("pending_login sem ticket"))?;
                let token = exchange_remote_auth_ticket(ticket, &private_key).await?;
                let _ = tx.send(RemoteAuthEvent::Authenticated { token });
                break;
            }
            "cancel" => {
                let _ = tx.send(RemoteAuthEvent::Cancelled);
                break;
            }
            "heartbeat_ack" => {}
            _ => {}
        }
    }

    if let Some(hb) = heartbeat_task.take() {
        hb.abort();
    }
    writer_task.abort();
    Ok(())
}

async fn exchange_remote_auth_ticket(ticket: &str, private_key: &RsaPrivateKey) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(DISCORD_DESKTOP_USER_AGENT)
        .build()
        .context("Falha ao criar cliente HTTP para Remote Auth")?;

    let resp = client
        .post(REMOTE_AUTH_LOGIN_URL)
        .header("Content-Type", "application/json")
        .header("X-Super-Properties", build_super_properties_b64())
        .header("X-Discord-Locale", "pt-BR")
        .header("X-Discord-Timezone", "America/Sao_Paulo")
        .header("Origin", "https://discord.com")
        .header("Referer", "https://discord.com/login")
        .json(&json!({ "ticket": ticket }))
        .send()
        .await
        .context("Falha ao trocar ticket de Remote Auth")?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(anyhow!(
            "Discord recusou a troca do ticket QR (HTTP {}). Tente gerar um novo QR Code.",
            status.as_u16()
        ));
    }

    let body: Value = resp
        .json()
        .await
        .context("Resposta JSON inválida no remote-auth/login")?;
    let encrypted_token = body
        .get("encrypted_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Resposta do Discord não contém encrypted_token"))?;
    let enc_bytes = B64_STD
        .decode(encrypted_token)
        .context("encrypted_token inválido")?;
    let plain = private_key
        .decrypt(Oaep::new::<Sha256>(), &enc_bytes)
        .context("Falha ao descriptografar token de sessão")?;
    let token = String::from_utf8(plain).context("Token descriptografado não é UTF-8 válido")?;
    if token.trim().is_empty() {
        return Err(anyhow!("Token descriptografado vazio"));
    }
    Ok(token)
}
