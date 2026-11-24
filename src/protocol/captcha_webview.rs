//! Interactive native WebKit2GTK WebView popup for user-solved hCaptcha challenges.
//!
//! When Discord returns `captcha_service: "hcaptcha"` during QR Code Remote Auth or
//! Email/Password login, this module opens a visible dark-themed native GTK WebView
//! window displaying the official hCaptcha widget so the **user themselves** can
//! solve the verification interactively. Never bypasses or automates CAPTCHAs.

use std::process::Stdio;

use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Structured hCaptcha challenge returned by Discord's `/auth/login` or `/remote-auth/login`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptchaChallenge {
    pub sitekey: String,
    pub service: String,
    pub rqdata: Option<String>,
    pub rqtoken: Option<String>,
    pub session_id: Option<String>,
}

/// User-solved CAPTCHA response headers to attach to the retried Discord request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptchaSolution {
    pub captcha_key: String,
    pub rqtoken: Option<String>,
    pub session_id: Option<String>,
}

impl CaptchaChallenge {
    /// Parse a Discord JSON response body into a `CaptchaChallenge` if `captcha_key` is present.
    pub fn from_discord_json(body: &Value) -> Option<Self> {
        body.get("captcha_key")?;
        let sitekey = body.get("captcha_sitekey")?.as_str()?.trim().to_string();
        if sitekey.is_empty() {
            return None;
        }
        let service = body
            .get("captcha_service")
            .and_then(|v| v.as_str())
            .unwrap_or("hcaptcha")
            .to_string();
        let rqdata = body
            .get("captcha_rqdata")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let rqtoken = body
            .get("captcha_rqtoken")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let session_id = body
            .get("captcha_session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Some(Self {
            sitekey,
            service,
            rqdata,
            rqtoken,
            session_id,
        })
    }
}

