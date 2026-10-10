//! Discord Voice Gateway v8 Signaling Implementation
//!
//! Handles WebSocket connection to `wss://{endpoint}/?v=8`, session lifecycle,
//! heartbeats with `seq_ack`, protocol selection, encryption mode negotiation,
//! speaking indicators, session resumption, and DAVE (E2EE) signaling.

use anyhow::{bail, Context, Result};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, watch, Mutex, RwLock};
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::protocol::{Message as WsMessage, WebSocketConfig};
use crate::protocol::voice_dave::{
    parse_server_binary_message, DaveAnnounceCommitTransitionPayload, DaveExecuteTransitionPayload,
    DaveOpcode, DavePrepareEpochPayload, DavePrepareTransitionPayload, VoiceDaveSession,
    SUPPORTED_DAVE_PROTOCOL_VERSION,
};

/// Preferred voice encryption mode for Discord voice connections.
pub const PREFERRED_ENCRYPTION_MODE: &str = "aead_aes256_gcm_rtpsize";

/// Fallback voice encryption mode.
pub const FALLBACK_ENCRYPTION_MODE: &str = "aead_xchacha20_poly1305_rtpsize";

/// Default Voice Gateway version.
pub const VOICE_GATEWAY_VERSION: u32 = 8;

/// Default DAVE protocol version requested in Identify payload (Version 1 per DAVE spec).
pub const DEFAULT_MAX_DAVE_PROTOCOL_VERSION: u32 = SUPPORTED_DAVE_PROTOCOL_VERSION as u32;

// ============================================================================
// DAVE PROTOCOL ARCHITECTURAL DOCUMENTATION
// ============================================================================
// Discord Audio Video End-to-end Encryption (DAVE) Protocol Overview:
//
// In Voice Gateway v8, Discord introduced the DAVE protocol for end-to-end
// encryption of voice and video streams across participants, built on top of
// MLS (Messaging Layer Security, RFC 9420).
//
// In the Opcode 0 Identify payload, clients specify `max_dave_protocol_version`.
//
// Implications for End-to-End Call Functionality:
// 1. DAVE Enforcement (Post-March 1, 2026):
//    - Discord strictly enforces DAVE E2EE across direct calls, group DMs, and voice channels.
//    - Clients must support DAVE Protocol Version 1 (RFC 9420 MLS) to participate.
//    - We send `max_dave_protocol_version: 1` by default in Opcode 0 Identify.
// 2. Full DAVE Protocol State Machine:
//    - Opcodes 21-31 manage MLS group lifecycle (KeyPackages, Proposals, Commits, Welcomes).
//    - Server-to-client binary framing (u16 seq + u8 op + payload) is decoded and dispatched.
//    - Client-to-server binary frames (u8 op + payload) are generated for KeyPackages and CommitWelcomes.
//    - Session Description (Opcode 4) provides negotiated `dave_protocol_version` and secret key.
// 3. Media Cryptor Contract:
//    - Opus frames are encrypted with current MLS epoch key and decrypted per remote sender.
//    - Automatic passthrough fallback is preserved during handshakes, transitions, or version 0.

// ============================================================================
// CONTRACT WITH VoiceUDP AGENT (Primitive Types Only)
// ============================================================================

/// Connection parameters received in Opcode 2 Ready from Discord Voice Gateway.
/// Contract type exposed for integration with VoiceUDP module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceReady {
    /// SSRC (Synchronization Source) identifier assigned to this client.
    pub ssrc: u32,
    /// Voice server IP address for UDP connection.
    pub ip: String,
    /// Voice server UDP port.
    pub port: u16,
    /// List of encryption modes supported by the server.
    pub modes: Vec<String>,
}

impl VoiceReady {
    /// Selects the best encryption mode supported by the Discord Voice Gateway.
    /// Prefers `aead_aes256_gcm_rtpsize`, with fallback to `aead_xchacha20_poly1305_rtpsize`.
    pub fn select_best_mode(&self) -> &str {
        if self.modes.iter().any(|m| m == PREFERRED_ENCRYPTION_MODE) {
            PREFERRED_ENCRYPTION_MODE
        } else if self.modes.iter().any(|m| m == FALLBACK_ENCRYPTION_MODE) {
            FALLBACK_ENCRYPTION_MODE
        } else {
            self.modes.first().map(|s| s.as_str()).unwrap_or(PREFERRED_ENCRYPTION_MODE)
        }
    }
}

/// Session description containing negotiated encryption parameters and secret key.
/// Contract type exposed for integration with VoiceUDP module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDescription {
    /// Selected encryption mode (e.g. "aead_aes256_gcm_rtpsize").
    pub mode: String,
    /// 32-byte secret key for RTP encryption/decryption.
    pub secret_key: Vec<u8>,
    /// Negotiated DAVE protocol version (e.g. Some(1) or None).
    #[serde(default)]
    pub dave_protocol_version: Option<u32>,
}

/// Voice server connection metadata received from main Discord Gateway (VOICE_SERVER_UPDATE).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VoiceServerData {
    pub token: String,
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub endpoint: Option<String>,
}

// ============================================================================
// VOICE GATEWAY OPCODES & PAYLOADS
// ============================================================================

/// Voice Gateway Opcodes according to official Discord documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum VoiceOpcode {
    Identify = 0,
    SelectProtocol = 1,
    Ready = 2,
    Heartbeat = 3,
    SessionDescription = 4,
    Speaking = 5,
    HeartbeatAck = 6,
    Resume = 7,
    Hello = 8,
    Resumed = 9,
}

impl VoiceOpcode {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Identify),
            1 => Some(Self::SelectProtocol),
            2 => Some(Self::Ready),
            3 => Some(Self::Heartbeat),
            4 => Some(Self::SessionDescription),
            5 => Some(Self::Speaking),
            6 => Some(Self::HeartbeatAck),
            7 => Some(Self::Resume),
            8 => Some(Self::Hello),
            9 => Some(Self::Resumed),
            _ => None,
        }
    }
}

/// Generic container for Discord Voice Gateway messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceGatewayPayload<T = serde_json::Value> {
    pub op: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "s")]
    pub seq: Option<i64>,
}

/// Opcode 0: Identify Payload sent from client to voice gateway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentifyPayload {
    pub server_id: String,
    pub user_id: String,
    pub session_id: String,
    pub token: String,
    #[serde(default)]
    pub max_dave_protocol_version: u32,
}

/// Opcode 1: Select Protocol Payload sent from client to voice gateway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectProtocolPayload {
    pub protocol: String,
    pub data: SelectProtocolData,
}

/// Network parameters for Opcode 1 Select Protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectProtocolData {
    pub address: String,
    pub port: u16,
    pub mode: String,
}

/// Opcode 2: Ready Payload received from voice gateway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadyPayload {
    pub ssrc: u32,
    pub ip: String,
    pub port: u16,
    pub modes: Vec<String>,
    #[serde(default)]
    pub experiments: Vec<serde_json::Value>,
    #[serde(default)]
    pub streams: Vec<serde_json::Value>,
}

/// Opcode 3: Heartbeat Payload sent periodically by client.
/// In Voice Gateway v8, `seq_ack` is mandatory for buffered resume support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<u64>,
    pub seq_ack: i64,
}

/// Opcode 4: Session Description Payload received from voice gateway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDescriptionPayload {
    pub mode: String,
    pub secret_key: Vec<u8>,
    #[serde(default)]
    pub dave_protocol_version: Option<u32>,
    #[serde(default)]
    pub media_session_id: Option<String>,
}

/// Opcode 5: Speaking Payload sent by client or received from server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeakingPayload {
    pub speaking: u32,
    #[serde(default)]
    pub delay: u32,
    pub ssrc: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// Bitflags for Speaking status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeakingFlags;

impl SpeakingFlags {
    pub const NONE: u32 = 0;
    pub const MICROPHONE: u32 = 1 << 0;
    pub const SOUNDSHARE: u32 = 1 << 1;
    pub const PRIORITY: u32 = 1 << 2;
}

/// Opcode 7: Resume Payload sent by client to resume an interrupted session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumePayload {
    pub server_id: String,
    pub session_id: String,
    pub token: String,
    pub seq_ack: i64,
}

/// Opcode 8: Hello Payload received immediately upon WebSocket connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelloPayload {
    #[serde(default)]
    pub v: Option<u32>,
    pub heartbeat_interval: f64,
}

// ============================================================================
// CONFIGURATION & EVENTS
// ============================================================================

/// Configuration for connecting to the Voice Gateway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceGatewayConfig {
    pub endpoint: String,
    pub server_id: String,
    pub user_id: String,
    pub session_id: String,
    pub token: String,
    pub max_dave_protocol_version: Option<u32>,
}

/// Decision determining whether and how the Voice Gateway should reconnect after closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceCloseDecision {
    /// Can resume the existing session (Opcode 7 Resume with buffered `seq_ack`).
    Resume,
    /// Session is no longer valid or timed out; must reconnect with a fresh Opcode 0 Identify.
    NewSession,
    /// Fatal closure (e.g. invalid auth, kicked, channel deleted, DAVE required); must stop and not reconnect.
    Fatal,
}

