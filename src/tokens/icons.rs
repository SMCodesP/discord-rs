//! Discord RS - Icon Tokens & Vector Library
//! Professional vector icon library with clean Discord / Lucide style paths.
//! Provides raw SVG paths, full SVG markup, IconAssetSource for in-memory GPUI rendering,
//! and reusable `render_icon` & `icon_*` helper functions returning `Div`.

use std::borrow::Cow;
use gpui::*;

// ============================================================================
// Raw SVG Path Data (24x24 standard grid, fill="currentColor")
// ============================================================================

/// Hash icon (#) - Text channel
pub const PATH_HASH: &str = "M5.887 21a.75.75 0 0 1-.74-.89l.83-4.52H3.643a.75.75 0 0 1-.74-.89l.3-1.61a.75.75 0 0 1 .74-.62h2.75l.73-4H4.887a.75.75 0 0 1-.74-.89l.3-1.61a.75.75 0 0 1 .74-.62h2.75l.83-4.53a.75.75 0 0 1 .74-.62h1.61a.75.75 0 0 1 .74.89l-.78 4.26h4l.83-4.53a.75.75 0 0 1 .74-.62h1.61a.75.75 0 0 1 .74.89l-.78 4.26h2.51a.75.75 0 0 1 .74.89l-.3 1.61a.75.75 0 0 1-.74.62h-2.75l-.73 4h2.51a.75.75 0 0 1 .74.89l-.3 1.61a.75.75 0 0 1-.74.62h-2.75l-.83 4.53a.75.75 0 0 1-.74.62h-1.61a.75.75 0 0 1-.74-.89l.78-4.26h-4l-.83 4.53a.75.75 0 0 1-.74.62H5.887zm2.75-7.94h4l.73-4h-4l-.73 4z";

/// Speaker icon - Voice channel / audio volume
pub const PATH_SPEAKER: &str = "M11.38 3.08a1.2 1.2 0 0 0-1.09.22L6 7.59H3c-.55 0-1 .45-1 1v6.82c0 .55.45 1 1 1h3l4.29 4.29a1.2 1.2 0 0 0 1.09.22c.45-.18.71-.62.71-1.1V4.18c0-.48-.26-.92-.71-1.1zm4.33 4.21a1 1 0 0 0-1.42 1.42c1.25 1.25 1.96 2.94 1.96 4.71s-.71 3.46-1.96 4.71a1 1 0 0 0 1.42 1.42c1.63-1.63 2.54-3.81 2.54-6.13s-.91-4.5-2.54-6.13zm3.54-3.54a1 1 0 0 0-1.42 1.42C20 7.33 21.25 10.25 21.25 13.41s-1.25 6.08-3.42 8.24a1 1 0 0 0 1.42 1.42C21.78 20.53 23.25 17.1 23.25 13.41s-1.47-7.12-4-9.66z";

/// Megaphone icon - Announcement channel
pub const PATH_MEGAPHONE: &str = "M21 3.5v13a1 1 0 0 1-1 1h-1v3a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1v-3h-1.5L7.5 20.5V3.5L12.5 6.5H19v-3a1 1 0 0 1 1-1c.55 0 1 .45 1 1zM6 6.5H4C2.9 6.5 2 7.4 2 8.5v7c0 1.1.9 2 2 2h2v-11z";

/// Gear icon - Settings / preferences
pub const PATH_GEAR: &str = "M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58a.49.49 0 0 0 .12-.61l-1.92-3.32a.49.49 0 0 0-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54a.48.48 0 0 0-.48-.41h-3.84c-.24 0-.44.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96a.49.49 0 0 0-.59.22L2.74 8.87a.49.49 0 0 0 .12.61l2.03 1.58c-.05.3-.09.63-.09.94s.02.64.07.94l-2.03 1.58a.49.49 0 0 0-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32a.49.49 0 0 0-.12-.61l-2.01-1.58zM12 15.6a3.6 3.6 0 1 1 0-7.2 3.6 3.6 0 0 1 0 7.2z";

/// Mic icon - Microphone / Voice input
pub const PATH_MIC: &str = "M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3zm5.91-3c-.49 0-.9.36-.98.85C16.52 14.2 14.47 16 12 16s-4.52-1.8-4.93-4.15c-.08-.49-.49-.85-.98-.85-.61 0-1.09.54-1 1.14.49 3 2.89 5.35 5.91 5.78V20h-2a1 1 0 1 0 0 2h6a1 1 0 1 0 0-2h-2v-2.08c3.02-.43 5.42-2.78 5.91-5.78.1-.6-.39-1.14-1-1.14z";

/// Headphones icon - Deafen / Audio output
pub const PATH_HEADPHONES: &str = "M12 3a9 9 0 0 0-9 9v7c0 1.1.9 2 2 2h3a2 2 0 0 0 2-2v-4a2 2 0 0 0-2-2H5v-1a7 7 0 1 1 14 0v1h-3a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h3a2 2 0 0 0 2-2v-7a9 9 0 0 0-9-9z";

/// Pin icon - Pinned messages
pub const PATH_PIN: &str = "M16 3a1 1 0 0 1 .71.29l4 4a1 1 0 0 1 0 1.42l-2.47 2.47 1.05 3.16a1 1 0 0 1-.24 1.02l-2.12 2.12a1 1 0 0 1-1.41 0L12.05 14l-4.59 4.59-1.41-1.41 4.59-4.59L7.17 9.12a1 1 0 0 1 0-1.41l2.12-2.12a1 1 0 0 1 1.02-.24l3.16 1.05L15.94 3.93A1 1 0 0 1 16 3z";

/// Search icon - Magnifying glass / search bar
pub const PATH_SEARCH: &str = "M21.71 20.29l-5.01-5.01C17.54 13.9 18 12.01 18 10a8 8 0 1 0-8 8c2.01 0 3.9-.46 5.28-1.3l5.01 5.01a1 1 0 0 0 1.42-1.42zM4 10a6 6 0 1 1 6 6 6 6 0 0 1-6-6z";

/// Inbox icon - Mentions / Notifications inbox
pub const PATH_INBOX: &str = "M19 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2zm0 2v7h-3.13a3 3 0 0 1-5.74 0H5V5h14z";

/// Members icon - Member list toggle (group / people)
pub const PATH_MEMBERS: &str = "M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3zm-8 0c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5C6.34 5 5 6.34 5 8s1.34 3 3 3zm0 2c-2.33 0-7 1.17-7 3.5V19h14v-2.5c0-2.33-4.67-3.5-7-3.5zm8 0c-.29 0-.62.02-.97.05 1.16.84 1.97 1.97 1.97 3.45V19h6v-2.5c0-2.33-4.67-3.5-7-3.5z";

/// Gift icon - Nitro gift box
pub const PATH_GIFT: &str = "M20 6h-2.18c.11-.31.18-.65.18-1 0-1.66-1.34-3-3-3-1.62 0-2.95 1.19-3 2.76L12 4.76 11.99 4.75C11.95 3.19 10.62 2 9 2 7.34 2 6 3.34 6 5c0 .35.07.69.18 1H4c-1.11 0-1.99.89-1.99 2L2 19c0 1.11.89 2 2 2h16c1.11 0 2-.89 2-2V8c0-1.11-.89-2-2-2zm-5-2c.55 0 1 .45 1 1s-.45 1-1 1h-2.22c.12-.58.6-1.01 1.22-1.01zm-6 0c.62 0 1.1.43 1.22 1.01H8c0-.55.45-1 1-1zm11 15H4V8h16v11zm-9-9h2v7h-2z";

