//! Read-only Protobuf decoder for Discord `PreloadedUserSettings`.
//!
//! Discord modern clients persist user preferences (including pinned/favorite DMs,
//! appearance theme, and client theme settings) in a binary protobuf message
//! (`PreloadedUserSettings`) sent base64-encoded via Gateway `READY` (`user_settings_proto`),
//! `USER_SETTINGS_PROTO_UPDATE` (`settings`), and REST `/api/v9/users/@me/settings-proto/1`.
//!
//! # Field Numbers and Official Schema Citations
//!
//! All field numbers below are verified against official Discord protobuf schemas:
//! - Primary Source: `discord-protos` (`PreloadedUserSettings.proto`, `PreloadedUserSettings.ts`)
//!   by `discord-userdoccers` (extracted from Discord web client builds).
//! - Documentation Source: `https://docs.discord.food/resources/user-settings-proto`
//!
//! Verified Field Numbers:
//! - `PreloadedUserSettings`:
//!   - Field 13: `appearance` (`AppearanceSettings`)
//!   - Field 15: `favorites` (`Favorites`)
//! - `AppearanceSettings`:
//!   - Field 1: `theme` (`Theme`: 0=UNSET, 1=DARK, 2=LIGHT, 3=DARKER, 4=MIDNIGHT)
//!   - Field 3: `client_theme_settings` (`ClientThemeSettings`)
//! - `ClientThemeSettings`:
//!   - Field 2: `background_gradient_preset_id` (`google.protobuf.UInt32Value`, wrapper with field 1 = uint32)
//!   - Field 4: `custom_user_theme_settings` (`CustomUserThemeSettings`)
//! - `CustomUserThemeSettings`:
//!   - Field 1: `colors` (`repeated string`, hex colors like "#1e1f22")
//!   - Field 2: `gradient_color_stops` (`repeated float`)
//!   - Field 3: `gradient_angle` (`int32`)
//!   - Field 4: `base_mix` (`int32`)
//! - `Favorites`:
//!   - Field 1: `favorite_channels` (`map<fixed64, FavoriteChannel>`)
//!   - Field 2: `muted` (`bool`)
//! - `FavoriteChannel`:
//!   - Field 1: `nickname` (`string`)
//!   - Field 2: `parent_id` (`fixed64`)
//!   - Field 3: `position` (`uint32`)
//!   - Field 4: `channel_id` (`fixed64`)
//!   - Field 5: `channel_type` (`google.protobuf.UInt32Value`, wrapper with field 1 = uint32: 1=DM, 3=GROUP_DM)
//!
//! Note: This implementation is strictly READ-ONLY. No write or PATCH endpoints are implemented.

use serde::{Deserialize, Serialize};

/// Appearance theme preference stored in Discord user settings.
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

/// Custom user theme gradient and color settings.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CustomUserThemeSettings {
    pub colors: Vec<String>,
    pub gradient_color_stops: Vec<f32>,
    pub gradient_angle: Option<i32>,
    pub base_mix: Option<i32>,
}

/// Client theme settings, including background gradient preset or custom colors.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ClientThemeSettings {
    pub background_gradient_preset_id: Option<u32>,
    pub custom_user_theme_settings: Option<CustomUserThemeSettings>,
}

/// Appearance settings container.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AppearanceSettings {
    pub theme: Option<Theme>,
    pub client_theme_settings: Option<ClientThemeSettings>,
}

/// Individual favorite channel metadata in `Favorites.favorite_channels`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FavoriteChannel {
    pub nickname: Option<String>,
    pub parent_id: Option<u64>,
    pub position: u32,
    pub channel_id: Option<u64>,
    pub channel_type: Option<u32>,
}

/// Favorites container holding favorited channels and categories.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FavoritesSettings {
    pub favorite_channels: Vec<(String, FavoriteChannel)>,
    pub muted: bool,
}

/// Decoded representation of `PreloadedUserSettings`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PreloadedUserSettings {
    pub appearance: Option<AppearanceSettings>,
    pub favorites: Option<FavoritesSettings>,
}