impl VoiceCloseDecision {
    /// Determines the reconnection strategy from a WebSocket close code per Discord documentation.
    /// Reference: https://discord.com/developers/docs/topics/opcodes-and-status-codes#voice
    pub fn from_close_code(code: u16) -> Self {
        match code {
            // Resumable codes per Discord docs ("Try resuming" / network interruptions)
            4015 => Self::Resume,        // Voice server crashed
            1001 | 1006 => Self::Resume, // Going away / Abnormal closure

            // Session invalidation requiring a fresh Identify (Opcode 0)
            4003 => Self::NewSession,    // Not authenticated (sent payload prior to identifying)
            4005 => Self::NewSession,    // Already authenticated (sent more than one identify)
            4006 => Self::NewSession,    // Session no longer valid
            4009 => Self::NewSession,    // Session timeout

            // Fatal codes (must not reconnect automatically)
            4001 => Self::Fatal,         // Unknown opcode
            4002 => Self::Fatal,         // Failed to decode payload
            4004 => Self::Fatal,         // Authentication failed (token is invalid)
            4011 => Self::Fatal,         // Server not found
            4012 => Self::Fatal,         // Unknown protocol
            4014 => Self::Fatal,         // Disconnected (channel deleted or user kicked - "Should not reconnect")
            4016 => Self::Fatal,         // Unknown encryption mode
            4017 => Self::Fatal,         // E2EE/DAVE protocol required
            4020 => Self::Fatal,         // Bad request
            4022 => Self::Fatal,         // Call terminated
            1000 => Self::Fatal,         // Normal closure

            // Any unknown or unhandled close codes default to Fatal to avoid infinite reconnection loops
            _ => Self::Fatal,
        }
    }

    /// Whether this close decision permits attempting a reconnection.
    pub fn should_reconnect(&self) -> bool {
        matches!(self, Self::Resume | Self::NewSession)
    }

    /// Whether this close decision allows session resumption (Opcode 7 Resume).
    pub fn can_resume(&self) -> bool {
        matches!(self, Self::Resume)
    }
}

/// Returns the official human-readable description for Discord Voice Gateway close codes.
/// Reference: https://discord.com/developers/docs/topics/opcodes-and-status-codes#voice
pub fn describe_voice_close_code(code: u16) -> &'static str {
    match code {
        1000 => "Normal closure",
        1001 => "Going away",
        1006 => "Abnormal closure",
        4001 => "Unknown opcode: you sent an invalid opcode",
        4002 => "Failed to decode: invalid payload sent",
        4003 => "Not authenticated: sent payload prior to identifying",
        4004 => "Authentication failed: token is invalid",
        4005 => "Already authenticated: sent multiple identify payloads",
        4006 => "Session no longer valid: session expired or invalidated",
        4009 => "Session timeout: session timed out, reconnect and re-identify",
        4011 => "Server not found: voice server not found at endpoint",
        4012 => "Unknown protocol: unrecognized protocol",
        4014 => "Disconnected: channel deleted or you were kicked from voice",
        4015 => "Voice server crashed: server crashed, resuming session",
        4016 => "Unknown encryption mode: unrecognized encryption mode",
        4017 => "E2EE/DAVE protocol required: this channel requires DAVE end-to-end encryption",
        4020 => "Bad request",
        4022 => "Call terminated",
        _ => "Unknown voice close code",
    }
}

/// Real-time events emitted by the Voice Gateway client.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceGatewayEvent {
    Connected,
    Ready(VoiceReady),
    SessionDescription(SessionDescription),
    Speaking(SpeakingPayload),
    HeartbeatAck { latency_ms: Option<u64> },
    Resumed,
    DavePrepareTransition(DavePrepareTransitionPayload),
    DaveExecuteTransition(DaveExecuteTransitionPayload),
    DavePrepareEpoch(DavePrepareEpochPayload),
    DaveAnnounceCommit(DaveAnnounceCommitTransitionPayload),
    DaveBinaryMessage(Vec<u8>),
    Disconnected {
        code: Option<u16>,
        reason: String,
        decision: VoiceCloseDecision,
    },
}

// ============================================================================
// PURE STATE MACHINE (Network-free, Fully Unit-Testable)
// ============================================================================

/// Connection states of the Discord Voice Gateway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceConnectionState {
    Disconnected,
    WaitingForHello,
    Identifying,
    Ready(VoiceReady),
    SelectingProtocol,
    Connected(SessionDescription),
    Resuming,
    Failed(String),
}

/// Actions emitted by the state machine to be executed by the transport layer.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceGatewayAction {
    SendMessage(String),
    SendBinaryMessage(Vec<u8>),
    StartHeartbeat { interval_ms: u64 },
    ReadyReceived(VoiceReady),
    SessionDescriptionReceived(SessionDescription),
    SpeakingUpdate(SpeakingPayload),
    HeartbeatAckReceived { timestamp: Option<u64> },
    Resumed,
    DavePrepareTransition(DavePrepareTransitionPayload),
    DaveExecuteTransition(DaveExecuteTransitionPayload),
    DavePrepareEpoch(DavePrepareEpochPayload),
    DaveAnnounceCommit(DaveAnnounceCommitTransitionPayload),
    CloseConnection { reason: String, can_resume: bool },
}

/// Pure state machine governing the Voice Gateway v8 lifecycle without I/O.
#[derive(Debug)]
pub struct VoiceGatewayStateMachine {
    state: VoiceConnectionState,
    config: VoiceGatewayConfig,
    last_seq: i64,
    heartbeat_interval_ms: Option<u64>,
    ready_data: Option<VoiceReady>,
    session_description: Option<SessionDescription>,
    pub dave_session: VoiceDaveSession,
}

impl VoiceGatewayStateMachine {
    /// Creates a new state machine in `Disconnected` state.
    pub fn new(config: VoiceGatewayConfig) -> Self {
        let user_id = config.user_id.parse::<u64>().unwrap_or(0);
        let channel_id = config.server_id.parse::<u64>().unwrap_or(0);
        Self {
            state: VoiceConnectionState::Disconnected,
            config,
            last_seq: -1,
            heartbeat_interval_ms: None,
            ready_data: None,
            session_description: None,
            dave_session: VoiceDaveSession::new(user_id, channel_id),
        }
    }

    /// Current connection lifecycle state.
    pub fn current_state(&self) -> &VoiceConnectionState {
        &self.state
    }

    /// Last sequence number acknowledged from the server.
    pub fn last_seq(&self) -> i64 {
        self.last_seq
    }

    /// Sets the last sequence number (useful for test setup and resume testing).
    pub fn set_last_seq(&mut self, seq: i64) {
        self.last_seq = seq;
    }

    /// Sets the current connection lifecycle state.
    pub fn set_state(&mut self, state: VoiceConnectionState) {
        self.state = state;
    }

    /// Sets cached session description.
    pub fn set_session_description(&mut self, desc: SessionDescription) {
        self.session_description = Some(desc);
    }

    /// Cached ready parameters if available.
    pub fn ready_data(&self) -> Option<&VoiceReady> {
        self.ready_data.as_ref()
    }

    /// Cached session description parameters if available.
    pub fn session_description(&self) -> Option<&SessionDescription> {
        self.session_description.as_ref()
    }

    /// Updates configuration (e.g. on new token or session).
    pub fn update_config(&mut self, config: VoiceGatewayConfig) {
        self.config = config;
    }

    /// Reference to inner DAVE session state and media cryptor.
    pub fn dave_session(&self) -> &VoiceDaveSession {
        &self.dave_session
    }

    /// Mutable reference to inner DAVE session state and media cryptor.
    pub fn dave_session_mut(&mut self) -> &mut VoiceDaveSession {
        &mut self.dave_session
    }

    /// Transitions to `WaitingForHello` when the WebSocket connection begins.
    pub fn start(&mut self) -> Result<()> {
        self.state = VoiceConnectionState::WaitingForHello;
        Ok(())
    }

    /// Generates Opcode 3 Heartbeat payload containing `seq_ack`.
    pub fn generate_heartbeat(&self, timestamp: Option<u64>) -> VoiceGatewayPayload<HeartbeatPayload> {
        VoiceGatewayPayload {
            op: VoiceOpcode::Heartbeat as u8,
            d: Some(HeartbeatPayload {
                t: timestamp,
                seq_ack: self.last_seq,
            }),
            seq: None,
        }
    }

    /// Generates Opcode 0 Identify payload.
    pub fn generate_identify(&self) -> VoiceGatewayPayload<IdentifyPayload> {
        VoiceGatewayPayload {
            op: VoiceOpcode::Identify as u8,
            d: Some(IdentifyPayload {
                server_id: self.config.server_id.clone(),
                user_id: self.config.user_id.clone(),
                session_id: self.config.session_id.clone(),
                token: self.config.token.clone(),
                max_dave_protocol_version: self
                    .config
                    .max_dave_protocol_version
                    .unwrap_or(DEFAULT_MAX_DAVE_PROTOCOL_VERSION),
            }),
            seq: None,
        }
    }

    /// Generates Opcode 7 Resume payload.
    pub fn generate_resume(&self) -> VoiceGatewayPayload<ResumePayload> {
        VoiceGatewayPayload {
            op: VoiceOpcode::Resume as u8,
            d: Some(ResumePayload {
                server_id: self.config.server_id.clone(),
                session_id: self.config.session_id.clone(),
                token: self.config.token.clone(),
                seq_ack: self.last_seq,
            }),
            seq: None,
        }
    }

    /// Generates Opcode 1 Select Protocol payload.
    pub fn generate_select_protocol(
        &self,
        external_ip: &str,
        external_port: u16,
        mode: &str,
    ) -> VoiceGatewayPayload<SelectProtocolPayload> {
        VoiceGatewayPayload {
            op: VoiceOpcode::SelectProtocol as u8,
            d: Some(SelectProtocolPayload {
                protocol: "udp".to_string(),
                data: SelectProtocolData {
                    address: external_ip.to_string(),
                    port: external_port,
                    mode: mode.to_string(),
                },
            }),
            seq: None,
        }
    }