/// GIF icon - GIF picker button
pub const PATH_GIF: &str = "M3 5a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h18a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2H3zm5 9H6v-4h3v1.5H7.5v1H9V14zm2-4h1.5v4H10v-4zm4 0h3v1.5h-1.5v1H17v1.5h-3v-4z";

/// Sticker icon - Sticker picker
pub const PATH_STICKER: &str = "M18.5 2h-13C3.57 2 2 3.57 2 5.5v13C2 20.43 3.57 22 5.5 22h7.38c.66 0 1.3-.26 1.77-.73l5.62-5.62c.47-.47.73-1.11.73-1.77V5.5C22 3.57 20.43 2 18.5 2zM14 20.17V16c0-1.1.9-2 2-2h4.17L14 20.17z";

/// Emoji icon - Emoji picker (smiley face)
pub const PATH_EMOJI: &str = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm3.5-9c.83 0 1.5-.67 1.5-1.5S16.33 8 15.5 8 14 8.67 14 9.5s.67 1.5 1.5 1.5zm-7 0c.83 0 1.5-.67 1.5-1.5S9.33 8 8.5 8 7 8.67 7 9.5 7.67 11 8.5 11zm3.5 6.5c2.33 0 4.31-1.46 5.11-3.5H6.89c.8 2.04 2.78 3.5 5.11 3.5z";

/// Plus icon - Add server / upload file
pub const PATH_PLUS: &str = "M19 11h-6V5a1 1 0 0 0-2 0v6H5a1 1 0 0 0 0 2h6v6a1 1 0 0 0 2 0v-6h6a1 1 0 0 0 0-2z";

/// Compass icon - Explore / discover public servers
pub const PATH_COMPASS: &str = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm2.19-12.19l-5.66 2.47a.5.5 0 0 0-.26.26l-2.47 5.66a.5.5 0 0 0 .65.65l5.66-2.47a.5.5 0 0 0 .26-.26l2.47-5.66a.5.5 0 0 0-.65-.65zm-2.19 5.19a1 1 0 1 1 0-2 1 1 0 0 1 0 2z";

/// Login icon - Door with entry arrow (sign-in)
pub const PATH_LOGIN: &str = "M10 17l5-5-5-5v3H3v4h7v3zm9-14H5c-1.11 0-2 .9-2 2v4h2V5h14v14H5v-4H3v4c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2z";

/// Key icon - Secret token / credential / auth
pub const PATH_KEY: &str = "M12.65 10C11.83 7.67 9.61 6 7 6c-3.31 0-6 2.69-6 6s2.69 6 6 6c2.61 0 4.83-1.67 5.65-4H17v4h4v-4h2v-4H12.65zM7 14c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2z";

/// Wifi icon - Wireless / network connection
pub const PATH_WIFI: &str = "M12 4C7.31 4 3.07 5.9 0 8.98L12 21 24 8.98A16.88 16.88 0 0 0 12 4zm0 4c3.34 0 6.39 1.25 8.74 3.31L12 19.82 3.26 11.31C5.61 9.25 8.66 8 12 8zm0 5c1.66 0 3.19.64 4.36 1.7L12 19.14 7.64 14.7C8.81 13.64 10.34 13 12 13z";

/// Connection status icon (alias for wifi)
pub const PATH_CONNECTION: &str = PATH_WIFI;

/// Lock icon - Padlock / secure channel / token security
pub const PATH_LOCK: &str = "M18 8h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zm-6 9c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zm3.1-9H8.9V6c0-1.71 1.39-3.1 3.1-3.1 1.71 0 3.1 1.39 3.1 3.1v2z";

/// Warning icon - Alert triangle with exclamation mark
pub const PATH_WARNING: &str = "M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z";

/// Info icon - Informational circle with "i"
pub const PATH_INFO: &str = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z";

/// Bot icon - Robot head for bot tags and automated members
pub const PATH_BOT: &str = "M12 2a2 2 0 0 1 2 2v1h4a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h4V4a2 2 0 0 1 2-2zm-3 8a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3zm6 0a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3zm-6 5.5a.75.75 0 0 0 0 1.5h6a.75.75 0 0 0 0-1.5H9z";

/// User icon - Single user profile / Direct Message avatar
pub const PATH_USER: &str = "M12 12c2.76 0 5-2.24 5-5s-2.24-5-5-5-5 2.24-5 5 2.24 5 5 5zm0 2c-3.33 0-10 1.67-10 5v2a1 1 0 0 0 1 1h18a1 1 0 0 0 1-1v-2c0-3.33-6.67-5-10-5z";

/// Eye icon - Reveal / show secret password or token
pub const PATH_EYE: &str = "M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z";

/// EyeOff icon - Mask / hide secret password or token
pub const PATH_EYE_OFF: &str = "M12 7c2.76 0 5 2.24 5 5 0 .65-.13 1.26-.36 1.83l2.92 2.92c1.51-1.26 2.7-2.89 3.44-4.75-1.73-4.39-6-7.5-11-7.5-1.4 0-2.74.25-3.98.7l2.16 2.16C10.74 7.13 11.35 7 12 7zM2 4.27l2.28 2.28.46.46C3.08 8.3 1.78 10.02 1 12c1.73 4.39 6 7.5 11 7.5 1.55 0 3.03-.3 4.38-.84l.42.42L19.73 22 21 20.73 3.27 3 2 4.27zM7.53 9.8l1.55 1.55c-.05.21-.08.43-.08.65 0 1.66 1.34 3 3 3 .22 0 .44-.03.65-.08l1.55 1.55c-.67.33-1.41.53-2.2.53-2.76 0-5-2.24-5-5 0-.79.2-1.53.53-2.2zm4.31-.78l3.15 3.15.02-.17c0-1.66-1.34-3-3-3l-.17.02z";

/// Paste icon - Clipboard paste for tokens and messages
pub const PATH_PASTE: &str = "M19 2h-4.18C14.4 0.84 13.3 0 12 0c-1.3 0-2.4.84-2.82 2H5c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-7 0c.55 0 1 .45 1 1s-.45 1-1 1-1-.45-1-1 .45-1 1-1zm7 18H5V4h2v3h10V4h2v16z";

/// Bell icon - Notifications / unread channel alerts
pub const PATH_BELL: &str = "M12 22c1.1 0 2-.9 2-2h-4c0 1.1.89 2 2 2zm6-6v-5c0-3.07-1.64-5.64-4.5-6.32V4c0-.83-.67-1.5-1.5-1.5s-1.5.67-1.5 1.5v.68C7.63 5.36 6 7.92 6 11v5l-2 2v1h16v-1l-2-2z";

/// Send icon - Paper airplane / message dispatch
pub const PATH_SEND: &str = "M2.01 21L23 12 2.01 3 2 10l15 2-15 2z";

/// Close icon - Dismiss / cancel / clear button (✕)
pub const PATH_CLOSE: &str = "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z";

/// Check icon - Confirm / checkmark (✓)
pub const PATH_CHECK: &str = "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z";

/// Copy icon - Copy to clipboard
pub const PATH_COPY: &str = "M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z";

