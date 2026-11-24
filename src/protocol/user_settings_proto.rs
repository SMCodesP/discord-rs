//! Read-only Protobuf decoder for Discord `PreloadedUserSettings` (Appearance & Theme).
//!
//! Discord modern clients persist user appearance preferences (theme, client theme settings,
//! gradient presets, and custom colors) in a binary protobuf message (`PreloadedUserSettings`)
//! sent base64-encoded via Gateway `READY` (`user_settings_proto`), Gateway
//! `USER_SETTINGS_PROTO_UPDATE` (`settings`), and REST `GET /api/v9/users/@me/settings-proto/1`.
//!
//! # Official Schema Source and Line Citations
//!
//! Verified against official protobuf definition at:
//! `/home/smcodes/.cache/discord-rs-parity/orch/PreloadedUserSettings.proto`
//! (package `discord_protos.discord_users.v1`):
//!
//! | Mensagem / Campo | Nº Campo | Tipo de Fio (Wire Type) | Linha no `.proto` |
//! |---|---|---|---|
//! | `PreloadedUserSettings.appearance` | 13 | 2 (Length-delimited) | Linha 607 |
//! | `AppearanceSettings.theme` | 1 | 0 (Varint, enum `Theme`) | Linha 438 |
//! | `AppearanceSettings.client_theme_settings` | 3 | 2 (Length-delimited) | Linha 440 |
//! | `ClientThemeSettings.background_gradient_preset_id` | 2 | 2 (Length-delimited `UInt32Value`) | Linha 406 |
//! | `ClientThemeSettings.custom_user_theme_settings` | 4 | 2 (Length-delimited `CustomUserThemeSettings`) | Linha 407 |
//! | `CustomUserThemeSettings.colors` | 1 | 2 (Repeated length-delimited `string`) | Linha 399 |
//! | `CustomUserThemeSettings.gradient_color_stops` | 2 | 5 (Fixed32) ou 2 (Packed Fixed32 `float`) | Linha 400 |
//! | `CustomUserThemeSettings.gradient_angle` | 3 | 0 (Varint `int32`) | Linha 401 |
//! | `CustomUserThemeSettings.base_mix` | 4 | 0 (Varint `int32`) | Linha 402 |
//! | Enum `Theme` (`THEME_UNSET = 0`) | - | 0 (Varint) | Linha 391 |
//! | Enum `Theme` (`THEME_DARK = 1`) | - | 0 (Varint) | Linha 392 |
//! | Enum `Theme` (`THEME_LIGHT = 2`) | - | 0 (Varint) | Linha 393 |
//! | Enum `Theme` (`THEME_DARKER = 3`) | - | 0 (Varint) | Linha 394 |
//! | Enum `Theme` (`THEME_MIDNIGHT = 4`) | - | 0 (Varint) | Linha 395 |
//!
//! # Pinned DMs
//!
//! Conforme verificação no `.proto` (linhas 1-623), **não existe campo de DMs fixadas** em
//! `PreloadedUserSettings`. O campo `Favorites` (linha 609, mensagem nas linhas 481-486) refere-se
//! ao recurso de servidor "Favoritos" (`guild_visible`, `auto_add_joined_threads`), não a DMs.
//! Portanto, DMs fixadas continuam sendo gerenciadas exclusivamente de forma local no `discord-rs`.
//!
//! # Fronteira de Somente Leitura
//!
//! O cliente opera estritamente em modo de leitura. Nenhuma chamada de escrita (`PATCH`) é enviada.

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// Appearance theme preference stored in Discord user settings (`PreloadedUserSettings.proto:390-396`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Unset = 0,
    Dark = 1,
    Light = 2,
    Darker = 3,
    Midnight = 4,
}

impl Theme {
    pub fn from_u32(val: u32) -> Self {
        match val {
            1 => Self::Dark,
            2 => Self::Light,
            3 => Self::Darker,
            4 => Self::Midnight,
            _ => Self::Unset,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Darker => "darker",
            Self::Midnight => "midnight",
        }
    }

    pub fn is_dark(&self) -> bool {
        matches!(self, Self::Dark | Self::Darker | Self::Midnight)
    }
}