    /// Generates Opcode 5 Speaking payload.
    pub fn generate_speaking(
        &self,
        speaking: bool,
        delay: u32,
        ssrc: u32,
    ) -> VoiceGatewayPayload<SpeakingPayload> {
        VoiceGatewayPayload {
            op: VoiceOpcode::Speaking as u8,
            d: Some(SpeakingPayload {
                speaking: if speaking { SpeakingFlags::MICROPHONE } else { SpeakingFlags::NONE },
                delay,
                ssrc,
                user_id: None,
            }),
            seq: None,
        }
    }

    /// Requests protocol selection with discovered IP/port and encryption mode.
    pub fn select_protocol(
        &mut self,
        external_ip: &str,
        external_port: u16,
        mode: &str,
    ) -> Result<VoiceGatewayAction> {
        let payload = self.generate_select_protocol(external_ip, external_port, mode);
        let json_str = serde_json::to_string(&payload)?;
        self.state = VoiceConnectionState::SelectingProtocol;
        Ok(VoiceGatewayAction::SendMessage(json_str))
    }

    /// Sets client speaking state (Opcode 5).
    pub fn set_speaking(&mut self, speaking: bool, delay: u32, ssrc: u32) -> Result<VoiceGatewayAction> {
        let payload = self.generate_speaking(speaking, delay, ssrc);
        let json_str = serde_json::to_string(&payload)?;
        Ok(VoiceGatewayAction::SendMessage(json_str))
    }

    /// Triggers an Opcode 3 Heartbeat action with current `seq_ack`.
    pub fn trigger_heartbeat(&mut self, timestamp: Option<u64>) -> Result<VoiceGatewayAction> {
        let payload = self.generate_heartbeat(timestamp);
        let json_str = serde_json::to_string(&payload)?;
        Ok(VoiceGatewayAction::SendMessage(json_str))
    }

    /// Processes an incoming JSON text message from the voice gateway WebSocket.
    pub fn handle_incoming_text(&mut self, text: &str) -> Result<Vec<VoiceGatewayAction>> {
        let payload: VoiceGatewayPayload<serde_json::Value> = serde_json::from_str(text)
            .with_context(|| format!("Failed to parse Voice Gateway payload: {}", text))?;

        if let Some(s) = payload.seq {
            self.last_seq = s;
        }

        let mut actions = Vec::new();

        match VoiceOpcode::from_u8(payload.op) {
            Some(VoiceOpcode::Hello) => {
                let data = payload.d.context("Missing Hello data payload")?;
                let hello: HelloPayload = serde_json::from_value(data)
                    .context("Failed to deserialize Hello payload")?;

                let interval_ms = hello.heartbeat_interval.round() as u64;
                self.heartbeat_interval_ms = Some(interval_ms);
                actions.push(VoiceGatewayAction::StartHeartbeat { interval_ms });

                // If we are in Resuming state, send Resume (Op 7). Otherwise send Identify (Op 0).
                if self.state == VoiceConnectionState::Resuming {
                    let resume = self.generate_resume();
                    actions.push(VoiceGatewayAction::SendMessage(serde_json::to_string(&resume)?));
                } else {
                    self.state = VoiceConnectionState::Identifying;
                    let identify = self.generate_identify();
                    actions.push(VoiceGatewayAction::SendMessage(serde_json::to_string(&identify)?));
                }
            }
            Some(VoiceOpcode::Ready) => {
                let data = payload.d.context("Missing Ready data payload")?;
                let ready: ReadyPayload = serde_json::from_value(data)
                    .context("Failed to deserialize Ready payload")?;

                let voice_ready = VoiceReady {
                    ssrc: ready.ssrc,
                    ip: ready.ip,
                    port: ready.port,
                    modes: ready.modes,
                };
                self.ready_data = Some(voice_ready.clone());
                self.state = VoiceConnectionState::Ready(voice_ready.clone());
                actions.push(VoiceGatewayAction::ReadyReceived(voice_ready));
            }
            Some(VoiceOpcode::SessionDescription) => {
                let data = payload.d.context("Missing SessionDescription data payload")?;
                let session: SessionDescriptionPayload = serde_json::from_value(data)
                    .context("Failed to deserialize SessionDescription payload")?;

                let desc = SessionDescription {
                    mode: session.mode,
                    secret_key: session.secret_key,
                    dave_protocol_version: session.dave_protocol_version,
                };
                if let Some(v) = session.dave_protocol_version {
                    if let Err(e) = self.dave_session.init_session(v as u16) {
                        log::warn!("Failed to initialize DAVE session for version {}: {}", v, e);
                    }
                }
                self.session_description = Some(desc.clone());
                self.state = VoiceConnectionState::Connected(desc.clone());
                actions.push(VoiceGatewayAction::SessionDescriptionReceived(desc));
            }
            Some(VoiceOpcode::Speaking) => {
                let data = payload.d.context("Missing Speaking data payload")?;
                let speaking: SpeakingPayload = serde_json::from_value(data)
                    .context("Failed to deserialize Speaking payload")?;
                if let Some(ref uid_str) = speaking.user_id {
                    if let Ok(uid) = uid_str.parse::<u64>() {
                        self.dave_session.register_ssrc_user_mapping(speaking.ssrc, uid);
                    }
                }
                actions.push(VoiceGatewayAction::SpeakingUpdate(speaking));
            }
            Some(VoiceOpcode::HeartbeatAck) => {
                let timestamp = payload.d.and_then(|v| v.as_u64());
                actions.push(VoiceGatewayAction::HeartbeatAckReceived { timestamp });
            }
            Some(VoiceOpcode::Resumed) => {
                if let Some(desc) = self.session_description.clone() {
                    self.state = VoiceConnectionState::Connected(desc);
                } else if let Some(ready) = self.ready_data.clone() {
                    self.state = VoiceConnectionState::Ready(ready);
                }
                actions.push(VoiceGatewayAction::Resumed);
            }
            _ => {
                if let Some(dave_op) = DaveOpcode::from_u8(payload.op) {
                    match dave_op {
                        DaveOpcode::PrepareTransition => {
                            if let Some(data) = payload.d {
                                let prep: DavePrepareTransitionPayload = serde_json::from_value(data)
                                    .context("Failed to deserialize DavePrepareTransition payload")?;
                                let ready = self.dave_session.handle_prepare_transition(prep.transition_id, prep.protocol_version);
                                let ready_payload = VoiceGatewayPayload {
                                    op: DaveOpcode::TransitionReady as u8,
                                    d: Some(ready),
                                    seq: None,
                                };
                                actions.push(VoiceGatewayAction::SendMessage(serde_json::to_string(&ready_payload)?));
                            }
                        }
                        DaveOpcode::ExecuteTransition => {
                            if let Some(data) = payload.d {
                                let exec: DaveExecuteTransitionPayload = serde_json::from_value(data)
                                    .context("Failed to deserialize DaveExecuteTransition payload")?;
                                self.dave_session.handle_execute_transition(exec.transition_id)?;
                            }
                        }
                        DaveOpcode::PrepareEpoch => {
                            if let Some(data) = payload.d {
                                let epoch: DavePrepareEpochPayload = serde_json::from_value(data)
                                    .context("Failed to deserialize DavePrepareEpoch payload")?;
                                self.dave_session.handle_prepare_epoch(epoch.epoch, epoch.protocol_version);
                            }
                        }
                        DaveOpcode::MlsAnnounceCommitTransition => {
                            if let Some(data) = payload.d {
                                let announce: DaveAnnounceCommitTransitionPayload = serde_json::from_value(data)
                                    .context("Failed to deserialize DaveAnnounceCommitTransition payload")?;
                                let commit_bytes = announce.commit.as_ref().map(|s| s.as_bytes()).unwrap_or(&[]);
                                if let Some(invalid_payload) = self.dave_session.handle_announce_commit(announce.transition_id, commit_bytes)? {
                                    let invalid_msg = VoiceGatewayPayload {
                                        op: DaveOpcode::MlsInvalidCommitWelcome as u8,
                                        d: Some(invalid_payload),
                                        seq: None,
                                    };
                                    actions.push(VoiceGatewayAction::SendMessage(serde_json::to_string(&invalid_msg)?));
                                }
                            }
                        }
                        _ => {
                            log::debug!("Unhandled DAVE JSON opcode: {:?}", dave_op);
                        }
                    }
                }
            }
        }

        Ok(actions)
    }