/// Chat / Message bubble icon (Direct Message / DMs)
pub const PATH_CHAT: &str = "M4.798 3h14.404C20.198 3 21 3.794 21 4.767v10.466C21 16.206 20.198 17 19.202 17H7.485L4.473 19.787A.75.75 0 0 1 3.25 19.23V4.767C3.25 3.794 4.052 3 5.048 3h-.25z";

/// Friends / People icon (Friends Hub)
pub const PATH_FRIENDS: &str = "M14 8a4 4 0 1 0-8 0 4 4 0 0 0 8 0zm-2 9c-4.42 0-8 1.79-8 4v1h16v-1c0-2.21-3.58-4-8-4zm8-6a3 3 0 1 0-6 0 3 3 0 0 0 6 0zm0 7c-1.63 0-3.13.39-4.38 1.07.64.88 1.05 1.94 1.12 3.08L22 20.15v-.15c0-1.66-2.69-3-6-3z";

/// Folder icon (Server category / folder)
pub const PATH_FOLDER: &str = "M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z";

/// Sparkles / Boost / Nitro icon
pub const PATH_SPARKLES: &str = "M12 1L9 9l-8 3 8 3 3 8 3-8 8-3-8-3-3-8z";

/// Shield icon (Security / Family Center / Spam protection)
pub const PATH_SHIELD: &str = "M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm0 10.99h7c-.53 4.12-3.28 7.79-7 8.94V12H5V6.3l7-3.11v8.8z";

/// Gamepad / Game activity icon
pub const PATH_GAMEPAD: &str = "M21 6H3c-1.1 0-2 .9-2 2v8c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm-10 7H8v3H6v-3H3v-2h3V8h2v3h3v2zm4.5 2c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zm3-3c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5z";

/// Music / Spotify icon
pub const PATH_MUSIC: &str = "M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z";

/// Phone / Voice Call start icon
pub const PATH_PHONE: &str = "M6.62 10.79a15.053 15.053 0 0 0 6.59 6.59l2.2-2.2c.27-.27.67-.36 1.02-.24 1.12.37 2.33.57 3.57.57.55 0 1 .45 1 1V20c0 .55-.45 1-1 1-9.39 0-17-7.61-17-17 0-.55.45-1 1-1h3.5c.55 0 1 .45 1 1 0 1.25.2 2.45.57 3.57.11.35.03.74-.25 1.02l-2.2 2.2z";

/// Phone Hangup / End Call icon
pub const PATH_PHONE_HANGUP: &str = "M12 9c-1.6 0-3.15.25-4.6.72v3.1c0 .39-.23.74-.56.9-.98.49-1.87 1.12-2.66 1.85-.18.18-.43.28-.7.28-.28 0-.53-.11-.71-.29L.29 13.08a.99.99 0 0 1 0-1.41C3.28 8.91 7.42 7.25 12 7.25s8.72 1.66 11.71 4.42c.39.39.39 1.02 0 1.41l-2.48 2.48c-.18.18-.43.29-.71.29-.27 0-.52-.11-.7-.28-.79-.74-1.69-1.36-2.67-1.85-.33-.16-.56-.5-.56-.9v-3.1C15.15 9.25 13.6 9 12 9z";

/// Screen share / Display icon
pub const PATH_SCREEN_SHARE: &str = "M20 18c1.1 0 1.99-.9 1.99-2L22 6c0-1.1-.9-2-2-2H4c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2H0v2h24v-2h-4zM4 6h16v10H4V6z";

/// Mic Muted icon
pub const PATH_MIC_OFF: &str = "M19 11h-1.7c0 .74-.16 1.43-.43 2.05l1.23 1.23c.56-.98.9-2.09.9-3.28zm-4.02.17L10.8 7.01c.07-.01.13-.01.2-.01 1.66 0 3 1.34 3 3v1.17zM4.41 2.86L3 4.27l6 6V11c0 1.66 1.34 3 3 3 .23 0 .44-.03.65-.08l1.66 1.66c-.71.33-1.5.52-2.31.52-2.76 0-5.3-2.1-5.3-5.1H5c0 3.41 2.72 6.23 6 6.72V21h2v-3.28c.91-.14 1.77-.45 2.55-.9l3.18 3.18 1.41-1.41L4.41 2.86z";

/// Headphones Muted / Deafened icon
pub const PATH_HEADPHONES_OFF: &str = "M12 4c3.87 0 7 3.13 7 7v1h-2v4h2v3h-3v2h3a2 2 0 0 0 2-2v-8a9 9 0 0 0-9-9c-1.63 0-3.15.44-4.47 1.21l1.46 1.46C8.98 4.25 10.44 4 12 4zM2.81 2.81L1.39 4.22l3.29 3.29A8.954 8.954 0 0 0 3 12v8a2 2 0 0 0 2 2h3v-2H5v-7h2v-2.17l5.9 5.9L12 16.73 19.78 24.5l1.41-1.41L2.81 2.81z";

/// Help / Question mark icon
pub const PATH_HELP: &str = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 16h-2v-2h2v2zm1.07-7.75l-.9.92C12.45 11.9 12 12.5 12 14h-2v-.5c0-1.1.45-2.1 1.17-2.83l1.24-1.26c.37-.36.59-.86.59-1.41 0-1.1-.9-2-2-2s-2 .9-2 2H7c0-2.76 2.24-5 5-5s5 2.24 5 5c0 .88-.36 1.68-.93 2.25z";

/// Smile / Reaction smiley icon
pub const PATH_SMILE: &str = "M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8zm3.5-9c.83 0 1.5-.67 1.5-1.5S16.33 8 15.5 8 14 8.67 14 9.5s.67 1.5 1.5 1.5zm-7 0c.83 0 1.5-.67 1.5-1.5S9.33 8 8.5 8 7 8.67 7 9.5 7.67 11 8.5 11zm3.5 6.5c2.33 0 4.31-1.46 5.11-3.5H6.89c.8 2.04 2.78 3.5 5.11 3.5z";

/// GitHub brand icon
pub const PATH_GITHUB: &str = "M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0 0 22 12.017C22 6.484 17.522 2 12 2z";

/// Twitch brand icon
pub const PATH_TWITCH: &str = "M2.149 0L.537 4.119v16.8h5.925V24h3.224l3.224-3.081h4.836L23.463 15.164V0H2.149zm19.164 14.075l-3.224 3.081h-5.373l-3.224 3.081v-3.081H5.373V2.149h15.94v11.926zm-7.791-7.254h2.149v6.448h-2.149V6.821zm-5.373 0h2.149v6.448H8.149V6.821z";

/// YouTube brand icon
pub const PATH_YOUTUBE: &str = "M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z";

/// Link / chain connection icon
pub const PATH_LINK: &str = "M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1h-4V17h4c2.76 0 5-2.24 5-5s-2.24-5-5-5z";

// ============================================================================
// Full SVG Markup Constants (viewBox="0 0 24 24", fill="currentColor")
// ============================================================================

