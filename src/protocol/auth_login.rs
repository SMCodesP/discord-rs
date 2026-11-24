//! Official Discord Email/Phone + Password + MFA TOTP authentication client.
//!
//! Uses `POST /api/v9/auth/login` and `POST /api/v9/auth/mfa/totp` with coherent
//! Linux Desktop headers (`User-Agent` + `X-Super-Properties`). Never bypasses
//! CAPTCHA: if Discord requests a CAPTCHA challenge, returns `LoginOutcome::CaptchaRequired`
//! so the user can authenticate via QR Code right beside the form.

use std::time::Duration;

use serde_json::{Value, json};

use crate::protocol::captcha_webview::{CaptchaChallenge, prompt_user_captcha};
use crate::protocol::discord_api::{DISCORD_DESKTOP_USER_AGENT, build_super_properties_b64};

const AUTH_LOGIN_URL: &str = "https://discord.com/api/v9/auth/login";
const AUTH_MFA_TOTP_URL: &str = "https://discord.com/api/v9/auth/mfa/totp";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginOutcome {
    Authenticated { token: String },
    MfaRequired { ticket: String, totp: bool, sms: bool },
    CaptchaRequired,
    Error(String),
}

fn build_auth_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .cookie_store(true)
        .user_agent(DISCORD_DESKTOP_USER_AGENT)
        .build()
        .map_err(|e| format!("Falha ao inicializar cliente HTTP: {}", e))
}

/// Parse Discord's `/auth/login` or `/auth/mfa/totp` JSON response body.
pub fn parse_login_response(status: u16, body: &Value) -> LoginOutcome {
    if let Some(token) = body.get("token").and_then(|v| v.as_str()) {
        if !token.trim().is_empty() {
            return LoginOutcome::Authenticated {
                token: token.to_string(),
            };
        }
    }

    if body.get("mfa").and_then(|v| v.as_bool()).unwrap_or(false) {
        if let Some(ticket) = body.get("ticket").and_then(|v| v.as_str()) {
            let totp = body.get("totp").and_then(|v| v.as_bool()).unwrap_or(true);
            let sms = body.get("sms").and_then(|v| v.as_bool()).unwrap_or(false);
            return LoginOutcome::MfaRequired {
                ticket: ticket.to_string(),
                totp,
                sms,
            };
        }
    }

    if body.get("captcha_key").is_some() {
        return LoginOutcome::CaptchaRequired;
    }

    // Extract real Discord error message if available
    if let Some(errors) = body.get("errors") {
        if let Some(msg) = extract_first_error_message(errors) {
            return LoginOutcome::Error(msg);
        }
    }
    if let Some(msg) = body.get("message").and_then(|v| v.as_str()) {
        if !msg.trim().is_empty() {
            return LoginOutcome::Error(msg.to_string());
        }
    }

    LoginOutcome::Error(format!(
        "Falha ao autenticar no Discord (HTTP {}). Verifique seus dados ou use o QR Code.",
        status
    ))
}

fn extract_first_error_message(val: &Value) -> Option<String> {
    match val {
        Value::Object(map) => {
            if let Some(arr) = map.get("_errors").and_then(|v| v.as_array()) {
                for item in arr {
                    if let Some(msg) = item.get("message").and_then(|v| v.as_str()) {
                        if !msg.trim().is_empty() {
                            return Some(msg.to_string());
                        }
                    }
                }
            }
            for (_, v) in map {
                if let Some(msg) = extract_first_error_message(v) {
                    return Some(msg);
                }
            }
            None
        }
        Value::Array(arr) => {
            for item in arr {
                if let Some(msg) = extract_first_error_message(item) {
                    return Some(msg);
                }
            }
            None
        }
        _ => None,
    }
}