    /// Processes an incoming binary message from the voice gateway WebSocket.
    ///
    /// Dispatches server-to-client DAVE opcodes:
    /// - Opcode 25 (MLS External Sender Package) -> responds with Opcode 26 (MLS Key Package).
    /// - Opcode 27 (MLS Proposals) -> responds with Opcode 28 (MLS Commit Welcome) if commit is generated.
    /// - Opcode 30 (MLS Welcome) -> responds with Opcode 31 (MLS Invalid Commit Welcome) if welcome fails.
    pub fn handle_incoming_binary(&mut self, data: &[u8]) -> Result<Vec<VoiceGatewayAction>> {
        let (seq, opcode, payload) = parse_server_binary_message(data)?;
        self.last_seq = seq as i64;
        let mut actions = Vec::new();

        match opcode {
            DaveOpcode::MlsExternalSender => {
                let key_package_frame = self.dave_session.handle_external_sender(payload)?;
                actions.push(VoiceGatewayAction::SendBinaryMessage(key_package_frame));
            }
            DaveOpcode::MlsProposals => {
                if payload.is_empty() {
                    bail!("Opcode 27 Proposals binary frame has empty payload");
                }
                let op_type = payload[0];
                let proposals_bytes = &payload[1..];
                if let Some(commit_welcome_frame) = self.dave_session.handle_proposals(op_type, proposals_bytes, None)? {
                    actions.push(VoiceGatewayAction::SendBinaryMessage(commit_welcome_frame));
                }
            }
            DaveOpcode::MlsWelcome => {
                if payload.len() < 2 {
                    bail!("Opcode 30 Welcome binary frame is too short: {} bytes", payload.len());
                }
                let transition_id = u16::from_be_bytes([payload[0], payload[1]]);
                let welcome_bytes = &payload[2..];
                if let Some(invalid_payload) = self.dave_session.handle_welcome(transition_id, welcome_bytes)? {
                    let invalid_msg = VoiceGatewayPayload {
                        op: DaveOpcode::MlsInvalidCommitWelcome as u8,
                        d: Some(invalid_payload),
                        seq: None,
                    };
                    actions.push(VoiceGatewayAction::SendMessage(serde_json::to_string(&invalid_msg)?));
                }
            }
            _ => {
                log::warn!("Unhandled binary voice opcode: {:?}", opcode);
            }
        }

        Ok(actions)
    }

    /// Evaluates a Discord Voice Gateway close code and updates the state machine accordingly.
    ///
    /// - `Resume`: preserves sequence counter, transitions to `Resuming` (if session is valid).
    /// - `NewSession`: resets sequence counter to -1, transitions to `WaitingForHello` for fresh Identify.
    /// - `Fatal`: transitions to `Failed(reason)`, emits `CloseConnection` with `can_resume: false`.
    pub fn handle_close_code(
        &mut self,
        code: u16,
        raw_reason: &str,
    ) -> (VoiceCloseDecision, Option<VoiceGatewayAction>) {
        let decision = VoiceCloseDecision::from_close_code(code);
        let desc = describe_voice_close_code(code);
        let full_reason = if raw_reason.trim().is_empty() {
            desc.to_string()
        } else {
            format!("{}: {}", desc, raw_reason.trim())
        };

        match decision {
            VoiceCloseDecision::Resume => {
                if self.last_seq >= 0 && !self.config.session_id.is_empty() {
                    self.state = VoiceConnectionState::Resuming;
                } else {
                    self.state = VoiceConnectionState::WaitingForHello;
                }
                (decision, None)
            }
            VoiceCloseDecision::NewSession => {
                // Must not resume; clear sequence counter to force a fresh Opcode 0 Identify
                self.last_seq = -1;
                self.state = VoiceConnectionState::WaitingForHello;
                (decision, None)
            }
            VoiceCloseDecision::Fatal => {
                self.state = VoiceConnectionState::Failed(full_reason.clone());
                (
                    decision,
                    Some(VoiceGatewayAction::CloseConnection {
                        reason: full_reason,
                        can_resume: false,
                    }),
                )
            }
        }
    }

