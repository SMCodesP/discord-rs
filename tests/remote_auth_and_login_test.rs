use discord_rs::protocol::auth_login::{LoginOutcome, parse_login_response};
use discord_rs::protocol::remote_auth::{
    QrMatrix, compute_nonce_proof, parse_remote_auth_user_payload,
};
use serde_json::json;

#[test]
fn test_qr_matrix_generates_valid_square_for_remote_auth_url() {
    let matrix = QrMatrix::from_url("https://discord.com/ra/test-fingerprint-1234567890").unwrap();
    assert!(matrix.width >= 21);
    assert_eq!(matrix.modules.len(), matrix.width * matrix.width);
    // Top-left finder pattern corner is always dark in standard QR codes
    assert!(matrix.is_dark(0, 0));
}

#[test]
fn test_compute_nonce_proof_matches_sha256_base64url_nopad() {
    let proof = compute_nonce_proof(b"discord-remote-auth-nonce");
    assert!(!proof.contains('='));
    assert!(!proof.contains('+'));
    assert!(!proof.contains('/'));
    assert_eq!(proof.len(), 43); // 32-byte SHA-256 -> 43 chars base64url nopad
}

#[test]
fn test_parse_remote_auth_user_payload_static_and_animated_avatars() {
    let (uid, uname, avatar) =
        parse_remote_auth_user_payload("123456789012345678:0:abcdef123456:tester").unwrap();
    assert_eq!(uid, "123456789012345678");
    assert_eq!(uname, "tester");
    assert_eq!(
        avatar.as_deref(),
        Some("https://cdn.discordapp.com/avatars/123456789012345678/abcdef123456.png?size=128")
    );

    let (_, _, anim_avatar) =
        parse_remote_auth_user_payload("123456789012345678:0:a_deadbeef:tester").unwrap();
    assert_eq!(
        anim_avatar.as_deref(),
        Some("https://cdn.discordapp.com/avatars/123456789012345678/a_deadbeef.gif?size=128")
    );

    let (_, _, no_avatar) =
        parse_remote_auth_user_payload("123456789012345678:0:0:tester").unwrap();
    assert!(no_avatar.is_none());
}

#[test]
fn test_parse_login_response_variants() {
    assert_eq!(
        parse_login_response(200, &json!({ "token": "synthetic.session.token" })),
        LoginOutcome::Authenticated {
            token: "synthetic.session.token".to_string()
        }
    );

    assert_eq!(
        parse_login_response(
            200,
            &json!({ "mfa": true, "ticket": "mfa-ticket-1", "totp": true, "sms": false })
        ),
        LoginOutcome::MfaRequired {
            ticket: "mfa-ticket-1".to_string(),
            totp: true,
            sms: false
        }
    );

    assert_eq!(
        parse_login_response(
            400,
            &json!({ "captcha_key": ["captcha-required"], "captcha_service": "hcaptcha" })
        ),
        LoginOutcome::CaptchaRequired
    );

    assert_eq!(
        parse_login_response(
            400,
            &json!({
                "errors": {
                    "login": {
                        "_errors": [{ "code": "INVALID_LOGIN", "message": "Login or password is invalid." }]
                    }
                }
            })
        ),
        LoginOutcome::Error("Login or password is invalid.".to_string())
    );
}