/// Custom user theme gradient and color settings (`PreloadedUserSettings.proto:398-403`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CustomUserThemeSettings {
    pub colors: Vec<String>,
    pub gradient_color_stops: Vec<f32>,
    pub gradient_angle: Option<i32>,
    pub base_mix: Option<i32>,
}

/// Client theme settings, including background gradient preset or custom colors (`PreloadedUserSettings.proto:405-408`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ClientThemeSettings {
    pub background_gradient_preset_id: Option<u32>,
    pub custom_user_theme_settings: Option<CustomUserThemeSettings>,
}

/// Appearance settings container (`PreloadedUserSettings.proto:437-452`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AppearanceSettings {
    pub theme: Option<Theme>,
    pub client_theme_settings: Option<ClientThemeSettings>,
}

/// Decoded representation of `PreloadedUserSettings` (somente leitura).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PreloadedUserSettings {
    pub appearance: Option<AppearanceSettings>,
}

impl PreloadedUserSettings {
    /// Returns the account appearance theme if present.
    pub fn theme(&self) -> Option<Theme> {
        self.appearance.as_ref().and_then(|a| a.theme)
    }

    /// Returns the client theme settings if present.
    pub fn client_theme_settings(&self) -> Option<&ClientThemeSettings> {
        self.appearance
            .as_ref()
            .and_then(|a| a.client_theme_settings.as_ref())
    }
}

// ============================================================================
// Base64 Decoder & Encoder (Pure Rust, Zero external dependencies)
// ============================================================================

/// Decodes standard or URL-safe Base64 string (with or without padding).
///
/// Whitespace is accepted, but padding must appear only at the end and unused
/// bits must be zero. Malformed input is rejected instead of silently producing
/// a truncated protobuf payload.
pub fn decode_base64(input: &str) -> anyhow::Result<Vec<u8>> {
    let mut table = [255u8; 256];
    for (i, &b) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        .iter()
        .enumerate()
    {
        table[b as usize] = i as u8;
    }
    table[b'-' as usize] = 62;
    table[b'_' as usize] = 63;

    let bytes: Vec<u8> = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    let first_padding = bytes.iter().position(|byte| *byte == b'=');
    let data_end = first_padding.unwrap_or(bytes.len());
    let padding = bytes.len() - data_end;
    if padding > 2 || bytes[data_end..].iter().any(|byte| *byte != b'=') {
        anyhow::bail!("Invalid base64 padding");
    }
    if data_end % 4 == 1 {
        anyhow::bail!("Invalid base64 length");
    }
    if padding > 0 && bytes.len() % 4 != 0 {
        anyhow::bail!("Invalid padded base64 length");
    }

    let mut buf = Vec::with_capacity((data_end * 3) / 4);
    let mut value = 0u32;
    let mut value_bits = 0u32;

    for &byte in &bytes[..data_end] {
        let digit = table[byte as usize];
        if digit == 255 {
            anyhow::bail!("Invalid base64 character");
        }
        value = (value << 6) | u32::from(digit);
        value_bits += 6;
        if value_bits >= 8 {
            value_bits -= 8;
            buf.push((value >> value_bits) as u8);
            value &= (1u32 << value_bits) - 1;
        }
    }

    if value_bits > 0 && value != 0 {
        anyhow::bail!("Non-zero unused bits in base64 input");
    }

    Ok(buf)
}