    /// Handles disconnection with indication of whether session is resumable.
    pub fn handle_disconnect(&mut self, can_resume: bool) -> Option<VoiceGatewayAction> {
        if can_resume && self.last_seq >= 0 && !self.config.session_id.is_empty() {
            self.state = VoiceConnectionState::Resuming;
            None
        } else {
            self.state = VoiceConnectionState::Disconnected;
            Some(VoiceGatewayAction::CloseConnection {
                reason: "Disconnected".to_string(),
                can_resume,
            })
        }
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Formats the endpoint returned by Discord to the standard Voice Gateway v8 WebSocket URL.
/// Strips unnecessary protocols, ports (`:443`, `:80`), and appends `/?v=8`.
pub fn format_voice_gateway_url(endpoint: &str) -> String {
    let clean = endpoint.trim();
    let clean = clean.strip_prefix("wss://").unwrap_or(clean);
    let clean = clean.strip_prefix("ws://").unwrap_or(clean);
    let host = if let Some(stripped) = clean.strip_suffix(":443") {
        stripped
    } else if let Some(stripped) = clean.strip_suffix(":80") {
        stripped
    } else {
        clean
    };
    format!("wss://{}/?v={}", host, VOICE_GATEWAY_VERSION)
}

// ============================================================================
// ASYNC LIVE CLIENT (tokio-tungstenite WebSocket client)
// ============================================================================

/// Live client managing the Voice Gateway v8 WebSocket connection.
pub struct VoiceGateway {
    config: VoiceGatewayConfig,
    state_machine: Arc<RwLock<VoiceGatewayStateMachine>>,
    ready_sender: watch::Sender<Option<VoiceReady>>,
    ready_receiver: watch::Receiver<Option<VoiceReady>>,
    session_sender: watch::Sender<Option<SessionDescription>>,
    session_receiver: watch::Receiver<Option<SessionDescription>>,
    event_sender: broadcast::Sender<VoiceGatewayEvent>,
    outbound_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<WsMessage>>>>,
    connection_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    is_connected: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
}

impl VoiceGateway {
    /// Creates a new Voice Gateway client.
    pub fn new(config: VoiceGatewayConfig) -> Self {
        let (ready_sender, ready_receiver) = watch::channel(None);
        let (session_sender, session_receiver) = watch::channel(None);
        let (event_sender, _) = broadcast::channel(64);
        let state_machine = Arc::new(RwLock::new(VoiceGatewayStateMachine::new(config.clone())));

        Self {
            config,
            state_machine,
            ready_sender,
            ready_receiver,
            session_sender,
            session_receiver,
            event_sender,
            outbound_tx: Arc::new(RwLock::new(None)),
            connection_task: Arc::new(Mutex::new(None)),
            is_connected: Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns whether the voice gateway is currently connected.
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }

    /// Subscribes to real-time events emitted by the voice gateway.
    pub fn subscribe_events(&self) -> broadcast::Receiver<VoiceGatewayEvent> {
        self.event_sender.subscribe()
    }

    /// Returns the cached `VoiceReady` parameters if received.
    pub async fn ready_data(&self) -> Option<VoiceReady> {
        self.ready_receiver.borrow().clone()
    }

    /// Returns the cached `SessionDescription` if received.
    pub async fn session_description(&self) -> Option<SessionDescription> {
        self.session_receiver.borrow().clone()
    }

    /// Connects to the Discord Voice Gateway WebSocket (`wss://{endpoint}/?v=8`).
    pub async fn connect(&self) -> Result<()> {
        self.stop_signal.store(false, Ordering::SeqCst);
        let mut task_lock = self.connection_task.lock().await;
        if let Some(task) = task_lock.take() {
            task.abort();
        }

        let state_machine_clone = self.state_machine.clone();
        let ready_sender_clone = self.ready_sender.clone();
        let session_sender_clone = self.session_sender.clone();
        let event_sender_clone = self.event_sender.clone();
        let outbound_tx_clone = self.outbound_tx.clone();
        let is_connected_clone = self.is_connected.clone();
        let stop_signal_clone = self.stop_signal.clone();
        let config_clone = self.config.clone();

        let handle = tokio::spawn(async move {
            run_voice_loop(
                config_clone,
                state_machine_clone,
                ready_sender_clone,
                session_sender_clone,
                event_sender_clone,
                outbound_tx_clone,
                is_connected_clone,
                stop_signal_clone,
            )
            .await;
        });

        *task_lock = Some(handle);
        Ok(())
    }

    /// Awaits receipt of `VoiceReady` with a timeout.
    pub async fn wait_for_ready(&self, timeout: Duration) -> Result<VoiceReady> {
        let mut rx = self.ready_receiver.clone();
        if let Some(ready) = rx.borrow().clone() {
            return Ok(ready);
        }

        tokio::time::timeout(timeout, async {
            while rx.changed().await.is_ok() {
                if let Some(ready) = rx.borrow().clone() {
                    return Ok(ready);
                }
            }
            bail!("Voice gateway ready channel closed");
        })
        .await
        .map_err(|_| anyhow::anyhow!("Timed out waiting for VoiceReady payload"))?
    }

    /// Selects the protocol (UDP external address and encryption mode).
    /// Called after IP discovery finishes resolving external address and port.
    pub async fn select_protocol(
        &self,
        external_ip: &str,
        external_port: u16,
        mode: &str,
    ) -> Result<SessionDescription> {
        let action = {
            let mut sm = self.state_machine.write().await;
            sm.select_protocol(external_ip, external_port, mode)?
        };

        if let VoiceGatewayAction::SendMessage(msg) = action {
            self.send_text(msg).await?;
        }

        self.wait_for_session_description(Duration::from_secs(10)).await
    }

    /// Awaits receipt of `SessionDescription` with a timeout.
    pub async fn wait_for_session_description(&self, timeout: Duration) -> Result<SessionDescription> {
        let mut rx = self.session_receiver.clone();
        if let Some(desc) = rx.borrow().clone() {
            return Ok(desc);
        }

        tokio::time::timeout(timeout, async {
            while rx.changed().await.is_ok() {
                if let Some(desc) = rx.borrow().clone() {
                    return Ok(desc);
                }
            }
            bail!("Voice gateway session description channel closed");
        })
        .await
        .map_err(|_| anyhow::anyhow!("Timed out waiting for SessionDescription payload"))?
    }

    /// Sets client speaking state (Opcode 5).
    pub async fn set_speaking(&self, speaking: bool, delay: u32, ssrc: u32) -> Result<()> {
        let action = {
            let mut sm = self.state_machine.write().await;
            sm.set_speaking(speaking, delay, ssrc)?
        };

        if let VoiceGatewayAction::SendMessage(msg) = action {
            self.send_text(msg).await?;
        }
        Ok(())
    }

    /// Returns a reference to the inner state machine lock.
    pub fn state_machine(&self) -> Arc<RwLock<VoiceGatewayStateMachine>> {
        self.state_machine.clone()
    }

    /// Encrypts an outgoing Opus frame using the active DAVE session (or passthrough).
    pub async fn encrypt_opus_frame(&self, frame: &[u8]) -> Result<Vec<u8>> {
        let mut sm = self.state_machine.write().await;
        sm.dave_session.encrypt_opus_frame(frame)
    }

    /// Decrypts an incoming Opus frame from a remote participant (or passthrough).
    pub async fn decrypt_opus_frame(&self, sender_user_id: u64, frame: &[u8]) -> Result<Vec<u8>> {
        let mut sm = self.state_machine.write().await;
        sm.dave_session.decrypt_opus_frame(sender_user_id, frame)
    }

    /// Decrypts an incoming Opus frame by sender SSRC (or passthrough).
    pub async fn decrypt_opus_frame_by_ssrc(&self, ssrc: u32, frame: &[u8]) -> Result<Vec<u8>> {
        let mut sm = self.state_machine.write().await;
        sm.dave_session.decrypt_opus_frame_by_ssrc(ssrc, frame)
    }

    /// Returns the currently negotiated DAVE protocol version.
    pub async fn dave_protocol_version(&self) -> u16 {
        let sm = self.state_machine.read().await;
        sm.dave_session.protocol_version()
    }

    /// Returns whether DAVE E2EE is active and ready to encrypt frames.
    pub async fn is_dave_ready(&self) -> bool {
        let sm = self.state_machine.read().await;
        sm.dave_session.is_ready()
    }

    /// Sends a raw text message over the WebSocket connection.
    pub async fn send_text(&self, text: String) -> Result<()> {
        let lock = self.outbound_tx.read().await;
        if let Some(ref tx) = *lock {
            tx.send(WsMessage::Text(text.into()))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send voice message: {}", e))?;
            Ok(())
        } else {
            bail!("Voice gateway is not connected");
        }
    }

    /// Sends a raw binary message over the WebSocket connection.
    pub async fn send_binary(&self, data: Vec<u8>) -> Result<()> {
        let lock = self.outbound_tx.read().await;
        if let Some(ref tx) = *lock {
            tx.send(WsMessage::Binary(data.into()))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send binary voice message: {}", e))?;
            Ok(())
        } else {
            bail!("Voice gateway is not connected");
        }
    }

    /// Gracefully disconnects from the Voice Gateway.
    pub async fn disconnect(&self) -> Result<()> {
        self.stop_signal.store(true, Ordering::SeqCst);
        self.is_connected.store(false, Ordering::SeqCst);

        let mut lock = self.connection_task.lock().await;
        if let Some(task) = lock.take() {
            task.abort();
        }

        let mut out_lock = self.outbound_tx.write().await;
        *out_lock = None;

        let _ = self.event_sender.send(VoiceGatewayEvent::Disconnected {
            code: Some(1000),
            reason: "Disconnected by client".to_string(),
            decision: VoiceCloseDecision::Fatal,
        });
        Ok(())
    }
}

// ============================================================================
// INTERNAL BACKGROUND LOOP
// ============================================================================

async fn run_voice_loop(
    config: VoiceGatewayConfig,
    state_machine: Arc<RwLock<VoiceGatewayStateMachine>>,
    ready_sender: watch::Sender<Option<VoiceReady>>,
    session_sender: watch::Sender<Option<SessionDescription>>,
    event_sender: broadcast::Sender<VoiceGatewayEvent>,
    outbound_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<WsMessage>>>>,
    is_connected: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
) {
    let ws_url = format_voice_gateway_url(&config.endpoint);
    let mut backoff_seconds = 1;

    while !stop_signal.load(Ordering::SeqCst) {
        log::info!("Connecting to Voice Gateway WebSocket: {}", ws_url);

        let mut ws_config = WebSocketConfig::default();
        ws_config.max_message_size = None;
        ws_config.max_frame_size = None;

        match connect_async_with_config(&ws_url, Some(ws_config), false).await {
            Ok((ws_stream, _)) => {
                log::info!("Connected to Discord Voice Gateway WebSocket");
                is_connected.store(true, Ordering::SeqCst);
                let _ = event_sender.send(VoiceGatewayEvent::Connected);
                backoff_seconds = 1;

                let (mut ws_write, mut ws_read) = ws_stream.split();
                let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<WsMessage>(32);
                *outbound_tx.write().await = Some(cmd_tx.clone());

                {
                    let mut sm = state_machine.write().await;
                    let _ = sm.start();
                }

                let (heartbeat_stop_tx, mut heartbeat_stop_rx) = tokio::sync::oneshot::channel::<()>();
                let (hb_interval_tx, hb_interval_rx) = tokio::sync::oneshot::channel::<u64>();

                // Heartbeat Background Task
                let cmd_tx_hb = cmd_tx.clone();
                let sm_hb = state_machine.clone();
                tokio::spawn(async move {
                    let interval_ms = match hb_interval_rx.await {
                        Ok(interval) => interval,
                        Err(_) => return,
                    };

                    let mut interval_timer = tokio::time::interval(Duration::from_millis(interval_ms));
                    loop {
                        tokio::select! {
                            _ = interval_timer.tick() => {
                                let timestamp = chrono::Utc::now().timestamp_millis() as u64;
                                let action = {
                                    let mut sm = sm_hb.write().await;
                                    sm.trigger_heartbeat(Some(timestamp))
                                };
                                if let Ok(VoiceGatewayAction::SendMessage(text)) = action {
                                    if cmd_tx_hb.send(WsMessage::Text(text.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            _ = &mut heartbeat_stop_rx => {
                                break;
                            }
                        }
                    }
                });

                let mut hb_interval_sender = Some(hb_interval_tx);

                // WebSocket Message Processing Loop
                loop {
                    tokio::select! {
                        Some(msg_to_send) = cmd_rx.recv() => {
                            if ws_write.send(msg_to_send).await.is_err() {
                                break;
                            }
                        }
                        incoming = ws_read.next() => {
                            match incoming {
                                Some(Ok(WsMessage::Text(text))) => {
                                    let actions_result = {
                                        let mut sm = state_machine.write().await;
                                        sm.handle_incoming_text(&text)
                                    };

                                    match actions_result {
                                        Ok(actions) => {
                                            for action in actions {
                                                match action {
                                                    VoiceGatewayAction::SendMessage(payload_text) => {
                                                        let _ = ws_write.send(WsMessage::Text(payload_text.into())).await;
                                                    }
                                                    VoiceGatewayAction::SendBinaryMessage(bin) => {
                                                        let _ = ws_write.send(WsMessage::Binary(bin.into())).await;
                                                    }
                                                    VoiceGatewayAction::StartHeartbeat { interval_ms } => {
                                                        if let Some(tx) = hb_interval_sender.take() {
                                                            let _ = tx.send(interval_ms);
                                                        }
                                                    }
                                                    VoiceGatewayAction::ReadyReceived(ready) => {
                                                        let _ = ready_sender.send(Some(ready.clone()));
                                                        let _ = event_sender.send(VoiceGatewayEvent::Ready(ready));
                                                    }
                                                    VoiceGatewayAction::SessionDescriptionReceived(desc) => {
                                                        let _ = session_sender.send(Some(desc.clone()));
                                                        let _ = event_sender.send(VoiceGatewayEvent::SessionDescription(desc));
                                                    }
                                                    VoiceGatewayAction::SpeakingUpdate(speaking) => {
                                                        let _ = event_sender.send(VoiceGatewayEvent::Speaking(speaking));
                                                    }
                                                    VoiceGatewayAction::HeartbeatAckReceived { timestamp } => {
                                                        let latency = timestamp.map(|t| {
                                                            let now = chrono::Utc::now().timestamp_millis() as u64;
                                                            now.saturating_sub(t)
                                                        });
                                                        let _ = event_sender.send(VoiceGatewayEvent::HeartbeatAck { latency_ms: latency });
                                                    }
                                                    VoiceGatewayAction::Resumed => {
                                                        let _ = event_sender.send(VoiceGatewayEvent::Resumed);
                                                    }
                                                    VoiceGatewayAction::DavePrepareTransition(prep) => {
                                                        let _ = event_sender.send(VoiceGatewayEvent::DavePrepareTransition(prep));
                                                    }
                                                    VoiceGatewayAction::DaveExecuteTransition(exec) => {
                                                        let _ = event_sender.send(VoiceGatewayEvent::DaveExecuteTransition(exec));
                                                    }
                                                    VoiceGatewayAction::DavePrepareEpoch(epoch) => {
                                                        let _ = event_sender.send(VoiceGatewayEvent::DavePrepareEpoch(epoch));
                                                    }
                                                    VoiceGatewayAction::DaveAnnounceCommit(announce) => {
                                                        let _ = event_sender.send(VoiceGatewayEvent::DaveAnnounceCommit(announce));
                                                    }
                                                    VoiceGatewayAction::CloseConnection { reason, can_resume } => {
                                                        log::warn!("Voice connection closing: {}", reason);
                                                        let decision = if can_resume {
                                                            VoiceCloseDecision::Resume
                                                        } else {
                                                            VoiceCloseDecision::Fatal
                                                        };
                                                        let _ = event_sender.send(VoiceGatewayEvent::Disconnected {
                                                            code: None,
                                                            reason: reason.clone(),
                                                            decision,
                                                        });
                                                        if !can_resume {
                                                            stop_signal.store(true, Ordering::SeqCst);
                                                        }
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("Error handling voice gateway payload: {}", e);
                                        }
                                    }
                                }
                                Some(Ok(WsMessage::Binary(bin_bytes))) => {
                                    let actions_result = {
                                        let mut sm = state_machine.write().await;
                                        sm.handle_incoming_binary(&bin_bytes)
                                    };

                                    match actions_result {
                                        Ok(actions) => {
                                            for action in actions {
                                                match action {
                                                    VoiceGatewayAction::SendBinaryMessage(bin) => {
                                                        let _ = ws_write.send(WsMessage::Binary(bin.into())).await;
                                                    }
                                                    VoiceGatewayAction::SendMessage(payload_text) => {
                                                        let _ = ws_write.send(WsMessage::Text(payload_text.into())).await;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("Error handling binary voice gateway payload: {}", e);
                                        }
                                    }
                                }
                                Some(Ok(WsMessage::Close(frame))) => {
                                    let (code, raw_reason) = if let Some(ref f) = frame {
                                        (u16::from(f.code), f.reason.to_string())
                                    } else {
                                        (1006, "Abnormal closure".to_string())
                                    };

                                    let (decision, _) = {
                                        let mut sm = state_machine.write().await;
                                        sm.handle_close_code(code, &raw_reason)
                                    };

                                    let desc = describe_voice_close_code(code);
                                    let full_reason = if raw_reason.trim().is_empty() {
                                        desc.to_string()
                                    } else {
                                        format!("{}: {}", desc, raw_reason.trim())
                                    };

                                    log::warn!(
                                        "Voice Gateway closed (code={}, decision={:?}): {}",
                                        code,
                                        decision,
                                        full_reason
                                    );

                                    let _ = event_sender.send(VoiceGatewayEvent::Disconnected {
                                        code: Some(code),
                                        reason: full_reason,
                                        decision,
                                    });

                                    if decision == VoiceCloseDecision::Fatal {
                                        // Stop reconnection loop completely for fatal close codes
                                        stop_signal.store(true, Ordering::SeqCst);
                                    }
                                    break;
                                }
                                Some(Err(e)) => {
                                    log::error!("Voice Gateway stream error: {}", e);
                                    let mut sm = state_machine.write().await;
                                    sm.handle_disconnect(true);
                                    let _ = event_sender.send(VoiceGatewayEvent::Disconnected {
                                        code: Some(1006),
                                        reason: format!("Stream error: {}", e),
                                        decision: VoiceCloseDecision::Resume,
                                    });
                                    break;
                                }
                                None => {
                                    log::warn!("Voice Gateway stream EOF");
                                    let mut sm = state_machine.write().await;
                                    sm.handle_disconnect(true);
                                    let _ = event_sender.send(VoiceGatewayEvent::Disconnected {
                                        code: Some(1006),
                                        reason: "Stream EOF".to_string(),
                                        decision: VoiceCloseDecision::Resume,
                                    });
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                let _ = heartbeat_stop_tx.send(());
                *outbound_tx.write().await = None;
                is_connected.store(false, Ordering::SeqCst);
            }
            Err(e) => {
                log::error!("Failed to connect to Voice Gateway: {}", e);
                is_connected.store(false, Ordering::SeqCst);
                let _ = event_sender.send(VoiceGatewayEvent::Disconnected {
                    code: None,
                    reason: format!("Connection error: {}", e),
                    decision: VoiceCloseDecision::Resume,
                });
            }
        }

        if stop_signal.load(Ordering::SeqCst) {
            break;
        }

        tokio::time::sleep(Duration::from_secs(backoff_seconds)).await;
        backoff_seconds = (backoff_seconds * 2).min(15);
    }
}

// ============================================================================
// UNIT TESTS (Network-free, Opcodes & State Machine)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> VoiceGatewayConfig {
        VoiceGatewayConfig {
            endpoint: "voice-server.discord.gg".to_string(),
            server_id: "guild_12345".to_string(),
            user_id: "user_67890".to_string(),
            session_id: "session_abcde".to_string(),
            token: "voice_token_xyz".to_string(),
            max_dave_protocol_version: Some(0),
        }
    }

    #[test]
    fn test_format_voice_gateway_url() {
        assert_eq!(
            format_voice_gateway_url("voice.discord.media"),
            "wss://voice.discord.media/?v=8"
        );
        assert_eq!(
            format_voice_gateway_url("voice.discord.media:443"),
            "wss://voice.discord.media/?v=8"
        );
        assert_eq!(
            format_voice_gateway_url("wss://voice.discord.media:443"),
            "wss://voice.discord.media/?v=8"
        );
        assert_eq!(
            format_voice_gateway_url("ws://voice.discord.media:80"),
            "wss://voice.discord.media/?v=8"
        );
    }

    #[test]
    fn test_opcode_0_identify_serialization() {
        let config = sample_config();
        let sm = VoiceGatewayStateMachine::new(config);
        let identify = sm.generate_identify();

        let json_val = serde_json::to_value(&identify).unwrap();
        assert_eq!(json_val["op"], 0);
        assert_eq!(json_val["d"]["server_id"], "guild_12345");
        assert_eq!(json_val["d"]["user_id"], "user_67890");
        assert_eq!(json_val["d"]["session_id"], "session_abcde");
        assert_eq!(json_val["d"]["token"], "voice_token_xyz");
        assert_eq!(json_val["d"]["max_dave_protocol_version"], 0);
    }

    #[test]
    fn test_opcode_1_select_protocol_serialization() {
        let config = sample_config();
        let sm = VoiceGatewayStateMachine::new(config);
        let select = sm.generate_select_protocol("198.51.100.1", 50002, PREFERRED_ENCRYPTION_MODE);

        let json_val = serde_json::to_value(&select).unwrap();
        assert_eq!(json_val["op"], 1);
        assert_eq!(json_val["d"]["protocol"], "udp");
        assert_eq!(json_val["d"]["data"]["address"], "198.51.100.1");
        assert_eq!(json_val["d"]["data"]["port"], 50002);
        assert_eq!(json_val["d"]["data"]["mode"], "aead_aes256_gcm_rtpsize");
    }

    #[test]
    fn test_opcode_2_ready_deserialization() {
        let ready_json = r#"{
            "op": 2,
            "d": {
                "ssrc": 10452,
                "ip": "198.51.100.42",
                "port": 50005,
                "modes": [
                    "aead_aes256_gcm_rtpsize",
                    "aead_xchacha20_poly1305_rtpsize"
                ],
                "experiments": []
            }
        }"#;

        let payload: VoiceGatewayPayload<ReadyPayload> = serde_json::from_str(ready_json).unwrap();
        assert_eq!(payload.op, 2);
        let ready = payload.d.unwrap();
        assert_eq!(ready.ssrc, 10452);
        assert_eq!(ready.ip, "198.51.100.42");
        assert_eq!(ready.port, 50005);
        assert_eq!(ready.modes.len(), 2);

        let voice_ready = VoiceReady {
            ssrc: ready.ssrc,
            ip: ready.ip,
            port: ready.port,
            modes: ready.modes,
        };
        assert_eq!(voice_ready.select_best_mode(), "aead_aes256_gcm_rtpsize");
    }

    #[test]
    fn test_opcode_3_heartbeat_with_seq_ack() {
        let config = sample_config();
        let mut sm = VoiceGatewayStateMachine::new(config);
        sm.last_seq = 42;

        let hb = sm.generate_heartbeat(Some(1620000000000));
        let json_val = serde_json::to_value(&hb).unwrap();
        assert_eq!(json_val["op"], 3);
        assert_eq!(json_val["d"]["seq_ack"], 42);
        assert_eq!(json_val["d"]["t"], 1620000000000u64);
    }

    #[test]
    fn test_opcode_4_session_description_deserialization() {
        let secret = vec![7u8; 32];
        let secret_json = serde_json::to_string(&secret).unwrap();
        let payload_str = format!(
            r#"{{
                "op": 4,
                "d": {{
                    "mode": "aead_aes256_gcm_rtpsize",
                    "secret_key": {},
                    "dave_protocol_version": 0
                }}
            }}"#,
            secret_json
        );

        let payload: VoiceGatewayPayload<SessionDescriptionPayload> =
            serde_json::from_str(&payload_str).unwrap();
        assert_eq!(payload.op, 4);
        let session = payload.d.unwrap();
        assert_eq!(session.mode, "aead_aes256_gcm_rtpsize");
        assert_eq!(session.secret_key.len(), 32);
        assert_eq!(session.secret_key[0], 7);
    }

    #[test]
    fn test_opcode_5_speaking_serialization_and_deserialization() {
        let config = sample_config();
        let sm = VoiceGatewayStateMachine::new(config);
        let speaking = sm.generate_speaking(true, 0, 10452);

        let json_val = serde_json::to_value(&speaking).unwrap();
        assert_eq!(json_val["op"], 5);
        assert_eq!(json_val["d"]["speaking"], 1);
        assert_eq!(json_val["d"]["ssrc"], 10452);

        // Deserialization of server speaking notification
        let server_msg = r#"{
            "op": 5,
            "d": {
                "speaking": 1,
                "delay": 0,
                "ssrc": 10452,
                "user_id": "user_67890"
            }
        }"#;
        let incoming: VoiceGatewayPayload<SpeakingPayload> = serde_json::from_str(server_msg).unwrap();
        let parsed = incoming.d.unwrap();
        assert_eq!(parsed.user_id.as_deref(), Some("user_67890"));
        assert_eq!(parsed.ssrc, 10452);
    }

    #[test]
    fn test_opcode_6_heartbeat_ack_deserialization() {
        let ack_json = r#"{
            "op": 6,
            "d": 1620000000000
        }"#;
        let payload: VoiceGatewayPayload<u64> = serde_json::from_str(ack_json).unwrap();
        assert_eq!(payload.op, 6);
        assert_eq!(payload.d, Some(1620000000000));
    }

    #[test]
    fn test_opcode_7_resume_serialization() {
        let config = sample_config();
        let mut sm = VoiceGatewayStateMachine::new(config);
        sm.last_seq = 15;

        let resume = sm.generate_resume();
        let json_val = serde_json::to_value(&resume).unwrap();
        assert_eq!(json_val["op"], 7);
        assert_eq!(json_val["d"]["server_id"], "guild_12345");
        assert_eq!(json_val["d"]["session_id"], "session_abcde");
        assert_eq!(json_val["d"]["token"], "voice_token_xyz");
        assert_eq!(json_val["d"]["seq_ack"], 15);
    }

    #[test]
    fn test_opcode_8_hello_deserialization() {
        let hello_json = r#"{
            "op": 8,
            "d": {
                "v": 8,
                "heartbeat_interval": 41250.0
            }
        }"#;
        let payload: VoiceGatewayPayload<HelloPayload> = serde_json::from_str(hello_json).unwrap();
        assert_eq!(payload.op, 8);
        let hello = payload.d.unwrap();
        assert_eq!(hello.v, Some(8));
        assert_eq!(hello.heartbeat_interval, 41250.0);
    }

    #[test]
    fn test_opcode_9_resumed_deserialization() {
        let resumed_json = r#"{
            "op": 9,
            "d": null
        }"#;
        let payload: VoiceGatewayPayload<serde_json::Value> =
            serde_json::from_str(resumed_json).unwrap();
        assert_eq!(payload.op, 9);
        assert!(payload.d.is_none());
    }

    #[test]
    fn test_state_machine_full_handshake_flow() {
        let mut sm = VoiceGatewayStateMachine::new(sample_config());
        assert_eq!(*sm.current_state(), VoiceConnectionState::Disconnected);

        // 1. Connection starts
        sm.start().unwrap();
        assert_eq!(*sm.current_state(), VoiceConnectionState::WaitingForHello);

        // 2. Server sends Op 8 Hello
        let hello_json = r#"{"op": 8, "d": {"v": 8, "heartbeat_interval": 20000.0}}"#;
        let actions = sm.handle_incoming_text(hello_json).unwrap();
        assert_eq!(*sm.current_state(), VoiceConnectionState::Identifying);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0], VoiceGatewayAction::StartHeartbeat { interval_ms: 20000 });
        if let VoiceGatewayAction::SendMessage(identify_str) = &actions[1] {
            let val: serde_json::Value = serde_json::from_str(identify_str).unwrap();
            assert_eq!(val["op"], 0);
        } else {
            panic!("Expected SendMessage(Identify)");
        }

        // 3. Server sends Op 2 Ready with seq = 1
        let ready_json = r#"{
            "op": 2,
            "seq": 1,
            "d": {
                "ssrc": 5555,
                "ip": "1.2.3.4",
                "port": 50001,
                "modes": ["aead_aes256_gcm_rtpsize", "aead_xchacha20_poly1305_rtpsize"]
            }
        }"#;
        let actions = sm.handle_incoming_text(ready_json).unwrap();
        assert_eq!(sm.last_seq(), 1);
        let expected_ready = VoiceReady {
            ssrc: 5555,
            ip: "1.2.3.4".to_string(),
            port: 50001,
            modes: vec![
                "aead_aes256_gcm_rtpsize".to_string(),
                "aead_xchacha20_poly1305_rtpsize".to_string(),
            ],
        };
        assert_eq!(*sm.current_state(), VoiceConnectionState::Ready(expected_ready.clone()));
        assert_eq!(actions, vec![VoiceGatewayAction::ReadyReceived(expected_ready.clone())]);

        // 4. IP Discovery finishes; client selects protocol (Op 1)
        let action = sm.select_protocol("198.51.100.99", 40000, "aead_aes256_gcm_rtpsize").unwrap();
        assert_eq!(*sm.current_state(), VoiceConnectionState::SelectingProtocol);
        if let VoiceGatewayAction::SendMessage(sel_str) = action {
            let val: serde_json::Value = serde_json::from_str(&sel_str).unwrap();
            assert_eq!(val["op"], 1);
            assert_eq!(val["d"]["data"]["address"], "198.51.100.99");
            assert_eq!(val["d"]["data"]["port"], 40000);
        } else {
            panic!("Expected SendMessage(SelectProtocol)");
        }

        // 5. Server responds with Op 4 Session Description with seq = 2
        let secret = vec![9u8; 32];
        let secret_json = serde_json::to_string(&secret).unwrap();
        let session_json = format!(
            r#"{{"op": 4, "seq": 2, "d": {{"mode": "aead_aes256_gcm_rtpsize", "secret_key": {}}}}}"#,
            secret_json
        );
        let actions = sm.handle_incoming_text(&session_json).unwrap();
        assert_eq!(sm.last_seq(), 2);
        let expected_desc = SessionDescription {
            mode: "aead_aes256_gcm_rtpsize".to_string(),
            secret_key: secret,
            dave_protocol_version: None,
        };
        assert_eq!(*sm.current_state(), VoiceConnectionState::Connected(expected_desc.clone()));
        assert_eq!(actions, vec![VoiceGatewayAction::SessionDescriptionReceived(expected_desc)]);

        // 6. Periodic Heartbeat triggered with seq_ack = 2
        let hb_action = sm.trigger_heartbeat(Some(12345678)).unwrap();
        if let VoiceGatewayAction::SendMessage(hb_str) = hb_action {
            let val: serde_json::Value = serde_json::from_str(&hb_str).unwrap();
            assert_eq!(val["op"], 3);
            assert_eq!(val["d"]["seq_ack"], 2);
            assert_eq!(val["d"]["t"], 12345678);
        } else {
            panic!("Expected SendMessage(Heartbeat)");
        }

        // 7. Server responds with Op 6 Heartbeat ACK
        let ack_json = r#"{"op": 6, "d": 12345678}"#;
        let actions = sm.handle_incoming_text(ack_json).unwrap();
        assert_eq!(actions, vec![VoiceGatewayAction::HeartbeatAckReceived { timestamp: Some(12345678) }]);

        // 8. Speaking update received
        let speaking_json = r#"{"op": 5, "d": {"speaking": 1, "delay": 0, "ssrc": 5555, "user_id": "user_67890"}}"#;
        let actions = sm.handle_incoming_text(speaking_json).unwrap();
        assert_eq!(actions.len(), 1);
        if let VoiceGatewayAction::SpeakingUpdate(sp) = &actions[0] {
            assert_eq!(sp.ssrc, 5555);
            assert_eq!(sp.user_id.as_deref(), Some("user_67890"));
        } else {
            panic!("Expected SpeakingUpdate");
        }
    }

    #[test]
    fn test_state_machine_resume_flow() {
        let mut sm = VoiceGatewayStateMachine::new(sample_config());
        sm.last_seq = 10;
        let session_desc = SessionDescription {
            mode: "aead_aes256_gcm_rtpsize".to_string(),
            secret_key: vec![1; 32],
            dave_protocol_version: None,
        };
        sm.session_description = Some(session_desc.clone());
        sm.state = VoiceConnectionState::Connected(session_desc.clone());

        // Disconnection happens with can_resume = true
        let action = sm.handle_disconnect(true);
        assert_eq!(action, None);
        assert_eq!(*sm.current_state(), VoiceConnectionState::Resuming);

        // Server sends Op 8 Hello on reconnect
        let hello_json = r#"{"op": 8, "d": {"v": 8, "heartbeat_interval": 30000.0}}"#;
        let actions = sm.handle_incoming_text(hello_json).unwrap();
        assert_eq!(actions.len(), 2);
        // Second action must be Resume (Op 7) with seq_ack = 10
        if let VoiceGatewayAction::SendMessage(resume_str) = &actions[1] {
            let val: serde_json::Value = serde_json::from_str(resume_str).unwrap();
            assert_eq!(val["op"], 7);
            assert_eq!(val["d"]["seq_ack"], 10);
            assert_eq!(val["d"]["session_id"], "session_abcde");
        } else {
            panic!("Expected SendMessage(Resume)");
        }

        // Server acknowledges resume with Op 9 Resumed
        let resumed_json = r#"{"op": 9, "d": null}"#;
        let actions = sm.handle_incoming_text(resumed_json).unwrap();
        assert_eq!(actions, vec![VoiceGatewayAction::Resumed]);
        assert_eq!(*sm.current_state(), VoiceConnectionState::Connected(session_desc));
    }

    #[test]
    fn test_encryption_mode_fallback() {
        let modes_aes_only = VoiceReady {
            ssrc: 1,
            ip: "127.0.0.1".to_string(),
            port: 1234,
            modes: vec![PREFERRED_ENCRYPTION_MODE.to_string()],
        };
        assert_eq!(modes_aes_only.select_best_mode(), PREFERRED_ENCRYPTION_MODE);

        let modes_fallback_only = VoiceReady {
            ssrc: 1,
            ip: "127.0.0.1".to_string(),
            port: 1234,
            modes: vec![FALLBACK_ENCRYPTION_MODE.to_string()],
        };
        assert_eq!(modes_fallback_only.select_best_mode(), FALLBACK_ENCRYPTION_MODE);

        let modes_both = VoiceReady {
            ssrc: 1,
            ip: "127.0.0.1".to_string(),
            port: 1234,
            modes: vec![
                FALLBACK_ENCRYPTION_MODE.to_string(),
                PREFERRED_ENCRYPTION_MODE.to_string(),
            ],
        };
        assert_eq!(modes_both.select_best_mode(), PREFERRED_ENCRYPTION_MODE);
    }

    #[test]
    fn test_voice_close_code_classification() {
        // Resumable codes
        assert_eq!(VoiceCloseDecision::from_close_code(4015), VoiceCloseDecision::Resume);
        assert_eq!(VoiceCloseDecision::from_close_code(1001), VoiceCloseDecision::Resume);
        assert_eq!(VoiceCloseDecision::from_close_code(1006), VoiceCloseDecision::Resume);
        assert!(VoiceCloseDecision::from_close_code(4015).can_resume());
        assert!(VoiceCloseDecision::from_close_code(4015).should_reconnect());

        // NewSession codes (must re-identify)
        assert_eq!(VoiceCloseDecision::from_close_code(4003), VoiceCloseDecision::NewSession);
        assert_eq!(VoiceCloseDecision::from_close_code(4005), VoiceCloseDecision::NewSession);
        assert_eq!(VoiceCloseDecision::from_close_code(4006), VoiceCloseDecision::NewSession);
        assert_eq!(VoiceCloseDecision::from_close_code(4009), VoiceCloseDecision::NewSession);
        assert!(!VoiceCloseDecision::from_close_code(4006).can_resume());
        assert!(VoiceCloseDecision::from_close_code(4006).should_reconnect());

        // Fatal codes (must NOT reconnect)
        assert_eq!(VoiceCloseDecision::from_close_code(4001), VoiceCloseDecision::Fatal);
        assert_eq!(VoiceCloseDecision::from_close_code(4002), VoiceCloseDecision::Fatal);
        assert_eq!(VoiceCloseDecision::from_close_code(4004), VoiceCloseDecision::Fatal);
        assert_eq!(VoiceCloseDecision::from_close_code(4011), VoiceCloseDecision::Fatal);
        assert_eq!(VoiceCloseDecision::from_close_code(4012), VoiceCloseDecision::Fatal);
        assert_eq!(VoiceCloseDecision::from_close_code(4014), VoiceCloseDecision::Fatal);
        assert_eq!(VoiceCloseDecision::from_close_code(4016), VoiceCloseDecision::Fatal);
        assert_eq!(VoiceCloseDecision::from_close_code(4017), VoiceCloseDecision::Fatal);
        assert_eq!(VoiceCloseDecision::from_close_code(4020), VoiceCloseDecision::Fatal);
        assert_eq!(VoiceCloseDecision::from_close_code(4022), VoiceCloseDecision::Fatal);
        assert_eq!(VoiceCloseDecision::from_close_code(1000), VoiceCloseDecision::Fatal);
        assert_eq!(VoiceCloseDecision::from_close_code(4999), VoiceCloseDecision::Fatal);
        assert!(!VoiceCloseDecision::from_close_code(4017).should_reconnect());
        assert!(!VoiceCloseDecision::from_close_code(4014).should_reconnect());
    }

    #[test]
    fn test_describe_voice_close_codes() {
        assert!(describe_voice_close_code(4017).contains("DAVE"));
        assert!(describe_voice_close_code(4014).contains("kicked"));
        assert!(describe_voice_close_code(4004).contains("Authentication failed"));
        assert!(describe_voice_close_code(4015).contains("crashed"));
    }

    #[test]
    fn test_state_machine_handle_close_code_resume() {
        let mut sm = VoiceGatewayStateMachine::new(sample_config());
        sm.last_seq = 42;
        let (decision, action) = sm.handle_close_code(4015, "Server restart");
        assert_eq!(decision, VoiceCloseDecision::Resume);
        assert_eq!(action, None);
        assert_eq!(*sm.current_state(), VoiceConnectionState::Resuming);
        assert_eq!(sm.last_seq(), 42); // Sequence retained for resume
    }

    #[test]
    fn test_state_machine_handle_close_code_new_session() {
        let mut sm = VoiceGatewayStateMachine::new(sample_config());
        sm.last_seq = 42;
        let (decision, action) = sm.handle_close_code(4006, "Session expired");
        assert_eq!(decision, VoiceCloseDecision::NewSession);
        assert_eq!(action, None);
        assert_eq!(*sm.current_state(), VoiceConnectionState::WaitingForHello);
        assert_eq!(sm.last_seq(), -1); // Sequence reset to force Opcode 0 Identify
    }

    #[test]
    fn test_state_machine_handle_close_code_fatal_dave_required() {
        let mut sm = VoiceGatewayStateMachine::new(sample_config());
        sm.last_seq = 42;
        let (decision, action) = sm.handle_close_code(4017, "");
        assert_eq!(decision, VoiceCloseDecision::Fatal);
        match sm.current_state() {
            VoiceConnectionState::Failed(msg) => {
                assert!(msg.contains("DAVE"));
            }
            s => panic!("Expected Failed state, got {:?}", s),
        }
        if let Some(VoiceGatewayAction::CloseConnection { reason, can_resume }) = action {
            assert!(!can_resume);
            assert!(reason.contains("DAVE"));
        } else {
            panic!("Expected CloseConnection action with can_resume: false");
        }
    }

    #[test]
    fn test_state_machine_handle_close_code_fatal_kicked() {
        let mut sm = VoiceGatewayStateMachine::new(sample_config());
        let (decision, action) = sm.handle_close_code(4014, "");
        assert_eq!(decision, VoiceCloseDecision::Fatal);
        match sm.current_state() {
            VoiceConnectionState::Failed(msg) => {
                assert!(msg.contains("kicked"));
            }
            s => panic!("Expected Failed state, got {:?}", s),
        }
        assert!(matches!(action, Some(VoiceGatewayAction::CloseConnection { can_resume: false, .. })));
    }

    #[test]
    fn test_dave_identify_default_version() {
        let mut cfg = sample_config();
        cfg.max_dave_protocol_version = None;
        let sm = VoiceGatewayStateMachine::new(cfg);
        let id = sm.generate_identify();
        let val = serde_json::to_value(&id).unwrap();
        assert_eq!(val["d"]["max_dave_protocol_version"], 1);
    }

    #[test]
    fn test_dave_session_description_negotiation_and_transition() {
        let mut sm = VoiceGatewayStateMachine::new(sample_config());
        assert_eq!(sm.dave_session().protocol_version(), 0);

        // Server negotiates DAVE version 1 in SessionDescription
        let session_raw = r#"{
            "op": 4,
            "d": {
                "mode": "aead_aes256_gcm_rtpsize",
                "secret_key": [0,1,2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,7,8,9,0,1],
                "dave_protocol_version": 1
            }
        }"#;
        let actions = sm.handle_incoming_text(session_raw).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(sm.dave_session().protocol_version(), 1);

        // Server sends Opcode 21 Prepare Transition
        let prep_raw = r#"{"op": 21, "d": {"transition_id": 100, "protocol_version": 1}}"#;
        let prep_actions = sm.handle_incoming_text(prep_raw).unwrap();
        assert_eq!(prep_actions.len(), 1);
        // Expect Opcode 23 Transition Ready
        if let VoiceGatewayAction::SendMessage(msg) = &prep_actions[0] {
            let val: serde_json::Value = serde_json::from_str(msg).unwrap();
            assert_eq!(val["op"], 23);
            assert_eq!(val["d"]["transition_id"], 100);
        } else {
            panic!("Expected Opcode 23 Transition Ready");
        }

        // Server sends Opcode 22 Execute Transition
        let exec_raw = r#"{"op": 22, "d": {"transition_id": 100}}"#;
        let exec_actions = sm.handle_incoming_text(exec_raw).unwrap();
        assert!(exec_actions.is_empty());

        // Test speaking mapping
        let speaking_raw = r#"{"op": 5, "d": {"speaking": 1, "ssrc": 9999, "user_id": "123456789"}}"#;
        sm.handle_incoming_text(speaking_raw).unwrap();
        assert_eq!(sm.dave_session().get_user_id_by_ssrc(9999), Some(123456789));

        // Media passthrough test
        let frame = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let encrypted = sm.dave_session_mut().encrypt_opus_frame(&frame).unwrap();
        assert_eq!(encrypted, frame); // In passthrough fallback while MLS group forms

        let decrypted = sm.dave_session_mut().decrypt_opus_frame_by_ssrc(9999, &frame).unwrap();
        assert_eq!(decrypted, frame);
    }

    #[test]
    fn test_dave_binary_frame_handling() {
        let mut sm = VoiceGatewayStateMachine::new(sample_config());
        sm.dave_session_mut().init_session(1).unwrap();

        // Synthetic Opcode 25: external sender package with malformed bytes should return Err cleanly
        let server_bin = vec![0x00, 0x0F, 25];
        let res = sm.handle_incoming_binary(&server_bin);
        assert!(res.is_err());
    }
}