impl PreloadedUserSettings {
    /// Extracts the list of pinned/favorite Direct Message (DM) channel IDs,
    /// ordered strictly by `position` ascending.
    ///
    /// Excludes guild/server channels (e.g. channel_type 0, 2, 4).
    /// Includes channels with channel_type 1 (DM), 3 (GROUP_DM), or unspecified.
    pub fn pinned_dms(&self) -> Vec<String> {
        let Some(favs) = &self.favorites else {
            return Vec::new();
        };

        let mut dm_entries: Vec<(&str, u32)> = Vec::new();

        for (key, channel) in &favs.favorite_channels {
            // Channel types in Discord:
            // 0: GUILD_TEXT, 1: DM, 2: GUILD_VOICE, 3: GROUP_DM, 4: GUILD_CATEGORY, ...
            let is_dm = match channel.channel_type {
                Some(1) | Some(3) => true,
                Some(_) => false, // Guild text, voice, category, etc.
                None => true,     // Unspecified, treat as candidate (filtered downstream against known DMs)
            };

            if is_dm && !key.is_empty() {
                dm_entries.push((key.as_str(), channel.position));
            }
        }

        // Sort stably by position ascending
        dm_entries.sort_by_key(|&(_, pos)| pos);

        // Deduplicate while preserving order
        let mut result = Vec::new();
        for (id, _) in dm_entries {
            if !result.iter().any(|existing: &String| existing == id) {
                result.push(id.to_string());
            }
        }

        result
    }

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
pub fn decode_base64(input: &str) -> anyhow::Result<Vec<u8>> {
    let mut table = [255u8; 256];
    for (i, &b) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".iter().enumerate() {
        table[b as usize] = i as u8;
    }
    table[b'-' as usize] = 62;
    table[b'_' as usize] = 63;

    let mut buf = Vec::with_capacity((input.len() * 3) / 4);
    let mut val = 0u32;
    let mut val_bits = 0;