pub const SVG_HASH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M5.887 21a.75.75 0 0 1-.74-.89l.83-4.52H3.643a.75.75 0 0 1-.74-.89l.3-1.61a.75.75 0 0 1 .74-.62h2.75l.73-4H4.887a.75.75 0 0 1-.74-.89l.3-1.61a.75.75 0 0 1 .74-.62h2.75l.83-4.53a.75.75 0 0 1 .74-.62h1.61a.75.75 0 0 1 .74.89l-.78 4.26h4l.83-4.53a.75.75 0 0 1 .74-.62h1.61a.75.75 0 0 1 .74.89l-.78 4.26h2.51a.75.75 0 0 1 .74.89l-.3 1.61a.75.75 0 0 1-.74.62h-2.75l-.73 4h2.51a.75.75 0 0 1 .74.89l-.3 1.61a.75.75 0 0 1-.74.62h-2.75l-.83 4.53a.75.75 0 0 1-.74.62h-1.61a.75.75 0 0 1-.74-.89l.78-4.26h-4l-.83 4.53a.75.75 0 0 1-.74.62H5.887zm2.75-7.94h4l.73-4h-4l-.73 4z"/></svg>"#;
pub const SVG_SPEAKER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M11.38 3.08a1.2 1.2 0 0 0-1.09.22L6 7.59H3c-.55 0-1 .45-1 1v6.82c0 .55.45 1 1 1h3l4.29 4.29a1.2 1.2 0 0 0 1.09.22c.45-.18.71-.62.71-1.1V4.18c0-.48-.26-.92-.71-1.1zm4.33 4.21a1 1 0 0 0-1.42 1.42c1.25 1.25 1.96 2.94 1.96 4.71s-.71 3.46-1.96 4.71a1 1 0 0 0 1.42 1.42c1.63-1.63 2.54-3.81 2.54-6.13s-.91-4.5-2.54-6.13zm3.54-3.54a1 1 0 0 0-1.42 1.42C20 7.33 21.25 10.25 21.25 13.41s-1.25 6.08-3.42 8.24a1 1 0 0 0 1.42 1.42C21.78 20.53 23.25 17.1 23.25 13.41s-1.47-7.12-4-9.66z"/></svg>"#;
pub const SVG_MEGAPHONE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M21 3.5v13a1 1 0 0 1-1 1h-1v3a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1v-3h-1.5L7.5 20.5V3.5L12.5 6.5H19v-3a1 1 0 0 1 1-1c.55 0 1 .45 1 1zM6 6.5H4C2.9 6.5 2 7.4 2 8.5v7c0 1.1.9 2 2 2h2v-11z"/></svg>"#;
pub const SVG_GEAR: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58a.49.49 0 0 0 .12-.61l-1.92-3.32a.49.49 0 0 0-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54a.48.48 0 0 0-.48-.41h-3.84c-.24 0-.44.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96a.49.49 0 0 0-.59.22L2.74 8.87a.49.49 0 0 0 .12.61l2.03 1.58c-.05.3-.09.63-.09.94s.02.64.07.94l-2.03 1.58a.49.49 0 0 0-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32a.49.49 0 0 0-.12-.61l-2.01-1.58zM12 15.6a3.6 3.6 0 1 1 0-7.2 3.6 3.6 0 0 1 0 7.2z"/></svg>"#;
pub const SVG_MIC: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3zm5.91-3c-.49 0-.9.36-.98.85C16.52 14.2 14.47 16 12 16s-4.52-1.8-4.93-4.15c-.08-.49-.49-.85-.98-.85-.61 0-1.09.54-1 1.14.49 3 2.89 5.35 5.91 5.78V20h-2a1 1 0 1 0 0 2h6a1 1 0 1 0 0-2h-2v-2.08c3.02-.43 5.42-2.78 5.91-5.78.1-.6-.39-1.14-1-1.14z"/></svg>"#;
pub const SVG_HEADPHONES: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 3a9 9 0 0 0-9 9v7c0 1.1.9 2 2 2h3a2 2 0 0 0 2-2v-4a2 2 0 0 0-2-2H5v-1a7 7 0 1 1 14 0v1h-3a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h3a2 2 0 0 0 2-2v-7a9 9 0 0 0-9-9z"/></svg>"#;
pub const SVG_PIN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M16 3a1 1 0 0 1 .71.29l4 4a1 1 0 0 1 0 1.42l-2.47 2.47 1.05 3.16a1 1 0 0 1-.24 1.02l-2.12 2.12a1 1 0 0 1-1.41 0L12.05 14l-4.59 4.59-1.41-1.41 4.59-4.59L7.17 9.12a1 1 0 0 1 0-1.41l2.12-2.12a1 1 0 0 1 1.02-.24l3.16 1.05L15.94 3.93A1 1 0 0 1 16 3z"/></svg>"#;
pub const SVG_SEARCH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M21.71 20.29l-5.01-5.01C17.54 13.9 18 12.01 18 10a8 8 0 1 0-8 8c2.01 0 3.9-.46 5.28-1.3l5.01 5.01a1 1 0 0 0 1.42-1.42zM4 10a6 6 0 1 1 6 6 6 6 0 0 1-6-6z"/></svg>"#;
pub const SVG_INBOX: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M19 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2zm0 2v7h-3.13a3 3 0 0 1-5.74 0H5V5h14z"/></svg>"#;
pub const SVG_MEMBERS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3zm-8 0c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5C6.34 5 5 6.34 5 8s1.34 3 3 3zm0 2c-2.33 0-7 1.17-7 3.5V19h14v-2.5c0-2.33-4.67-3.5-7-3.5zm8 0c-.29 0-.62.02-.97.05 1.16.84 1.97 1.97 1.97 3.45V19h6v-2.5c0-2.33-4.67-3.5-7-3.5z"/></svg>"#;
pub const SVG_GIFT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M20 6h-2.18c.11-.31.18-.65.18-1 0-1.66-1.34-3-3-3-1.62 0-2.95 1.19-3 2.76L12 4.76 11.99 4.75C11.95 3.19 10.62 2 9 2 7.34 2 6 3.34 6 5c0 .35.07.69.18 1H4c-1.11 0-1.99.89-1.99 2L2 19c0 1.11.89 2 2 2h16c1.11 0 2-.89 2-2V8c0-1.11-.89-2-2-2zm-5-2c.55 0 1 .45 1 1s-.45 1-1 1h-2.22c.12-.58.6-1.01 1.22-1.01zm-6 0c.62 0 1.1.43 1.22 1.01H8c0-.55.45-1 1-1zm11 15H4V8h16v11zm-9-9h2v7h-2z"/></svg>"#;
pub const SVG_GIF: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M3 5a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h18a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2H3zm5 9H6v-4h3v1.5H7.5v1H9V14zm2-4h1.5v4H10v-4zm4 0h3v1.5h-1.5v1H17v1.5h-3v-4z"/></svg>"#;
pub const SVG_STICKER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M18.5 2h-13C3.57 2 2 3.57 2 5.5v13C2 20.43 3.57 22 5.5 22h7.38c.66 0 1.3-.26 1.77-.73l5.62-5.62c.47-.47.73-1.11.73-1.77V5.5C22 3.57 20.43 2 18.5 2zM14 20.17V16c0-1.1.9-2 2-2h4.17L14 20.17z"/></svg>"#;
pub const SVG_EMOJI: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm3.5-9c.83 0 1.5-.67 1.5-1.5S16.33 8 15.5 8 14 8.67 14 9.5s.67 1.5 1.5 1.5zm-7 0c.83 0 1.5-.67 1.5-1.5S9.33 8 8.5 8 7 8.67 7 9.5 7.67 11 8.5 11zm3.5 6.5c2.33 0 4.31-1.46 5.11-3.5H6.89c.8 2.04 2.78 3.5 5.11 3.5z"/></svg>"#;
pub const SVG_PLUS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M19 11h-6V5a1 1 0 0 0-2 0v6H5a1 1 0 0 0 0 2h6v6a1 1 0 0 0 2 0v-6h6a1 1 0 0 0 0-2z"/></svg>"#;
pub const SVG_COMPASS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm2.19-12.19l-5.66 2.47a.5.5 0 0 0-.26.26l-2.47 5.66a.5.5 0 0 0 .65.65l5.66-2.47a.5.5 0 0 0 .26-.26l2.47-5.66a.5.5 0 0 0-.65-.65zm-2.19 5.19a1 1 0 1 1 0-2 1 1 0 0 1 0 2z"/></svg>"#;
pub const SVG_LOGIN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M10 17l5-5-5-5v3H3v4h7v3zm9-14H5c-1.11 0-2 .9-2 2v4h2V5h14v14H5v-4H3v4c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2z"/></svg>"#;
pub const SVG_KEY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12.65 10C11.83 7.67 9.61 6 7 6c-3.31 0-6 2.69-6 6s2.69 6 6 6c2.61 0 4.83-1.67 5.65-4H17v4h4v-4h2v-4H12.65zM7 14c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2z"/></svg>"#;
pub const SVG_WIFI: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 4C7.31 4 3.07 5.9 0 8.98L12 21 24 8.98A16.88 16.88 0 0 0 12 4zm0 4c3.34 0 6.39 1.25 8.74 3.31L12 19.82 3.26 11.31C5.61 9.25 8.66 8 12 8zm0 5c1.66 0 3.19.64 4.36 1.7L12 19.14 7.64 14.7C8.81 13.64 10.34 13 12 13z"/></svg>"#;
pub const SVG_CONNECTION: &str = SVG_WIFI;
pub const SVG_LOCK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M18 8h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zm-6 9c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zm3.1-9H8.9V6c0-1.71 1.39-3.1 3.1-3.1 1.71 0 3.1 1.39 3.1 3.1v2z"/></svg>"#;
pub const SVG_WARNING: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z"/></svg>"#;
pub const SVG_INFO: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z"/></svg>"#;