/// Authenticate using email/phone and password via `POST /api/v9/auth/login`.
pub async fn login_with_password(login: &str, password: &str) -> LoginOutcome {
    let login = login.trim();
    if login.is_empty() || password.is_empty() {
        return LoginOutcome::Error("Informe seu e-mail/telefone e senha.".to_string());
    }

    let client = match build_auth_http_client() {
        Ok(c) => c,
        Err(e) => return LoginOutcome::Error(e),
    };

    let super_props = build_super_properties_b64();
    let fingerprint = match client
        .get("https://discord.com/api/v9/experiments")
        .header("X-Super-Properties", &super_props)
        .header("X-Discord-Locale", "pt-BR")
        .header("Origin", "https://discord.com")
        .header("Referer", "https://discord.com/login")
        .send()
        .await
    {
        Ok(exp) => exp
            .json::<Value>()
            .await
            .ok()
            .and_then(|v| v.get("fingerprint").and_then(|f| f.as_str()).map(|s| s.to_string())),
        Err(_) => None,
    };

    let mut req = client
        .post(AUTH_LOGIN_URL)
        .header("Content-Type", "application/json")
        .header("X-Super-Properties", &super_props)
        .header("X-Discord-Locale", "pt-BR")
        .header("X-Discord-Timezone", "America/Sao_Paulo")
        .header("Origin", "https://discord.com")
        .header("Referer", "https://discord.com/login")
        .json(&json!({
            "login": login,
            "password": password,
            "undelete": false,
            "login_source": Value::Null,
            "gift_code_sku_id": Value::Null
        }));
    if let Some(ref fp) = fingerprint {
        req = req.header("X-Fingerprint", fp);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return LoginOutcome::Error(format!(
                "Erro de rede ao conectar ao Discord: {}",
                e
            ));
        }
    };

    let mut status = resp.status().as_u16();
    let mut body: Value = resp.json().await.unwrap_or(Value::Null);

    if status == 400 {
        if let Some(challenge) = CaptchaChallenge::from_discord_json(&body) {
            match prompt_user_captcha(&challenge).await {
                Ok(sol) => {
                    let mut retry_req = client
                        .post(AUTH_LOGIN_URL)
                        .header("Content-Type", "application/json")
                        .header("X-Super-Properties", &super_props)
                        .header("X-Discord-Locale", "pt-BR")
                        .header("X-Discord-Timezone", "America/Sao_Paulo")
                        .header("Origin", "https://discord.com")
                        .header("Referer", "https://discord.com/login")
                        .header("X-Captcha-Key", &sol.captcha_key)
                        .json(&json!({
                            "login": login,
                            "password": password,
                            "undelete": false,
                            "login_source": Value::Null,
                            "gift_code_sku_id": Value::Null
                        }));
                    if let Some(ref fp) = fingerprint {
                        retry_req = retry_req.header("X-Fingerprint", fp);
                    }
                    if let Some(ref rqtoken) = sol.rqtoken {
                        retry_req = retry_req.header("X-Captcha-Rqtoken", rqtoken);
                    }
                    if let Some(ref sid) = sol.session_id {
                        retry_req = retry_req.header("X-Captcha-Session-Id", sid);
                    }
                    match retry_req.send().await {
                        Ok(r) => {
                            status = r.status().as_u16();
                            body = r.json().await.unwrap_or(Value::Null);
                        }
                        Err(e) => {
                            return LoginOutcome::Error(format!(
                                "Erro de rede após verificação de segurança: {}",
                                e
                            ));
                        }
                    }
                }
                Err(e) => return LoginOutcome::Error(e.to_string()),
            }
        }
    }

    parse_login_response(status, &body)
}

/// Complete 2FA TOTP verification via `POST /api/v9/auth/mfa/totp`.
pub async fn verify_mfa_totp(ticket: &str, code: &str) -> LoginOutcome {
    let code = code.trim();
    if code.is_empty() {
        return LoginOutcome::Error("Digite o código de 6 dígitos do seu autenticador.".to_string());
    }

    let client = match build_auth_http_client() {
        Ok(c) => c,
        Err(e) => return LoginOutcome::Error(e),
    };

    let resp = match client
        .post(AUTH_MFA_TOTP_URL)
        .header("Content-Type", "application/json")
        .header("X-Super-Properties", build_super_properties_b64())
        .header("X-Discord-Locale", "pt-BR")
        .header("X-Discord-Timezone", "America/Sao_Paulo")
        .header("Origin", "https://discord.com")
        .header("Referer", "https://discord.com/login")
        .json(&json!({
            "code": code,
            "ticket": ticket,
            "login_source": Value::Null,
            "gift_code_sku_id": Value::Null
        }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return LoginOutcome::Error(format!("Erro de rede na verificação 2FA: {}", e)),
    };

    let status = resp.status().as_u16();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    parse_login_response(status, &body)
}