/// Build the dark Discord-themed HTML document that renders the official hCaptcha widget.
pub fn build_captcha_html(challenge: &CaptchaChallenge) -> String {
    let sitekey_json = serde_json::to_string(&challenge.sitekey).unwrap_or_else(|_| "\"\"".into());
    let rqdata_json = match &challenge.rqdata {
        Some(r) => serde_json::to_string(r).unwrap_or_else(|_| "null".into()),
        None => "null".into(),
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="pt-BR">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>Verificação de Segurança — Discord RS</title>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  html, body {{
    width: 100%;
    height: 100%;
    background: linear-gradient(135deg, #131642 0%, #2b39c4 100%);
    font-family: 'gg sans', 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    color: #f2f3f5;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }}
  .card {{
    width: 390px;
    background: #313338;
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 10px;
    padding: 28px 24px;
    box-shadow: 0 16px 40px rgba(0,0,0,0.45);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    text-align: center;
  }}
  .badge {{
    width: 44px;
    height: 44px;
    border-radius: 12px;
    background: #5865f2;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 22px;
    font-weight: 800;
    color: #fff;
  }}
  h1 {{ font-size: 20px; font-weight: 700; color: #f2f3f5; }}
  p {{ font-size: 13px; color: #b5bac1; line-height: 1.45; }}
  #captcha-host {{
    min-height: 78px;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-top: 4px;
  }}
  #status {{ font-size: 12px; color: #949ba4; min-height: 18px; }}
</style>
</head>
<body>
  <div class="card">
    <div class="badge">D</div>
    <h1>Verificação de Segurança</h1>
    <p>O Discord solicitou uma confirmação rápida para proteger sua conta. Resolva o desafio abaixo para entrar automaticamente.</p>
    <div id="captcha-host"><div id="captcha-box"></div></div>
    <div id="status">Carregando verificação...</div>
  </div>
  <script>
    const SITEKEY = {sitekey_json};
    const RQDATA = {rqdata_json};

    function setStatus(msg) {{
      const el = document.getElementById('status');
      if (el) el.textContent = msg;
    }}

    function onHcaptchaLoad() {{
      try {{
        setStatus('');
        const widgetId = hcaptcha.render('captcha-box', {{
          sitekey: SITEKEY,
          theme: 'dark',
          callback: function(token) {{
            setStatus('Verificado! Conectando ao Discord RS...');
            if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.discordCaptcha) {{
              window.webkit.messageHandlers.discordCaptcha.postMessage(token);
            }}
          }},
          'error-callback': function() {{
            setStatus('Erro ao carregar verificação. Tente novamente.');
          }},
          'expired-callback': function() {{
            setStatus('Verificação expirada. Resolva novamente.');
          }}
        }});
        if (RQDATA && typeof hcaptcha.setData === 'function') {{
          hcaptcha.setData(widgetId, {{ rqdata: RQDATA }});
        }}
      }} catch (e) {{
        setStatus('Falha ao iniciar hCaptcha: ' + e);
      }}
    }}
  </script>
  <script src="https://js.hcaptcha.com/1/api.js?render=explicit&onload=onHcaptchaLoad&hl=pt" async defer></script>
</body>
</html>"#
    )
}

const WEBKIT_HELPER_PY: &str = r#"
import sys, base64
import gi
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, Gdk

html = base64.b64decode(sys.stdin.read().strip()).decode('utf-8')

win = Gtk.Window(title='Verificação de Segurança — Discord RS')
win.set_default_size(460, 600)
win.set_position(Gtk.WindowPosition.CENTER)
win.set_keep_above(True)

ucm = WebKit2.UserContentManager()
solved = {'done': False}

def on_msg(_ucm, js_result):
    try:
        token = js_result.get_js_value().to_string()
    except Exception:
        token = ''
    if token:
        solved['done'] = True
        print('CAPTCHA_TOKEN:' + token, flush=True)
        Gtk.main_quit()

ucm.register_script_message_handler('discordCaptcha')
ucm.connect('script-message-received::discordCaptcha', on_msg)

settings = WebKit2.Settings()
settings.set_enable_javascript(True)
settings.set_user_agent('Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.170 Chrome/130.0.6723.191 Electron/33.2.0 Safari/537.36')

webview = WebKit2.WebView.new_with_user_content_manager(ucm)
webview.set_settings(settings)
rgba = Gdk.RGBA(0.075, 0.086, 0.259, 1.0)
webview.set_background_color(rgba)
webview.load_html(html, 'https://discord.com/login')

win.add(webview)
win.connect('destroy', lambda *_: Gtk.main_quit())
win.show_all()
Gtk.main()
if not solved['done']:
    print('CAPTCHA_CANCELLED', flush=True)
"#;

/// Open a visible native WebKit2GTK window so the user can interactively solve the hCaptcha.
pub async fn prompt_user_captcha(challenge: &CaptchaChallenge) -> Result<CaptchaSolution> {
    use base64::Engine as _;
    use tokio::io::AsyncWriteExt;

    if challenge.service != "hcaptcha" {
        return Err(anyhow!(
            "Serviço de verificação '{}' não suportado na janela nativa.",
            challenge.service
        ));
    }

    let html = build_captcha_html(challenge);
    let html_b64 = base64::engine::general_purpose::STANDARD.encode(html.as_bytes());

    let python_bin = if std::path::Path::new("/usr/bin/python3").exists() {
        "/usr/bin/python3"
    } else {
        "python3"
    };

    let mut child = Command::new(python_bin)
        .arg("-c")
        .arg(WEBKIT_HELPER_PY)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Não foi possível abrir a janela nativa de verificação (WebKit2GTK)")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(html_b64.as_bytes()).await?;
        let _ = stdin.shutdown().await;
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Falha ao capturar saída da janela de verificação"))?;
    let stderr = child.stderr.take();
    let mut reader = BufReader::new(stdout).lines();

    let mut solved_token: Option<String> = None;
    while let Some(line) = reader.next_line().await? {
        if let Some(tok) = line.strip_prefix("CAPTCHA_TOKEN:") {
            let trimmed = tok.trim();
            if !trimmed.is_empty() {
                solved_token = Some(trimmed.to_string());
                break;
            }
        } else if line.trim() == "CAPTCHA_CANCELLED" {
            break;
        }
    }

    let status = child.wait().await;
    if solved_token.is_none() {
        if let Some(mut err_pipe) = stderr {
            use tokio::io::AsyncReadExt;
            let mut err_buf = String::new();
            let _ = err_pipe.read_to_string(&mut err_buf).await;
            if !err_buf.trim().is_empty() {
                log::warn!(
                    "[CaptchaWebView] helper ({}) terminou ({:?}): {}",
                    python_bin,
                    status,
                    err_buf.trim()
                );
            }
        }
    }

    match solved_token {
        Some(captcha_key) => Ok(CaptchaSolution {
            captcha_key,
            rqtoken: challenge.rqtoken.clone(),
            session_id: challenge.session_id.clone(),
        }),
        None => Err(anyhow!("Verificação de segurança cancelada pelo usuário.")),
    }
}