pub const SVG_BOT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2a2 2 0 0 1 2 2v1h4a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h4V4a2 2 0 0 1 2-2zm-3 8a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3zm6 0a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3zm-6 5.5a.75.75 0 0 0 0 1.5h6a.75.75 0 0 0 0-1.5H9z"/></svg>"#;
pub const SVG_USER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 12c2.76 0 5-2.24 5-5s-2.24-5-5-5-5 2.24-5 5 2.24 5 5 5zm0 2c-3.33 0-10 1.67-10 5v2a1 1 0 0 0 1 1h18a1 1 0 0 0 1-1v-2c0-3.33-6.67-5-10-5z"/></svg>"#;
pub const SVG_EYE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z"/></svg>"#;
pub const SVG_EYE_OFF: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 7c2.76 0 5 2.24 5 5 0 .65-.13 1.26-.36 1.83l2.92 2.92c1.51-1.26 2.7-2.89 3.44-4.75-1.73-4.39-6-7.5-11-7.5-1.4 0-2.74.25-3.98.7l2.16 2.16C10.74 7.13 11.35 7 12 7zM2 4.27l2.28 2.28.46.46C3.08 8.3 1.78 10.02 1 12c1.73 4.39 6 7.5 11 7.5 1.55 0 3.03-.3 4.38-.84l.42.42L19.73 22 21 20.73 3.27 3 2 4.27zM7.53 9.8l1.55 1.55c-.05.21-.08.43-.08.65 0 1.66 1.34 3 3 3 .22 0 .44-.03.65-.08l1.55 1.55c-.67.33-1.41.53-2.2.53-2.76 0-5-2.24-5-5 0-.79.2-1.53.53-2.2zm4.31-.78l3.15 3.15.02-.17c0-1.66-1.34-3-3-3l-.17.02z"/></svg>"#;
pub const SVG_PASTE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M19 2h-4.18C14.4 0.84 13.3 0 12 0c-1.3 0-2.4.84-2.82 2H5c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-7 0c.55 0 1 .45 1 1s-.45 1-1 1-1-.45-1-1 .45-1 1-1zm7 18H5V4h2v3h10V4h2v16z"/></svg>"#;
pub const SVG_BELL: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 22c1.1 0 2-.9 2-2h-4c0 1.1.89 2 2 2zm6-6v-5c0-3.07-1.64-5.64-4.5-6.32V4c0-.83-.67-1.5-1.5-1.5s-1.5.67-1.5 1.5v.68C7.63 5.36 6 7.92 6 11v5l-2 2v1h16v-1l-2-2z"/></svg>"#;
pub const SVG_SEND: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"/></svg>"#;
pub const SVG_CLOSE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg>"#;
pub const SVG_CHECK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/></svg>"#;
pub const SVG_COPY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"/></svg>"#;
pub const SVG_CHAT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M4.798 3h14.404C20.198 3 21 3.794 21 4.767v10.466C21 16.206 20.198 17 19.202 17H7.485L4.473 19.787A.75.75 0 0 1 3.25 19.23V4.767C3.25 3.794 4.052 3 5.048 3h-.25z"/></svg>"#;
pub const SVG_FRIENDS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M14 8a4 4 0 1 0-8 0 4 4 0 0 0 8 0zm-2 9c-4.42 0-8 1.79-8 4v1h16v-1c0-2.21-3.58-4-8-4zm8-6a3 3 0 1 0-6 0 3 3 0 0 0 6 0zm0 7c-1.63 0-3.13.39-4.38 1.07.64.88 1.05 1.94 1.12 3.08L22 20.15v-.15c0-1.66-2.69-3-6-3z"/></svg>"#;
pub const SVG_FOLDER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/></svg>"#;
pub const SVG_SPARKLES: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 1L9 9l-8 3 8 3 3 8 3-8 8-3-8-3-3-8z"/></svg>"#;
pub const SVG_SHIELD: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm0 10.99h7c-.53 4.12-3.28 7.79-7 8.94V12H5V6.3l7-3.11v8.8z"/></svg>"#;
pub const SVG_GAMEPAD: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M21 6H3c-1.1 0-2 .9-2 2v8c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm-10 7H8v3H6v-3H3v-2h3V8h2v3h3v2zm4.5 2c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zm3-3c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5z"/></svg>"#;
pub const SVG_MUSIC: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/></svg>"#;
pub const SVG_PHONE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M6.62 10.79a15.053 15.053 0 0 0 6.59 6.59l2.2-2.2c.27-.27.67-.36 1.02-.24 1.12.37 2.33.57 3.57.57.55 0 1 .45 1 1V20c0 .55-.45 1-1 1-9.39 0-17-7.61-17-17 0-.55.45-1 1-1h3.5c.55 0 1 .45 1 1 0 1.25.2 2.45.57 3.57.11.35.03.74-.25 1.02l-2.2 2.2z"/></svg>"#;
pub const SVG_PHONE_HANGUP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 9c-1.6 0-3.15.25-4.6.72v3.1c0 .39-.23.74-.56.9-.98.49-1.87 1.12-2.66 1.85-.18.18-.43.28-.7.28-.28 0-.53-.11-.71-.29L.29 13.08a.99.99 0 0 1 0-1.41C3.28 8.91 7.42 7.25 12 7.25s8.72 1.66 11.71 4.42c.39.39.39 1.02 0 1.41l-2.48 2.48c-.18.18-.43.29-.71.29-.27 0-.52-.11-.7-.28-.79-.74-1.69-1.36-2.67-1.85-.33-.16-.56-.5-.56-.9v-3.1C15.15 9.25 13.6 9 12 9z"/></svg>"#;
pub const SVG_SCREEN_SHARE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M20 18c1.1 0 1.99-.9 1.99-2L22 6c0-1.1-.9-2-2-2H4c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2H0v2h24v-2h-4zM4 6h16v10H4V6z"/></svg>"#;
pub const SVG_MIC_OFF: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M19 11h-1.7c0 .74-.16 1.43-.43 2.05l1.23 1.23c.56-.98.9-2.09.9-3.28zm-4.02.17L10.8 7.01c.07-.01.13-.01.2-.01 1.66 0 3 1.34 3 3v1.17zM4.41 2.86L3 4.27l6 6V11c0 1.66 1.34 3 3 3 .23 0 .44-.03.65-.08l1.66 1.66c-.71.33-1.5.52-2.31.52-2.76 0-5.3-2.1-5.3-5.1H5c0 3.41 2.72 6.23 6 6.72V21h2v-3.28c.91-.14 1.77-.45 2.55-.9l3.18 3.18 1.41-1.41L4.41 2.86z"/></svg>"#;
pub const SVG_HEADPHONES_OFF: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 4c3.87 0 7 3.13 7 7v1h-2v4h2v3h-3v2h3a2 2 0 0 0 2-2v-8a9 9 0 0 0-9-9c-1.63 0-3.15.44-4.47 1.21l1.46 1.46C8.98 4.25 10.44 4 12 4zM2.81 2.81L1.39 4.22l3.29 3.29A8.954 8.954 0 0 0 3 12v8a2 2 0 0 0 2 2h3v-2H5v-7h2v-2.17l5.9 5.9L12 16.73 19.78 24.5l1.41-1.41L2.81 2.81z"/></svg>"#;
pub const SVG_HELP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 16h-2v-2h2v2zm1.07-7.75l-.9.92C12.45 11.9 12 12.5 12 14h-2v-.5c0-1.1.45-2.1 1.17-2.83l1.24-1.26c.37-.36.59-.86.59-1.41 0-1.1-.9-2-2-2s-2 .9-2 2H7c0-2.76 2.24-5 5-5s5 2.24 5 5c0 .88-.36 1.68-.93 2.25z"/></svg>"#;
pub const SVG_SMILE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8zm3.5-9c.83 0 1.5-.67 1.5-1.5S16.33 8 15.5 8 14 8.67 14 9.5s.67 1.5 1.5 1.5zm-7 0c.83 0 1.5-.67 1.5-1.5S9.33 8 8.5 8 7 8.67 7 9.5 7.67 11 8.5 11zm3.5 6.5c2.33 0 4.31-1.46 5.11-3.5H6.89c.8 2.04 2.78 3.5 5.11 3.5z"/></svg>"#;
pub const SVG_GITHUB: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0 0 22 12.017C22 6.484 17.522 2 12 2z"/></svg>"#;
pub const SVG_TWITCH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M2.149 0L.537 4.119v16.8h5.925V24h3.224l3.224-3.081h4.836L23.463 15.164V0H2.149zm19.164 14.075l-3.224 3.081h-5.373l-3.224 3.081v-3.081H5.373V2.149h15.94v11.926zm-7.791-7.254h2.149v6.448h-2.149V6.821zm-5.373 0h2.149v6.448H8.149V6.821z"/></svg>"#;
pub const SVG_YOUTUBE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z"/></svg>"#;
pub const SVG_LINK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1h-4V17h4c2.76 0 5-2.24 5-5s-2.24-5-5-5z"/></svg>"#;