/// Encodes binary bytes to standard Base64 string with padding.
pub fn encode_base64(input: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(CHARSET[((n >> 18) & 0x3F) as usize] as char);
        out.push(CHARSET[((n >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARSET[((n >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARSET[(n & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// ============================================================================
// Protobuf Wire Decoder
// ============================================================================

fn read_varint(buf: &[u8], pos: &mut usize) -> anyhow::Result<u64> {
    let mut result = 0u64;
    let mut shift = 0;
    while *pos < buf.len() {
        let byte = buf[*pos];
        *pos += 1;
        if shift == 63 && byte > 1 {
            anyhow::bail!("Protobuf varint overflow");
        }
        result |= ((byte & 0x7F) as u64) << shift;
        if (byte & 0x80) == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 64 {
            anyhow::bail!("Protobuf varint overflow");
        }
    }
    anyhow::bail!("Unexpected EOF reading varint");
}

fn read_tag(buf: &[u8], pos: &mut usize) -> anyhow::Result<(u32, u8)> {
    let tag = read_varint(buf, pos)?;
    let field_num = tag >> 3;
    let wire_type = (tag & 0x07) as u8;
    if field_num == 0 || field_num > u64::from(u32::MAX) {
        anyhow::bail!("Invalid protobuf field number");
    }
    Ok((field_num as u32, wire_type))
}

fn read_fixed32(buf: &[u8], pos: &mut usize) -> anyhow::Result<u32> {
    if *pos + 4 > buf.len() {
        anyhow::bail!("Unexpected EOF reading fixed32");
    }
    let bytes: [u8; 4] = buf[*pos..*pos + 4].try_into().unwrap();
    *pos += 4;
    Ok(u32::from_le_bytes(bytes))
}

fn read_bytes<'a>(buf: &'a [u8], pos: &mut usize) -> anyhow::Result<&'a [u8]> {
    let len = usize::try_from(read_varint(buf, pos)?)?;
    let end = pos
        .checked_add(len)
        .filter(|end| *end <= buf.len())
        .ok_or_else(|| anyhow::anyhow!("Unexpected EOF reading length-delimited bytes"))?;
    let slice = &buf[*pos..end];
    *pos = end;
    Ok(slice)
}

fn read_string(buf: &[u8], pos: &mut usize) -> anyhow::Result<String> {
    let bytes = read_bytes(buf, pos)?;
    String::from_utf8(bytes.to_vec()).context("Invalid UTF-8 in protobuf string")
}

fn skip_field(buf: &[u8], pos: &mut usize, wire_type: u8) -> anyhow::Result<()> {
    match wire_type {
        0 => {
            read_varint(buf, pos)?;
        }
        1 => {
            let end = pos
                .checked_add(8)
                .filter(|end| *end <= buf.len())
                .ok_or_else(|| anyhow::anyhow!("Unexpected EOF skipping fixed64"))?;
            *pos = end;
        }
        2 => {
            // Reutiliza a validação de comprimento/overflow de read_bytes.
            let _ = read_bytes(buf, pos)?;
        }
        5 => {
            let end = pos
                .checked_add(4)
                .filter(|end| *end <= buf.len())
                .ok_or_else(|| anyhow::anyhow!("Unexpected EOF skipping fixed32"))?;
            *pos = end;
        }
        _ => anyhow::bail!("Unsupported wire type {}", wire_type),
    }
    Ok(())
}

fn decode_uint32_value(buf: &[u8]) -> anyhow::Result<u32> {
    let mut pos = 0;
    let mut val = 0;
    while pos < buf.len() {
        let (field_num, wire_type) = read_tag(buf, &mut pos)?;
        if field_num == 1 {
            if wire_type != 0 {
                anyhow::bail!("UInt32Value field 1 has invalid wire type");
            }
            let value = read_varint(buf, &mut pos)?;
            if value > u64::from(u32::MAX) {
                anyhow::bail!("UInt32Value exceeds u32 range");
            }
            val = value as u32;
        } else {
            skip_field(buf, &mut pos, wire_type)?;
        }
    }
    Ok(val)
}

fn decode_custom_user_theme_settings(buf: &[u8]) -> anyhow::Result<CustomUserThemeSettings> {
    let mut settings = CustomUserThemeSettings::default();
    let mut pos = 0;

    while pos < buf.len() {
        let (field_num, wire_type) = read_tag(buf, &mut pos)?;

        match (field_num, wire_type) {
            // Field 1: colors (repeated string)
            (1, 2) => {
                settings.colors.push(read_string(buf, &mut pos)?);
            }
            // Field 2: gradient_color_stops (repeated float: wire type 5 or packed wire type 2)
            (2, 5) => {
                let bits = read_fixed32(buf, &mut pos)?;
                settings.gradient_color_stops.push(f32::from_bits(bits));
            }
            (2, 2) => {
                let slice = read_bytes(buf, &mut pos)?;
                if slice.len() % 4 != 0 {
                    anyhow::bail!("Packed fixed32 length is not a multiple of four");
                }
                let mut i = 0;
                while i + 4 <= slice.len() {
                    let b: [u8; 4] = slice[i..i + 4].try_into().unwrap();
                    settings
                        .gradient_color_stops
                        .push(f32::from_bits(u32::from_le_bytes(b)));
                    i += 4;
                }
            }
            // Field 3: gradient_angle (int32)
            (3, 0) => {
                settings.gradient_angle = Some(read_varint(buf, &mut pos)? as i32);
            }
            // Field 4: base_mix (int32)
            (4, 0) => {
                settings.base_mix = Some(read_varint(buf, &mut pos)? as i32);
            }
            (1 | 2 | 3 | 4, _) => {
                anyhow::bail!("Campo conhecido de CustomUserThemeSettings com wire type inválido");
            }
            _ => {
                skip_field(buf, &mut pos, wire_type)?;
            }
        }
    }

    Ok(settings)
}

fn decode_client_theme_settings(buf: &[u8]) -> anyhow::Result<ClientThemeSettings> {
    let mut settings = ClientThemeSettings::default();
    let mut pos = 0;

    while pos < buf.len() {
        let (field_num, wire_type) = read_tag(buf, &mut pos)?;

        match (field_num, wire_type) {
            // Field 2: background_gradient_preset_id (google.protobuf.UInt32Value wrapper or raw varint)
            (2, 2) => {
                let wrapper = read_bytes(buf, &mut pos)?;
                settings.background_gradient_preset_id = Some(decode_uint32_value(wrapper)?);
            }
            (2, 0) => {
                let value = read_varint(buf, &mut pos)?;
                if value > u64::from(u32::MAX) {
                    anyhow::bail!("background_gradient_preset_id exceeds u32 range");
                }
                settings.background_gradient_preset_id = Some(value as u32);
            }
            // Field 4: custom_user_theme_settings
            (4, 2) => {
                let inner = read_bytes(buf, &mut pos)?;
                settings.custom_user_theme_settings =
                    Some(decode_custom_user_theme_settings(inner)?);
            }
            (2 | 4, _) => {
                anyhow::bail!("Campo conhecido de ClientThemeSettings com wire type inválido");
            }
            _ => {
                skip_field(buf, &mut pos, wire_type)?;
            }
        }
    }

    Ok(settings)
}

fn decode_appearance_settings(buf: &[u8]) -> anyhow::Result<AppearanceSettings> {
    let mut appearance = AppearanceSettings::default();
    let mut pos = 0;

    while pos < buf.len() {
        let (field_num, wire_type) = read_tag(buf, &mut pos)?;

        match (field_num, wire_type) {
            // Field 1: theme (enum Theme: THEME_UNSET=0, THEME_DARK=1, THEME_LIGHT=2, THEME_DARKER=3, THEME_MIDNIGHT=4)
            (1, 0) => {
                appearance.theme = Some(Theme::from_u32(read_varint(buf, &mut pos)? as u32));
            }
            // Field 3: client_theme_settings
            (3, 2) => {
                let inner = read_bytes(buf, &mut pos)?;
                appearance.client_theme_settings = Some(decode_client_theme_settings(inner)?);
            }
            (1 | 3, _) => {
                anyhow::bail!("Campo conhecido de AppearanceSettings com wire type inválido");
            }
            _ => {
                skip_field(buf, &mut pos, wire_type)?;
            }
        }
    }

    Ok(appearance)
}

/// Decodes binary protobuf buffer into `PreloadedUserSettings`.
pub fn decode_preloaded_user_settings_bytes(buf: &[u8]) -> anyhow::Result<PreloadedUserSettings> {
    let mut settings = PreloadedUserSettings::default();
    let mut pos = 0;

    while pos < buf.len() {
        let (field_num, wire_type) = read_tag(buf, &mut pos)?;

        match (field_num, wire_type) {
            // Field 13: appearance (AppearanceSettings, PreloadedUserSettings.proto:607)
            (13, 2) => {
                let inner = read_bytes(buf, &mut pos)?;
                settings.appearance = Some(decode_appearance_settings(inner)?);
            }
            (13, _) => {
                anyhow::bail!("Campo appearance com wire type inválido");
            }
            _ => {
                skip_field(buf, &mut pos, wire_type)?;
            }
        }
    }

    Ok(settings)
}

/// Decodes Base64-encoded `PreloadedUserSettings` string from Gateway or REST.
pub fn decode_preloaded_user_settings(base64_str: &str) -> anyhow::Result<PreloadedUserSettings> {
    let bytes = decode_base64(base64_str)?;
    decode_preloaded_user_settings_bytes(&bytes)
}

// ============================================================================
// Synthetic Protobuf Test Helpers
// ============================================================================

/// Helper for constructing synthetic protobuf messages for tests.
pub mod synthetic {

    pub fn write_varint(buf: &mut Vec<u8>, mut val: u64) {
        loop {
            let byte = (val & 0x7F) as u8;
            val >>= 7;
            if val == 0 {
                buf.push(byte);
                break;
            } else {
                buf.push(byte | 0x80);
            }
        }
    }

    pub fn write_tag(buf: &mut Vec<u8>, field_num: u32, wire_type: u8) {
        write_varint(buf, ((field_num as u64) << 3) | (wire_type as u64));
    }

    pub fn write_length_delimited(buf: &mut Vec<u8>, field_num: u32, data: &[u8]) {
        write_tag(buf, field_num, 2);
        write_varint(buf, data.len() as u64);
        buf.extend_from_slice(data);
    }

    pub fn write_uint32_value(val: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        write_tag(&mut buf, 1, 0);
        write_varint(&mut buf, val as u64);
        buf
    }

    pub fn build_appearance(
        theme: u32,
        preset_id: Option<u32>,
        colors: &[&str],
        stops: &[f32],
        angle: Option<i32>,
        base_mix: Option<i32>,
    ) -> Vec<u8> {
        let mut app_buf = Vec::new();
        // Field 1: theme
        write_tag(&mut app_buf, 1, 0);
        write_varint(&mut app_buf, theme as u64);

        let mut client_theme_buf = Vec::new();
        if let Some(pid) = preset_id {
            let pid_wrapper = write_uint32_value(pid);
            write_length_delimited(&mut client_theme_buf, 2, &pid_wrapper);
        }

        let mut custom_buf = Vec::new();
        for c in colors {
            write_length_delimited(&mut custom_buf, 1, c.as_bytes());
        }
        for s in stops {
            write_tag(&mut custom_buf, 2, 5);
            custom_buf.extend_from_slice(&s.to_bits().to_le_bytes());
        }
        if let Some(ang) = angle {
            write_tag(&mut custom_buf, 3, 0);
            write_varint(&mut custom_buf, ang as u64);
        }
        if let Some(bm) = base_mix {
            write_tag(&mut custom_buf, 4, 0);
            write_varint(&mut custom_buf, bm as u64);
        }

        if !custom_buf.is_empty() {
            write_length_delimited(&mut client_theme_buf, 4, &custom_buf);
        }

        if !client_theme_buf.is_empty() {
            write_length_delimited(&mut app_buf, 3, &client_theme_buf);
        }

        app_buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_roundtrip() {
        let data = b"Hello Discord Protobuf 2026!";
        let encoded = encode_base64(data);
        let decoded = decode_base64(&encoded).expect("base64 decode failed");
        assert_eq!(data.as_slice(), decoded.as_slice());
    }

    #[test]
    fn test_base64_url_safe_and_whitespace() {
        let encoded = "  SGVsbG8g\n\rVGhlcmU  ";
        let decoded = decode_base64(encoded).expect("url safe base64 decode");
        assert_eq!(decoded, b"Hello There");
    }

    #[test]
    fn test_base64_rejects_malformed_padding_and_unused_bits() {
        for malformed in ["A", "AA=A", "AA====", "SGVsbG8=extra", "AB"] {
            assert!(
                decode_base64(malformed).is_err(),
                "malformed base64 must be rejected: {malformed}"
            );
        }
    }

    #[test]
    fn test_decode_synthetic_preloaded_user_settings() {
        use synthetic::*;

        let mut top_level = Vec::new();

        // Build Appearance (Field 13): Theme::Darker (3), preset 42, custom color "#1e1f22", stops [0.0, 1.0], angle 90, base_mix 20
        let appearance_bytes =
            build_appearance(3, Some(42), &["#1e1f22"], &[0.0, 1.0], Some(90), Some(20));
        write_length_delimited(&mut top_level, 13, &appearance_bytes);

        let b64 = encode_base64(&top_level);
        let settings = decode_preloaded_user_settings(&b64).expect("decode user settings");

        assert_eq!(settings.theme(), Some(Theme::Darker));
        let client_theme = settings.client_theme_settings().expect("client theme");
        assert_eq!(client_theme.background_gradient_preset_id, Some(42));
        let custom = client_theme.custom_user_theme_settings.as_ref().unwrap();
        assert_eq!(custom.colors, vec!["#1e1f22".to_string()]);
        assert_eq!(custom.gradient_color_stops, vec![0.0, 1.0]);
        assert_eq!(custom.gradient_angle, Some(90));
        assert_eq!(custom.base_mix, Some(20));
    }

    #[test]
    fn test_protobuf_rejects_invalid_tags_varints_and_packed_floats() {
        assert!(decode_preloaded_user_settings_bytes(&[0]).is_err());

        let mut overflow = vec![0xFF; 9];
        overflow.push(0x02);
        assert!(decode_preloaded_user_settings_bytes(&overflow).is_err());

        let mut packed = Vec::new();
        synthetic::write_tag(&mut packed, 2, 2);
        synthetic::write_varint(&mut packed, 3);
        packed.extend_from_slice(&[0, 0, 0]);
        assert!(decode_custom_user_theme_settings(&packed).is_err());

        let mut uint32_overflow = Vec::new();
        synthetic::write_tag(&mut uint32_overflow, 1, 0);
        synthetic::write_varint(&mut uint32_overflow, u64::from(u32::MAX) + 1);
        assert!(decode_uint32_value(&uint32_overflow).is_err());
    }

    #[test]
    fn test_protobuf_rejects_invalid_utf8_strings() {
        let mut colors = Vec::new();
        synthetic::write_tag(&mut colors, 1, 2);
        synthetic::write_varint(&mut colors, 1);
        colors.push(0xFF);
        assert!(decode_custom_user_theme_settings(&colors).is_err());
    }

    #[test]
    fn test_protobuf_decoder_never_panics_on_malformed_bytes() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for length in 0..64 {
            let mut bytes = Vec::with_capacity(length);
            for _ in 0..length {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                bytes.push((state >> 32) as u8);
            }
            let result = std::panic::catch_unwind(|| decode_preloaded_user_settings_bytes(&bytes));
            assert!(result.is_ok(), "decoder não deve entrar em panic");
        }
    }

    #[test]
    fn test_protobuf_rejects_unknown_overflowing_length_and_known_wrong_wire_types() {
        let mut overflowing = Vec::new();
        synthetic::write_tag(&mut overflowing, 99, 2);
        synthetic::write_varint(&mut overflowing, u64::MAX);
        assert!(decode_preloaded_user_settings_bytes(&overflowing).is_err());

        let mut wrong_top_level = Vec::new();
        synthetic::write_tag(&mut wrong_top_level, 13, 0);
        synthetic::write_varint(&mut wrong_top_level, 1);
        assert!(decode_preloaded_user_settings_bytes(&wrong_top_level).is_err());

        let mut wrong_appearance = Vec::new();
        synthetic::write_tag(&mut wrong_appearance, 1, 2);
        synthetic::write_length_delimited(&mut wrong_appearance, 1, b"dark");
        assert!(decode_appearance_settings(&wrong_appearance).is_err());

        let mut raw_u32_overflow = Vec::new();
        synthetic::write_tag(&mut raw_u32_overflow, 2, 0);
        synthetic::write_varint(&mut raw_u32_overflow, u64::from(u32::MAX) + 1);
        assert!(decode_client_theme_settings(&raw_u32_overflow).is_err());
    }
}
