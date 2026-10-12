//! Discord RS - Internationalization (i18n) Module
//! Defaults to Brazilian Portuguese (pt-BR) with support for variable substitution.

pub mod en_us;
pub mod pt_br;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    PtBr,
    EnUs,
}

static CURRENT_LOCALE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0); // 0 = PtBr, 1 = EnUs

/// Sets the current application locale.
pub fn set_locale(locale: Locale) {
    let val = match locale {
        Locale::PtBr => 0,
        Locale::EnUs => 1,
    };
    CURRENT_LOCALE.store(val, std::sync::atomic::Ordering::Relaxed);
}

/// Gets the current application locale.
pub fn current_locale() -> Locale {
    match CURRENT_LOCALE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => Locale::EnUs,
        _ => Locale::PtBr,
    }
}

/// Translates a key to the localized string in the active locale.
pub fn t(key: &'static str) -> &'static str {
    let dict = match current_locale() {
        Locale::PtBr => pt_br::DICTIONARY,
        Locale::EnUs => en_us::DICTIONARY,
    };

    for &(k, v) in dict {
        if k == key {
            return v;
        }
    }

    // Fallback to pt-BR if in en-US
    for &(k, v) in pt_br::DICTIONARY {
        if k == key {
            return v;
        }
    }

    // If key not found, return key itself
    key
}

/// Translates a key with variable replacement, e.g. `{channel}` or `{user}`.
pub fn t_with(key: &'static str, vars: &[(&str, &str)]) -> String {
    let mut text = t(key).to_string();
    for &(placeholder, val) in vars {
        let needle = format!("{{{}}}", placeholder);
        text = text.replace(&needle, val);
    }
    text
}

/// Translates a key with variable replacement (alias for t_with).
pub fn t_fmt(key: &'static str, vars: &[(&str, &str)]) -> String {
    t_with(key, vars)
}

/// Macro for easy string translation: `t!("key")` or `t!("key", channel = "geral")`
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::t($key)
    };
    ($key:expr, $($k:ident = $v:expr),+ $(,)?) => {
        $crate::i18n::t_fmt($key, &[
            $((stringify!($k), &$v.to_string()),)+
        ])
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_pt_br_translations() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_locale(Locale::PtBr);
        assert_eq!(t("status.online"), "Online");
        assert_eq!(t("status.idle"), "Ausente");
        assert_eq!(t("status.ausente"), "Ausente");
        assert_eq!(t("status.dnd"), "Não perturbar");
        assert_eq!(t("channels.text_channels"), "CANAIS DE TEXTO");
        assert_eq!(t("user.mute"), "Silenciar");
        assert_eq!(t("server.add"), "Adicionar um servidor");
        assert_eq!(t("server.explore"), "Descobrir servidores públicos");
    }

    #[test]
    fn test_interpolation() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_locale(Locale::PtBr);
        let msg = t_fmt("chat.placeholder", &[("channel", "geral")]);
        assert_eq!(msg, "Conversar em #geral");

        let msg_dm = t_with("chat.placeholder_dm", &[("user", "Prism")]);
        assert_eq!(msg_dm, "Conversar em @Prism");

        let members = t_fmt("members.online_count", &[("count", "42")]);
        assert_eq!(members, "ONLINE — 42");
    }

    #[test]
    fn test_auth_and_connection_translations() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_locale(Locale::PtBr);
        assert_eq!(t("auth.login"), "Entrar no Discord");
        assert_eq!(t("auth.enter_token"), "Inserir Token");
        assert_eq!(t("auth.token_type_hint"), "Token de Bot ou Usuário");
        assert_eq!(t("auth.save_token"), "Salvar Token");
        assert_eq!(t("auth.connect"), "Conectar");
        assert_eq!(t("auth.connecting"), "Conectando...");
        assert_eq!(t("auth.connected"), "Conectado");
        assert_eq!(t("auth.disconnect"), "Desconectar");
        assert_eq!(t("auth.failed"), "Falha ao autenticar");
        assert_eq!(
            t("auth.description"),
            "Insira seu token para carregar seus servidores e canais reais"
        );
    }

    #[test]
    fn test_en_us_fallback() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_locale(Locale::EnUs);
        assert_eq!(t("status.dnd"), "Do Not Disturb");
        assert_eq!(t("user.mute"), "Mute");
        assert_eq!(t("auth.login"), "Log In to Discord");
        assert_eq!(t("auth.connect"), "Connect");
        set_locale(Locale::PtBr); // Restore
    }
}