// ============================================================================
// DiscordIcon Enum
// ============================================================================

/// Canonical Discord user interface icons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiscordIcon {
    /// Text channel (#)
    Hash,
    /// Voice channel / volume
    Speaker,
    /// Announcement channel
    Megaphone,
    /// Settings / cog
    Gear,
    /// Microphone / mute
    Mic,
    /// Headphones / deafen
    Headphones,
    /// Pinned messages
    Pin,
    /// Search magnifying glass
    Search,
    /// Inbox / notifications
    Inbox,
    /// Member list toggle
    Members,
    /// Nitro gift
    Gift,
    /// GIF picker
    Gif,
    /// Sticker picker
    Sticker,
    /// Emoji picker (smiley)
    Emoji,
    /// Add server / upload
    Plus,
    /// Compass / explore discover servers
    Compass,
    /// Login / sign in
    Login,
    /// Key / authentication token
    Key,
    /// Wifi / wireless network
    Wifi,
    /// Connection status
    Connection,
    /// Lock / security
    Lock,
    /// Warning / alert
    Warning,
    /// Info / help
    Info,
    /// Bot / robot tag
    Bot,
    /// User profile / DM avatar
    User,
    /// Eye (show secret)
    Eye,
    /// EyeOff (hide secret)
    EyeOff,
    /// Paste clipboard
    Paste,
    /// Notification bell
    Bell,
    /// Send message plane
    Send,
    /// Close / dismiss ✕
    Close,
    /// Checkmark ✓
    Check,
    /// Copy to clipboard
    Copy,
    /// Direct Message / Chat bubble
    Chat,
    /// Friends / People
    Friends,
    /// Folder / Server folder
    Folder,
    /// Sparkles / Boost / Nitro
    Sparkles,
    /// Shield / Security / Verification
    Shield,
    /// Gamepad / Game activity
    Gamepad,
    /// Music / Spotify
    Music,
    /// Phone / Voice Call
    Phone,
    /// Phone Hangup / End Call
    PhoneHangup,
    /// Screen share / Display
    ScreenShare,
    /// Mic Muted
    MicOff,
    /// Headphones Muted / Deafened
    HeadphonesOff,
    /// Help / Question
    Help,
    /// Reaction smiley
    Smile,
    /// GitHub brand
    Github,
    /// Twitch brand
    Twitch,
    /// YouTube brand
    Youtube,
    /// Link / chain connection
    Link,
}