    for &byte in input.as_bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'=' {
            break;
        }
        let d = table[byte as usize];
        if d == 255 {
            anyhow::bail!("Invalid base64 character: {}", byte as char);
        }
        val = (val << 6) | (d as u32);
        val_bits += 6;
        if val_bits >= 8 {
            val_bits -= 8;
            buf.push((val >> val_bits) as u8);
        }
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

fn read_fixed64(buf: &[u8], pos: &mut usize) -> anyhow::Result<u64> {
    if *pos + 8 > buf.len() {
        anyhow::bail!("Unexpected EOF reading fixed64");
    }
    let bytes: [u8; 8] = buf[*pos..*pos + 8].try_into().unwrap();
    *pos += 8;
    Ok(u64::from_le_bytes(bytes))
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
    let len = read_varint(buf, pos)? as usize;
    if *pos + len > buf.len() {
        anyhow::bail!("Unexpected EOF reading length-delimited bytes (len: {})", len);
    }
    let slice = &buf[*pos..*pos + len];
    *pos += len;
    Ok(slice)
}

fn read_string(buf: &[u8], pos: &mut usize) -> anyhow::Result<String> {
    let bytes = read_bytes(buf, pos)?;
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

fn skip_field(buf: &[u8], pos: &mut usize, wire_type: u8) -> anyhow::Result<()> {
    match wire_type {
        0 => {
            read_varint(buf, pos)?;
        }
        1 => {
            if *pos + 8 > buf.len() {
                anyhow::bail!("Unexpected EOF skipping fixed64");
            }
            *pos += 8;
        }
        2 => {
            let len = read_varint(buf, pos)? as usize;
            if *pos + len > buf.len() {
                anyhow::bail!("Unexpected EOF skipping length-delimited");
            }
            *pos += len;
        }
        5 => {
            if *pos + 4 > buf.len() {
                anyhow::bail!("Unexpected EOF skipping fixed32");
            }
            *pos += 4;
        }
        _ => anyhow::bail!("Unsupported wire type {}", wire_type),
    }
    Ok(())
}

fn decode_uint32_value(buf: &[u8]) -> anyhow::Result<u32> {
    let mut pos = 0;
    let mut val = 0;
    while pos < buf.len() {
        let tag = read_varint(buf, &mut pos)?;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;
        if field_num == 1 && wire_type == 0 {
            val = read_varint(buf, &mut pos)? as u32;
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
        let tag = read_varint(buf, &mut pos)?;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

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
                let mut i = 0;
                while i + 4 <= slice.len() {
                    let b: [u8; 4] = slice[i..i + 4].try_into().unwrap();
                    settings.gradient_color_stops.push(f32::from_bits(u32::from_le_bytes(b)));
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
        let tag = read_varint(buf, &mut pos)?;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

        match (field_num, wire_type) {
            // Field 2: background_gradient_preset_id (google.protobuf.UInt32Value wrapper or raw varint)
            (2, 2) => {
                let wrapper = read_bytes(buf, &mut pos)?;
                settings.background_gradient_preset_id = Some(decode_uint32_value(wrapper)?);
            }
            (2, 0) => {
                settings.background_gradient_preset_id = Some(read_varint(buf, &mut pos)? as u32);
            }
            // Field 4: custom_user_theme_settings
            (4, 2) => {
                let inner = read_bytes(buf, &mut pos)?;
                settings.custom_user_theme_settings = Some(decode_custom_user_theme_settings(inner)?);
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
        let tag = read_varint(buf, &mut pos)?;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

        match (field_num, wire_type) {
            // Field 1: theme (enum Theme)
            (1, 0) => {
                appearance.theme = Some(Theme::from_u32(read_varint(buf, &mut pos)? as u32));
            }
            // Field 3: client_theme_settings
            (3, 2) => {
                let inner = read_bytes(buf, &mut pos)?;
                appearance.client_theme_settings = Some(decode_client_theme_settings(inner)?);
            }
            _ => {
                skip_field(buf, &mut pos, wire_type)?;
            }
        }
    }

    Ok(appearance)
}

fn decode_favorite_channel(buf: &[u8]) -> anyhow::Result<FavoriteChannel> {
    let mut channel = FavoriteChannel::default();
    let mut pos = 0;

    while pos < buf.len() {
        let tag = read_varint(buf, &mut pos)?;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

        match (field_num, wire_type) {
            // Field 1: nickname (string)
            (1, 2) => {
                channel.nickname = Some(read_string(buf, &mut pos)?);
            }
            // Field 2: parent_id (fixed64 or varint)
            (2, 1) => {
                channel.parent_id = Some(read_fixed64(buf, &mut pos)?);
            }
            (2, 0) => {
                channel.parent_id = Some(read_varint(buf, &mut pos)?);
            }
            // Field 3: position (uint32)
            (3, 0) => {
                channel.position = read_varint(buf, &mut pos)? as u32;
            }
            // Field 4: channel_id (fixed64, varint, or string)
            (4, 1) => {
                channel.channel_id = Some(read_fixed64(buf, &mut pos)?);
            }
            (4, 0) => {
                channel.channel_id = Some(read_varint(buf, &mut pos)?);
            }
            // Field 5: channel_type (google.protobuf.UInt32Value wrapper or raw varint)
            (5, 2) => {
                let wrapper = read_bytes(buf, &mut pos)?;
                channel.channel_type = Some(decode_uint32_value(wrapper)?);
            }
            (5, 0) => {
                channel.channel_type = Some(read_varint(buf, &mut pos)? as u32);
            }
            _ => {
                skip_field(buf, &mut pos, wire_type)?;
            }
        }
    }

    Ok(channel)
}

fn decode_favorite_map_entry(buf: &[u8]) -> anyhow::Result<(String, FavoriteChannel)> {
    let mut key = String::new();
    let mut channel = FavoriteChannel::default();
    let mut pos = 0;

    while pos < buf.len() {
        let tag = read_varint(buf, &mut pos)?;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

        match (field_num, wire_type) {
            // Field 1: key (fixed64, varint, or string)
            (1, 1) => {
                key = read_fixed64(buf, &mut pos)?.to_string();
            }
            (1, 0) => {
                key = read_varint(buf, &mut pos)?.to_string();
            }
            (1, 2) => {
                key = read_string(buf, &mut pos)?;
            }
            // Field 2: value (FavoriteChannel)
            (2, 2) => {
                let inner = read_bytes(buf, &mut pos)?;
                channel = decode_favorite_channel(inner)?;
            }
            _ => {
                skip_field(buf, &mut pos, wire_type)?;
            }
        }
    }

    if key.is_empty() {
        if let Some(id) = channel.channel_id {
            key = id.to_string();
        }
    }

    Ok((key, channel))
}

fn decode_favorites_settings(buf: &[u8]) -> anyhow::Result<FavoritesSettings> {
    let mut favorites = FavoritesSettings::default();
    let mut pos = 0;

    while pos < buf.len() {
        let tag = read_varint(buf, &mut pos)?;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

        match (field_num, wire_type) {
            // Field 1: favorite_channels (repeated MapEntry)
            (1, 2) => {
                let entry_bytes = read_bytes(buf, &mut pos)?;
                if let Ok(entry) = decode_favorite_map_entry(entry_bytes) {
                    favorites.favorite_channels.push(entry);
                }
            }
            // Field 2: muted (bool)
            (2, 0) => {
                favorites.muted = read_varint(buf, &mut pos)? != 0;
            }
            _ => {
                skip_field(buf, &mut pos, wire_type)?;
            }
        }
    }

    Ok(favorites)
}

/// Decodes binary protobuf buffer into `PreloadedUserSettings`.
pub fn decode_preloaded_user_settings_bytes(buf: &[u8]) -> anyhow::Result<PreloadedUserSettings> {
    let mut settings = PreloadedUserSettings::default();
    let mut pos = 0;

    while pos < buf.len() {
        let tag = read_varint(buf, &mut pos)?;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

        match (field_num, wire_type) {
            // Field 13: appearance (AppearanceSettings)
            (13, 2) => {
                let inner = read_bytes(buf, &mut pos)?;
                settings.appearance = Some(decode_appearance_settings(inner)?);
            }
            // Field 15: favorites (Favorites)
            (15, 2) => {
                let inner = read_bytes(buf, &mut pos)?;
                settings.favorites = Some(decode_favorites_settings(inner)?);
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

    pub fn build_favorite_channel(position: u32, channel_type: Option<u32>) -> Vec<u8> {
        let mut buf = Vec::new();
        // Field 3: position
        write_tag(&mut buf, 3, 0);
        write_varint(&mut buf, position as u64);
        // Field 5: channel_type
        if let Some(ct) = channel_type {
            let wrapper = write_uint32_value(ct);
            write_length_delimited(&mut buf, 5, &wrapper);
        }
        buf
    }

    pub fn build_favorite_entry_fixed64(key_channel_id: u64, position: u32, channel_type: Option<u32>) -> Vec<u8> {
        let mut entry = Vec::new();
        // Field 1: key (fixed64)
        write_tag(&mut entry, 1, 1);
        entry.extend_from_slice(&key_channel_id.to_le_bytes());
        // Field 2: value
        let val_bytes = build_favorite_channel(position, channel_type);
        write_length_delimited(&mut entry, 2, &val_bytes);
        entry
    }

    pub fn build_appearance(theme: u32, preset_id: Option<u32>, colors: &[&str]) -> Vec<u8> {
        let mut app_buf = Vec::new();
        // Field 1: theme
        write_tag(&mut app_buf, 1, 0);
        write_varint(&mut app_buf, theme as u64);

        let mut client_theme_buf = Vec::new();
        if let Some(pid) = preset_id {
            let pid_wrapper = write_uint32_value(pid);
            write_length_delimited(&mut client_theme_buf, 2, &pid_wrapper);
        }
        if !colors.is_empty() {
            let mut custom_buf = Vec::new();
            for c in colors {
                write_length_delimited(&mut custom_buf, 1, c.as_bytes());
            }
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
        let encoded = "  SGVsbG8-\n\rVGhlcmU_  ";
        let decoded = decode_base64(encoded).expect("url safe base64 decode");
        assert_eq!(decoded.len(), 12);
    }

    #[test]
    fn test_decode_synthetic_preloaded_user_settings() {
        use synthetic::*;

        let mut top_level = Vec::new();

        // Build Appearance (Field 13): Theme::Darker (3), preset 42, custom color "#1e1f22"
        let appearance_bytes = build_appearance(3, Some(42), &["#1e1f22"]);
        write_length_delimited(&mut top_level, 13, &appearance_bytes);

        // Build Favorites (Field 15) with 2 DMs in reversed position order
        let mut favs_buf = Vec::new();
        // DM 1: id 10001, pos 2, channel_type 1 (DM)
        let entry1 = build_favorite_entry_fixed64(10001, 2, Some(1));
        write_length_delimited(&mut favs_buf, 1, &entry1);
        // DM 2: id 10002, pos 0, channel_type 1 (DM)
        let entry2 = build_favorite_entry_fixed64(10002, 0, Some(1));
        write_length_delimited(&mut favs_buf, 1, &entry2);
        // Server channel: id 99999, pos 1, channel_type 0 (GUILD_TEXT - should be ignored)
        let entry_server = build_favorite_entry_fixed64(99999, 1, Some(0));
        write_length_delimited(&mut favs_buf, 1, &entry_server);

        write_length_delimited(&mut top_level, 15, &favs_buf);

        let b64 = encode_base64(&top_level);
        let settings = decode_preloaded_user_settings(&b64).expect("decode user settings");

        assert_eq!(settings.theme(), Some(Theme::Darker));
        let client_theme = settings.client_theme_settings().expect("client theme");
        assert_eq!(client_theme.background_gradient_preset_id, Some(42));
        assert_eq!(
            client_theme
                .custom_user_theme_settings
                .as_ref()
                .unwrap()
                .colors,
            vec!["#1e1f22".to_string()]
        );

        // Pinned DMs should be ordered by position ascending: [10002 (pos 0), 10001 (pos 2)]
        let pinned = settings.pinned_dms();
        assert_eq!(pinned, vec!["10002".to_string(), "10001".to_string()]);
    }
}