impl DiscordIcon {
    /// Canonical name of the icon
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Hash => "hash",
            Self::Speaker => "speaker",
            Self::Megaphone => "megaphone",
            Self::Gear => "gear",
            Self::Mic => "mic",
            Self::Headphones => "headphones",
            Self::Pin => "pin",
            Self::Search => "search",
            Self::Inbox => "inbox",
            Self::Members => "members",
            Self::Gift => "gift",
            Self::Gif => "gif",
            Self::Sticker => "sticker",
            Self::Emoji => "emoji",
            Self::Plus => "plus",
            Self::Compass => "compass",
            Self::Login => "login",
            Self::Key => "key",
            Self::Wifi => "wifi",
            Self::Connection => "connection",
            Self::Lock => "lock",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Bot => "bot",
            Self::User => "user",
            Self::Eye => "eye",
            Self::EyeOff => "eye_off",
            Self::Paste => "paste",
            Self::Bell => "bell",
            Self::Send => "send",
            Self::Close => "close",
            Self::Check => "check",
            Self::Copy => "copy",
            Self::Chat => "chat",
            Self::Friends => "friends",
            Self::Folder => "folder",
            Self::Sparkles => "sparkles",
            Self::Shield => "shield",
            Self::Gamepad => "gamepad",
            Self::Music => "music",
            Self::Phone => "phone",
            Self::PhoneHangup => "phone_hangup",
            Self::ScreenShare => "screen_share",
            Self::MicOff => "mic_off",
            Self::HeadphonesOff => "headphones_off",
            Self::Help => "help",
            Self::Smile => "smile",
            Self::Github => "github",
            Self::Twitch => "twitch",
            Self::Youtube => "youtube",
            Self::Link => "link",
        }
    }

    /// SVG path `d` attribute string
    pub const fn path_data(&self) -> &'static str {
        match self {
            Self::Hash => PATH_HASH,
            Self::Speaker => PATH_SPEAKER,
            Self::Megaphone => PATH_MEGAPHONE,
            Self::Gear => PATH_GEAR,
            Self::Mic => PATH_MIC,
            Self::Headphones => PATH_HEADPHONES,
            Self::Pin => PATH_PIN,
            Self::Search => PATH_SEARCH,
            Self::Inbox => PATH_INBOX,
            Self::Members => PATH_MEMBERS,
            Self::Gift => PATH_GIFT,
            Self::Gif => PATH_GIF,
            Self::Sticker => PATH_STICKER,
            Self::Emoji => PATH_EMOJI,
            Self::Plus => PATH_PLUS,
            Self::Compass => PATH_COMPASS,
            Self::Login => PATH_LOGIN,
            Self::Key => PATH_KEY,
            Self::Wifi => PATH_WIFI,
            Self::Connection => PATH_CONNECTION,
            Self::Lock => PATH_LOCK,
            Self::Warning => PATH_WARNING,
            Self::Info => PATH_INFO,
            Self::Bot => PATH_BOT,
            Self::User => PATH_USER,
            Self::Eye => PATH_EYE,
            Self::EyeOff => PATH_EYE_OFF,
            Self::Paste => PATH_PASTE,
            Self::Bell => PATH_BELL,
            Self::Send => PATH_SEND,
            Self::Close => PATH_CLOSE,
            Self::Check => PATH_CHECK,
            Self::Copy => PATH_COPY,
            Self::Chat => PATH_CHAT,
            Self::Friends => PATH_FRIENDS,
            Self::Folder => PATH_FOLDER,
            Self::Sparkles => PATH_SPARKLES,
            Self::Shield => PATH_SHIELD,
            Self::Gamepad => PATH_GAMEPAD,
            Self::Music => PATH_MUSIC,
            Self::Phone => PATH_PHONE,
            Self::PhoneHangup => PATH_PHONE_HANGUP,
            Self::ScreenShare => PATH_SCREEN_SHARE,
            Self::MicOff => PATH_MIC_OFF,
            Self::HeadphonesOff => PATH_HEADPHONES_OFF,
            Self::Help => PATH_HELP,
            Self::Smile => PATH_SMILE,
            Self::Github => PATH_GITHUB,
            Self::Twitch => PATH_TWITCH,
            Self::Youtube => PATH_YOUTUBE,
            Self::Link => PATH_LINK,
        }
    }

    /// Full SVG XML markup
    pub const fn svg_data(&self) -> &'static str {
        match self {
            Self::Hash => SVG_HASH,
            Self::Speaker => SVG_SPEAKER,
            Self::Megaphone => SVG_MEGAPHONE,
            Self::Gear => SVG_GEAR,
            Self::Mic => SVG_MIC,
            Self::Headphones => SVG_HEADPHONES,
            Self::Pin => SVG_PIN,
            Self::Search => SVG_SEARCH,
            Self::Inbox => SVG_INBOX,
            Self::Members => SVG_MEMBERS,
            Self::Gift => SVG_GIFT,
            Self::Gif => SVG_GIF,
            Self::Sticker => SVG_STICKER,
            Self::Emoji => SVG_EMOJI,
            Self::Plus => SVG_PLUS,
            Self::Compass => SVG_COMPASS,
            Self::Login => SVG_LOGIN,
            Self::Key => SVG_KEY,
            Self::Wifi => SVG_WIFI,
            Self::Connection => SVG_CONNECTION,
            Self::Lock => SVG_LOCK,
            Self::Warning => SVG_WARNING,
            Self::Info => SVG_INFO,
            Self::Bot => SVG_BOT,
            Self::User => SVG_USER,
            Self::Eye => SVG_EYE,
            Self::EyeOff => SVG_EYE_OFF,
            Self::Paste => SVG_PASTE,
            Self::Bell => SVG_BELL,
            Self::Send => SVG_SEND,
            Self::Close => SVG_CLOSE,
            Self::Check => SVG_CHECK,
            Self::Copy => SVG_COPY,
            Self::Chat => SVG_CHAT,
            Self::Friends => SVG_FRIENDS,
            Self::Folder => SVG_FOLDER,
            Self::Sparkles => SVG_SPARKLES,
            Self::Shield => SVG_SHIELD,
            Self::Gamepad => SVG_GAMEPAD,
            Self::Music => SVG_MUSIC,
            Self::Phone => SVG_PHONE,
            Self::PhoneHangup => SVG_PHONE_HANGUP,
            Self::ScreenShare => SVG_SCREEN_SHARE,
            Self::MicOff => SVG_MIC_OFF,
            Self::HeadphonesOff => SVG_HEADPHONES_OFF,
            Self::Help => SVG_HELP,
            Self::Smile => SVG_SMILE,
            Self::Github => SVG_GITHUB,
            Self::Twitch => SVG_TWITCH,
            Self::Youtube => SVG_YOUTUBE,
            Self::Link => SVG_LINK,
        }
    }

    /// Standard asset source path (e.g. `icons/hash.svg`)
    pub fn asset_path(&self) -> SharedString {
        SharedString::from(format!("icons/{}.svg", self.name()))
    }

    /// Standard viewBox attribute
    pub const fn view_box(&self) -> &'static str {
        "0 0 24 24"
    }
}

/// All canonical Discord icons
pub const ALL_ICONS: [DiscordIcon; 51] = [
    DiscordIcon::Hash,
    DiscordIcon::Speaker,
    DiscordIcon::Megaphone,
    DiscordIcon::Gear,
    DiscordIcon::Mic,
    DiscordIcon::Headphones,
    DiscordIcon::Pin,
    DiscordIcon::Search,
    DiscordIcon::Inbox,
    DiscordIcon::Members,
    DiscordIcon::Gift,
    DiscordIcon::Gif,
    DiscordIcon::Sticker,
    DiscordIcon::Emoji,
    DiscordIcon::Plus,
    DiscordIcon::Compass,
    DiscordIcon::Login,
    DiscordIcon::Key,
    DiscordIcon::Wifi,
    DiscordIcon::Connection,
    DiscordIcon::Lock,
    DiscordIcon::Warning,
    DiscordIcon::Info,
    DiscordIcon::Bot,
    DiscordIcon::User,
    DiscordIcon::Eye,
    DiscordIcon::EyeOff,
    DiscordIcon::Paste,
    DiscordIcon::Bell,
    DiscordIcon::Send,
    DiscordIcon::Close,
    DiscordIcon::Check,
    DiscordIcon::Copy,
    DiscordIcon::Chat,
    DiscordIcon::Friends,
    DiscordIcon::Folder,
    DiscordIcon::Sparkles,
    DiscordIcon::Shield,
    DiscordIcon::Gamepad,
    DiscordIcon::Music,
    DiscordIcon::Phone,
    DiscordIcon::PhoneHangup,
    DiscordIcon::ScreenShare,
    DiscordIcon::MicOff,
    DiscordIcon::HeadphonesOff,
    DiscordIcon::Help,
    DiscordIcon::Smile,
    DiscordIcon::Github,
    DiscordIcon::Twitch,
    DiscordIcon::Youtube,
    DiscordIcon::Link,
];

// ============================================================================
// AssetSource Provider (In-Memory SVG Assets for GPUI)
// ============================================================================

/// In-memory asset source providing all canonical Discord vector icons to GPUI without external files.
#[derive(Debug, Clone, Copy, Default)]
pub struct IconAssetSource;

impl AssetSource for IconAssetSource {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let clean = path.trim_start_matches('/');
        for icon in ALL_ICONS {
            let direct = format!("icons/{}.svg", icon.name());
            let raw_name = format!("{}.svg", icon.name());
            if clean == direct || clean == raw_name || clean == icon.name() {
                return Ok(Some(Cow::Borrowed(icon.svg_data().as_bytes())));
            }
        }
        Ok(None)
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(ALL_ICONS
            .iter()
            .map(|icon| icon.asset_path())
            .collect())
    }
}

// ============================================================================
// Reusable Component Helpers (Returning Div)
// ============================================================================

/// Generic vector icon renderer returning a GPUI `Div`.
/// Uses `svg().path(...)` bound to the in-memory icon asset source.
pub fn render_icon(icon: DiscordIcon, size: Pixels, color: impl Into<Hsla>) -> Div {
    let color: Hsla = color.into();
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(size)
        .flex_shrink_0()
        .child(
            svg()
                .path(icon.asset_path())
                .size(size)
                .text_color(color),
        )
}

/// Helper for rendering Bot tag / robot icon
pub fn icon_bot(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Bot, size, color)
}

/// Helper for rendering User profile icon
pub fn icon_user(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::User, size, color)
}

/// Helper for rendering Eye (reveal secret)
pub fn icon_eye(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Eye, size, color)
}

/// Helper for rendering EyeOff (hide secret)
pub fn icon_eye_off(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::EyeOff, size, color)
}

/// Helper for rendering Key (auth token)
pub fn icon_key(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Key, size, color)
}

/// Helper for rendering Paste (clipboard)
pub fn icon_paste(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Paste, size, color)
}

/// Helper for rendering Lock (secure channel / token security)
pub fn icon_lock(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Lock, size, color)
}

/// Helper for rendering Hash (# text channel)
pub fn icon_hash(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Hash, size, color)
}

/// Helper for rendering Speaker (voice channel)
pub fn icon_speaker(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Speaker, size, color)
}

/// Helper for rendering Bell (notifications)
pub fn icon_bell(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Bell, size, color)
}

/// Helper for rendering Gear (settings)
pub fn icon_gear(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Gear, size, color)
}

/// Helper for rendering Microphone / mute
pub fn icon_mic(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Mic, size, color)
}

/// Helper for rendering Headphones / deafen
pub fn icon_headphones(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Headphones, size, color)
}

/// Helper for rendering Send (submit chat message)
pub fn icon_send(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Send, size, color)
}

/// Helper for rendering Close / Dismiss ✕
pub fn icon_close(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Close, size, color)
}

/// Helper for rendering Checkmark ✓
pub fn icon_check(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Check, size, color)
}

/// Helper for rendering Warning alert
pub fn icon_warning(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Warning, size, color)
}

/// Helper for rendering Info icon
pub fn icon_info(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Info, size, color)
}

/// Helper for rendering Megaphone (announcement channel)
pub fn icon_megaphone(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Megaphone, size, color)
}

/// Helper for rendering Members list icon
pub fn icon_members(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Members, size, color)
}

/// Helper for rendering Search magnifying glass
pub fn icon_search(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Search, size, color)
}

/// Helper for rendering Inbox notifications tray
pub fn icon_inbox(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Inbox, size, color)
}

/// Helper for rendering Pinned messages
pub fn icon_pin(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Pin, size, color)
}

/// Helper for rendering Plus (add server / upload)
pub fn icon_plus(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Plus, size, color)
}

/// Helper for rendering Gift (Nitro gift)
pub fn icon_gift(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Gift, size, color)
}

/// Helper for rendering GIF picker
pub fn icon_gif(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Gif, size, color)
}

/// Helper for rendering Sticker picker
pub fn icon_sticker(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Sticker, size, color)
}

/// Helper for rendering Emoji picker
pub fn icon_emoji(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Emoji, size, color)
}

/// Helper for rendering Compass (discover public servers)
pub fn icon_compass(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Compass, size, color)
}

/// Helper for rendering Chat (Direct Message / DMs)
pub fn icon_chat(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Chat, size, color)
}

/// Helper for rendering Friends (Friends Hub)
pub fn icon_friends(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Friends, size, color)
}

/// Helper for rendering Folder (Server category / folder)
pub fn icon_folder(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Folder, size, color)
}

/// Helper for rendering Sparkles / Nitro Boost
pub fn icon_sparkles(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Sparkles, size, color)
}

/// Helper for rendering Shield (Security / Family Center / Spam protection)
pub fn icon_shield(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Shield, size, color)
}

/// Helper for rendering Gamepad (Activity / Games)
pub fn icon_gamepad(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Gamepad, size, color)
}

/// Helper for rendering Music / Spotify
pub fn icon_music(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Music, size, color)
}

/// Helper for rendering Phone (Voice call start)
pub fn icon_phone(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Phone, size, color)
}

/// Helper for rendering Phone Hangup (End voice call)
pub fn icon_phone_hangup(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::PhoneHangup, size, color)
}

/// Helper for rendering Screen Share
pub fn icon_screen_share(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::ScreenShare, size, color)
}

/// Helper for rendering Muted Microphone
pub fn icon_mic_off(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::MicOff, size, color)
}

/// Helper for rendering Deafened Headphones
pub fn icon_headphones_off(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::HeadphonesOff, size, color)
}

/// Helper for rendering Help / Question mark
pub fn icon_help(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Help, size, color)
}

/// Helper for rendering Smile / Reaction smiley
pub fn icon_smile(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Smile, size, color)
}

/// Helper for rendering Wifi / Network connection
pub fn icon_wifi(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Wifi, size, color)
}

/// Helper for rendering GitHub brand
pub fn icon_github(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Github, size, color)
}

/// Helper for rendering Twitch brand
pub fn icon_twitch(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Twitch, size, color)
}

/// Helper for rendering YouTube brand
pub fn icon_youtube(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Youtube, size, color)
}

/// Helper for rendering Link / chain connection
pub fn icon_link(size: Pixels, color: impl Into<Hsla>) -> Div {
    render_icon(DiscordIcon::Link, size, color)
}


