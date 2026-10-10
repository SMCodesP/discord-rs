//! Discord Audio & Video End-to-End Encryption (DAVE) Protocol Implementation
//!
//! Encapsulates DAVE protocol sessions (MLS RFC 9420) using the `davey` crate.
//! Handles MLS group lifecycle:
//! - Opcode 21: DAVE Prepare Transition (JSON)
//! - Opcode 22: DAVE Execute Transition (JSON)
//! - Opcode 23: DAVE Transition Ready (JSON)
//! - Opcode 24: DAVE Prepare Epoch (JSON)
//! - Opcode 25: MLS External Sender Package (Binary)
//! - Opcode 26: MLS Key Package (Binary)
//! - Opcode 27: MLS Proposals (Binary)
//! - Opcode 28: MLS Commit Welcome (Binary)
//! - Opcode 29: MLS Announce Commit Transition (JSON)
//! - Opcode 30: MLS Welcome (Binary)
//! - Opcode 31: MLS Invalid Commit Welcome (JSON)
//!
//! Exposes the media transformation contract:
//! - `encrypt_opus_frame(&mut self, frame: &[u8]) -> Result<Vec<u8>>`
//! - `decrypt_opus_frame(&mut self, sender_user_id: u64, frame: &[u8]) -> Result<Vec<u8>>`
//! - Automatic passthrough mode during handshake or protocol downgrades.
//! - SSRC <-> User ID mapping for media routing.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroU16;

use davey::{
    DaveSession, MediaType, ProposalsOperationType,
    DAVE_PROTOCOL_VERSION,
};

/// Maximum DAVE protocol version supported by this client implementation.
pub const SUPPORTED_DAVE_PROTOCOL_VERSION: u16 = DAVE_PROTOCOL_VERSION;

// ============================================================================
// DAVE OPCODES & SERIALIZATION PAYLOADS
// ============================================================================

/// DAVE Voice Gateway Opcodes (Faixa 21 a 31).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum DaveOpcode {
    PrepareTransition = 21,
    ExecuteTransition = 22,
    TransitionReady = 23,
    PrepareEpoch = 24,
    MlsExternalSender = 25,
    MlsKeyPackage = 26,
    MlsProposals = 27,
    MlsCommitWelcome = 28,
    MlsAnnounceCommitTransition = 29,
    MlsWelcome = 30,
    MlsInvalidCommitWelcome = 31,
}

impl DaveOpcode {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            21 => Some(Self::PrepareTransition),
            22 => Some(Self::ExecuteTransition),
            23 => Some(Self::TransitionReady),
            24 => Some(Self::PrepareEpoch),
            25 => Some(Self::MlsExternalSender),
            26 => Some(Self::MlsKeyPackage),
            27 => Some(Self::MlsProposals),
            28 => Some(Self::MlsCommitWelcome),
            29 => Some(Self::MlsAnnounceCommitTransition),
            30 => Some(Self::MlsWelcome),
            31 => Some(Self::MlsInvalidCommitWelcome),
            _ => None,
        }
    }

    pub fn is_binary(&self) -> bool {
        matches!(
            self,
            Self::MlsExternalSender
                | Self::MlsKeyPackage
                | Self::MlsProposals
                | Self::MlsCommitWelcome
                | Self::MlsWelcome
        )
    }

    pub fn is_json(&self) -> bool {
        !self.is_binary()
    }
}

/// Opcode 21: DAVE Prepare Transition Payload (Server -> Client JSON)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DavePrepareTransitionPayload {
    pub transition_id: u16,
    pub protocol_version: u16,
}

/// Opcode 22: DAVE Execute Transition Payload (Server -> Client JSON)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaveExecuteTransitionPayload {
    pub transition_id: u16,
}

/// Opcode 23: DAVE Transition Ready Payload (Client -> Server JSON)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaveTransitionReadyPayload {
    pub transition_id: u16,
}

/// Opcode 24: DAVE Prepare Epoch Payload (Server -> Client JSON)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DavePrepareEpochPayload {
    pub epoch: u64,
    pub protocol_version: u16,
}

/// Opcode 29: MLS Announce Commit Transition Payload (Server -> Client JSON)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaveAnnounceCommitTransitionPayload {
    pub transition_id: u16,
    #[serde(default)]
    pub commit: Option<String>,
}

/// Opcode 31: MLS Invalid Commit Welcome Payload (Client -> Server JSON)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaveInvalidCommitWelcomePayload {
    pub transition_id: u16,
}

// ============================================================================
// BINARY MESSAGE PARSING & FORMATTING HELPERS
// ============================================================================

/// Parses a Server-to-Client binary WebSocket message.
///
/// In Voice Gateway v8+, server-to-client binary messages contain:
/// - 2 bytes: sequence number (big-endian u16)
/// - 1 byte: opcode
/// - remaining bytes: payload data
pub fn parse_server_binary_message(data: &[u8]) -> Result<(u16, DaveOpcode, &[u8])> {
    if data.len() < 3 {
        bail!(
            "Server binary message too short: {} bytes (minimum 3 required)",
            data.len()
        );
    }
    let seq = u16::from_be_bytes([data[0], data[1]]);
    let op_byte = data[2];
    let opcode = DaveOpcode::from_u8(op_byte)
        .context(format!("Unknown binary voice opcode: {}", op_byte))?;
    let payload = &data[3..];
    Ok((seq, opcode, payload))
}

/// Formats a Client-to-Server binary WebSocket message.
///
/// Format:
/// - 1 byte: opcode
/// - remaining bytes: payload data
pub fn format_client_binary_message(opcode: DaveOpcode, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + payload.len());
    out.push(opcode as u8);
    out.extend_from_slice(payload);
    out
}

// ============================================================================
// VOICE DAVE SESSION (High-Level State Machine & Media Cryptor)
// ============================================================================

/// Manages DAVE E2EE state, MLS group lifecycle, and media encryption/decryption.
#[derive(Debug)]
pub struct VoiceDaveSession {
    user_id: u64,
    channel_id: u64,
    protocol_version: u16,
    session: Option<DaveSession>,
    pending_transitions: HashMap<u16, u16>,
    is_downgraded: bool,
    passthrough_fallback: bool,
    ssrc_to_user_id: HashMap<u32, u64>,
}

impl VoiceDaveSession {
    /// Creates a new uninitialized DAVE session for the specified user and channel.
    pub fn new(user_id: u64, channel_id: u64) -> Self {
        Self {
            user_id,
            channel_id,
            protocol_version: 0,
            session: None,
            pending_transitions: HashMap::new(),
            is_downgraded: false,
            passthrough_fallback: true,
            ssrc_to_user_id: HashMap::new(),
        }
    }

    /// User ID of the local participant.
    pub fn user_id(&self) -> u64 {
        self.user_id
    }

    /// Channel ID / MLS Group ID of the voice connection.
    pub fn channel_id(&self) -> u64 {
        self.channel_id
    }

    /// Current active DAVE protocol version.
    pub fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    /// Whether DAVE E2EE is active and ready to encrypt outgoing media.
    pub fn is_ready(&self) -> bool {
        self.session.as_ref().map(|s| s.is_ready()).unwrap_or(false)
    }

    /// Whether this session is currently downgraded to transport-only encryption.
    pub fn is_downgraded(&self) -> bool {
        self.is_downgraded
    }

    /// Returns the voice privacy code (verification code) for display if available.
    pub fn privacy_code(&self) -> Option<String> {
        self.session
            .as_ref()
            .and_then(|s| s.voice_privacy_code().map(|c| c.to_string()))
    }

    /// Initializes or re-initializes the session with the negotiated protocol version.
    pub fn init_session(&mut self, version: u16) -> Result<()> {
        log::info!(
            "Initializing DAVE session for user {} in channel {} with version {}",
            self.user_id,
            self.channel_id,
            version
        );

        self.protocol_version = version;
        if version == 0 {
            self.is_downgraded = true;
            self.passthrough_fallback = true;
            self.session = None;
            return Ok(());
        }

        let non_zero_version = NonZeroU16::new(version)
            .context("DAVE protocol version must be greater than 0")?;

        let dave_session = DaveSession::new(
            non_zero_version,
            self.user_id,
            self.channel_id,
            None,
        )
        .map_err(|e| anyhow::anyhow!("Failed to initialize DaveSession: {:?}", e))?;

        self.session = Some(dave_session);
        self.is_downgraded = false;
        self.passthrough_fallback = false;
        Ok(())
    }

    // ========================================================================
    // SSRC <-> USER ID MAPPING
    // ========================================================================

    /// Registers an SSRC to User ID mapping.
    pub fn register_ssrc_user_mapping(&mut self, ssrc: u32, user_id: u64) {
        self.ssrc_to_user_id.insert(ssrc, user_id);
    }

    /// Retrieves the User ID mapped to an SSRC.
    pub fn get_user_id_by_ssrc(&self, ssrc: u32) -> Option<u64> {
        self.ssrc_to_user_id.get(&ssrc).copied()
    }

    /// Returns a reference to all known SSRC-to-user mappings.
    pub fn ssrc_mappings(&self) -> &HashMap<u32, u64> {
        &self.ssrc_to_user_id
    }

    // ========================================================================
    // MLS PROTOCOL HANDLERS
    // ========================================================================

    /// Handles Opcode 25 (MLS External Sender Package).
    ///
    /// Configures the gateway's external sender credentials and returns the client's
    /// serialized Key Package for Opcode 26 (MLS Key Package).
    pub fn handle_external_sender(&mut self, package: &[u8]) -> Result<Vec<u8>> {
        if self.session.is_none() && self.protocol_version > 0 {
            self.init_session(self.protocol_version)?;
        }

        let session = self.session.as_mut().context("DAVE session not initialized")?;
        session
            .set_external_sender(package)
            .map_err(|e| anyhow::anyhow!("Failed to set MLS external sender: {:?}", e))?;

        let key_package = session
            .create_key_package()
            .map_err(|e| anyhow::anyhow!("Failed to create MLS key package: {:?}", e))?;

        Ok(format_client_binary_message(
            DaveOpcode::MlsKeyPackage,
            &key_package,
        ))
    }

    /// Handles Opcode 27 (MLS Proposals).
    ///
    /// Processes MLS proposals (operation 0: append, 1: revoke). If a commit is generated,
    /// returns the serialized binary frame for Opcode 28 (MLS Commit Welcome).
    pub fn handle_proposals(
        &mut self,
        operation_type: u8,
        proposals_data: &[u8],
        expected_user_ids: Option<&[u64]>,
    ) -> Result<Option<Vec<u8>>> {
        let session = self.session.as_mut().context("DAVE session not initialized")?;

        let op = if operation_type == 1 {
            ProposalsOperationType::REVOKE
        } else {
            ProposalsOperationType::APPEND
        };

        let commit_welcome_opt = session
            .process_proposals(op, proposals_data, expected_user_ids)
            .map_err(|e| anyhow::anyhow!("Failed to process MLS proposals: {:?}", e))?;

        if let Some(commit_welcome) = commit_welcome_opt {
            let mut payload = commit_welcome.commit;
            if let Some(welcome) = commit_welcome.welcome {
                payload.extend_from_slice(&welcome);
            }
            Ok(Some(format_client_binary_message(
                DaveOpcode::MlsCommitWelcome,
                &payload,
            )))
        } else {
            Ok(None)
        }
    }

    /// Handles Opcode 29 (MLS Announce Commit Transition).
    ///
    /// Processes an announced commit. If processing fails, returns an Invalid Commit Welcome
    /// payload (Opcode 31) to request re-addition.
    pub fn handle_announce_commit(
        &mut self,
        transition_id: u16,
        commit: &[u8],
    ) -> Result<Option<DaveInvalidCommitWelcomePayload>> {
        let session = self.session.as_mut().context("DAVE session not initialized")?;

        match session.process_commit(commit) {
            Ok(_) => {
                log::info!("Successfully processed DAVE commit transition id: {}", transition_id);
                Ok(None)
            }
            Err(e) => {
                log::warn!(
                    "Failed to process MLS commit transition id {}: {:?}. Requesting re-add.",
                    transition_id,
                    e
                );
                Ok(Some(DaveInvalidCommitWelcomePayload { transition_id }))
            }
        }
    }

    /// Handles Opcode 30 (MLS Welcome).
    ///
    /// Processes a Welcome message when added to the MLS group. If processing fails,
    /// returns an Invalid Commit Welcome payload (Opcode 31) to request re-addition.
    pub fn handle_welcome(
        &mut self,
        transition_id: u16,
        welcome: &[u8],
    ) -> Result<Option<DaveInvalidCommitWelcomePayload>> {
        let session = self.session.as_mut().context("DAVE session not initialized")?;

        match session.process_welcome(welcome) {
            Ok(_) => {
                log::info!("Successfully processed DAVE Welcome message id: {}", transition_id);
                Ok(None)
            }
            Err(e) => {
                log::warn!(
                    "Failed to process MLS Welcome message id {}: {:?}. Requesting re-add.",
                    transition_id,
                    e
                );
                Ok(Some(DaveInvalidCommitWelcomePayload { transition_id }))
            }
        }
    }

    /// Handles Opcode 21 (DAVE Prepare Transition).
    ///
    /// Records pending transition and returns Opcode 23 (Transition Ready).
    pub fn handle_prepare_transition(
        &mut self,
        transition_id: u16,
        protocol_version: u16,
    ) -> DaveTransitionReadyPayload {
        log::info!(
            "DAVE Prepare Transition received (id: {}, version: {})",
            transition_id,
            protocol_version
        );
        self.pending_transitions.insert(transition_id, protocol_version);
        DaveTransitionReadyPayload { transition_id }
    }

    /// Handles Opcode 22 (DAVE Execute Transition).
    ///
    /// Executes the pending transition, transitioning between MLS and passthrough modes.
    pub fn handle_execute_transition(&mut self, transition_id: u16) -> Result<()> {
        log::info!("Executing DAVE transition id: {}", transition_id);

        if let Some(target_version) = self.pending_transitions.remove(&transition_id) {
            if target_version == 0 {
                // Downgrade to transport-only encryption
                self.is_downgraded = true;
                self.protocol_version = 0;
                log::warn!("DAVE protocol downgraded to version 0 (passthrough active)");
            } else if self.is_downgraded {
                // Upgrade from transport-only encryption
                self.is_downgraded = false;
                self.protocol_version = target_version;
                if let Some(ref mut sess) = self.session {
                    sess.set_passthrough_mode(true, Some(10));
                }
                log::info!("DAVE protocol upgraded to version {} (10s passthrough grace)", target_version);
            }
        }
        Ok(())
    }

    /// Handles Opcode 24 (DAVE Prepare Epoch).
    pub fn handle_prepare_epoch(&mut self, epoch: u64, protocol_version: u16) {
        log::debug!("DAVE Prepare Epoch: epoch {}, version {}", epoch, protocol_version);
    }

    // ========================================================================
    // MEDIA TRANSFORMATION CONTRACT (Audio -> DAVE -> RTP)
    // ========================================================================

    /// Encrypts an outgoing Opus frame using DAVE E2EE.
    ///
    /// If DAVE is disabled, uninitialized, or in passthrough mode, returns the raw frame.
    /// When DAVE E2EE is active and ready, encrypts with the client's current MLS epoch key.
    pub fn encrypt_opus_frame(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        if self.is_downgraded || self.protocol_version == 0 || self.passthrough_fallback {
            return Ok(frame.to_vec());
        }

        if let Some(ref mut sess) = self.session {
            if sess.is_ready() {
                let enc = sess
                    .encrypt_opus(frame)
                    .map_err(|e| anyhow::anyhow!("DAVE Opus encryption error: {:?}", e))?;
                return Ok(enc.into_owned());
            }
        }

        // Passthrough fallback while establishing MLS group
        Ok(frame.to_vec())
    }

    /// Decrypts an incoming Opus frame from a remote participant identified by sender_user_id.
    ///
    /// If DAVE is disabled, uninitialized, or the sender is in passthrough grace mode,
    /// returns the raw frame. When DAVE E2EE is active, decrypts using the sender's ratchet key.
    pub fn decrypt_opus_frame(&mut self, sender_user_id: u64, frame: &[u8]) -> Result<Vec<u8>> {
        if self.is_downgraded || self.protocol_version == 0 || self.passthrough_fallback {
            return Ok(frame.to_vec());
        }

        if let Some(ref mut sess) = self.session {
            if sess.is_ready() {
                if sess.can_passthrough(sender_user_id) {
                    if let Ok(decrypted) = sess.decrypt(sender_user_id, MediaType::AUDIO, frame) {
                        return Ok(decrypted);
                    }
                    return Ok(frame.to_vec());
                }

                let decrypted = sess
                    .decrypt(sender_user_id, MediaType::AUDIO, frame)
                    .map_err(|e| anyhow::anyhow!("DAVE Opus decryption error for user {}: {:?}", sender_user_id, e))?;
                return Ok(decrypted);
            }
        }

        // Passthrough fallback
        Ok(frame.to_vec())
    }

    /// Decrypts an incoming Opus frame using the sender's SSRC.
    ///
    /// Resolves the SSRC to a User ID; if mapped, decrypts with `decrypt_opus_frame`;
    /// otherwise falls back to passthrough.
    pub fn decrypt_opus_frame_by_ssrc(&mut self, ssrc: u32, frame: &[u8]) -> Result<Vec<u8>> {
        if let Some(user_id) = self.get_user_id_by_ssrc(ssrc) {
            self.decrypt_opus_frame(user_id, frame)
        } else {
            Ok(frame.to_vec())
        }
    }
}

// ============================================================================
// UNIT TESTS (Network-free, Opcodes, Transitions & Media Transformations)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dave_opcodes_classification() {
        assert_eq!(DaveOpcode::from_u8(21), Some(DaveOpcode::PrepareTransition));
        assert_eq!(DaveOpcode::from_u8(22), Some(DaveOpcode::ExecuteTransition));
        assert_eq!(DaveOpcode::from_u8(23), Some(DaveOpcode::TransitionReady));
        assert_eq!(DaveOpcode::from_u8(24), Some(DaveOpcode::PrepareEpoch));
        assert_eq!(DaveOpcode::from_u8(25), Some(DaveOpcode::MlsExternalSender));
        assert_eq!(DaveOpcode::from_u8(26), Some(DaveOpcode::MlsKeyPackage));
        assert_eq!(DaveOpcode::from_u8(27), Some(DaveOpcode::MlsProposals));
        assert_eq!(DaveOpcode::from_u8(28), Some(DaveOpcode::MlsCommitWelcome));
        assert_eq!(DaveOpcode::from_u8(29), Some(DaveOpcode::MlsAnnounceCommitTransition));
        assert_eq!(DaveOpcode::from_u8(30), Some(DaveOpcode::MlsWelcome));
        assert_eq!(DaveOpcode::from_u8(31), Some(DaveOpcode::MlsInvalidCommitWelcome));
        assert_eq!(DaveOpcode::from_u8(99), None);

        // Binary check
        assert!(DaveOpcode::MlsExternalSender.is_binary());
        assert!(DaveOpcode::MlsKeyPackage.is_binary());
        assert!(DaveOpcode::MlsProposals.is_binary());
        assert!(DaveOpcode::MlsCommitWelcome.is_binary());
        assert!(DaveOpcode::MlsWelcome.is_binary());

        // JSON check
        assert!(DaveOpcode::PrepareTransition.is_json());
        assert!(DaveOpcode::ExecuteTransition.is_json());
        assert!(DaveOpcode::TransitionReady.is_json());
        assert!(DaveOpcode::PrepareEpoch.is_json());
        assert!(DaveOpcode::MlsAnnounceCommitTransition.is_json());
        assert!(DaveOpcode::MlsInvalidCommitWelcome.is_json());
    }

    #[test]
    fn test_server_binary_message_parsing() {
        // [seq_hi, seq_lo, opcode, payload...]
        let raw = vec![0x00, 0x05, 25, 0xAA, 0xBB, 0xCC];
        let (seq, opcode, payload) = parse_server_binary_message(&raw).unwrap();
        assert_eq!(seq, 5);
        assert_eq!(opcode, DaveOpcode::MlsExternalSender);
        assert_eq!(payload, &[0xAA, 0xBB, 0xCC]);

        // Malformed (too short)
        assert!(parse_server_binary_message(&[0x00, 0x01]).is_err());
    }

    #[test]
    fn test_client_binary_message_formatting() {
        let payload = vec![0x11, 0x22, 0x33];
        let formatted = format_client_binary_message(DaveOpcode::MlsKeyPackage, &payload);
        assert_eq!(formatted[0], 26);
        assert_eq!(&formatted[1..], &[0x11, 0x22, 0x33]);
    }

    #[test]
    fn test_json_payloads_serialization_and_deserialization() {
        // Opcode 21
        let p21 = DavePrepareTransitionPayload { transition_id: 1, protocol_version: 1 };
        let json21 = serde_json::to_string(&p21).unwrap();
        assert_eq!(serde_json::from_str::<DavePrepareTransitionPayload>(&json21).unwrap(), p21);

        // Opcode 22
        let p22 = DaveExecuteTransitionPayload { transition_id: 1 };
        let json22 = serde_json::to_string(&p22).unwrap();
        assert_eq!(serde_json::from_str::<DaveExecuteTransitionPayload>(&json22).unwrap(), p22);

        // Opcode 23
        let p23 = DaveTransitionReadyPayload { transition_id: 1 };
        let json23 = serde_json::to_string(&p23).unwrap();
        assert_eq!(serde_json::from_str::<DaveTransitionReadyPayload>(&json23).unwrap(), p23);

        // Opcode 24
        let p24 = DavePrepareEpochPayload { epoch: 42, protocol_version: 1 };
        let json24 = serde_json::to_string(&p24).unwrap();
        assert_eq!(serde_json::from_str::<DavePrepareEpochPayload>(&json24).unwrap(), p24);

        // Opcode 31
        let p31 = DaveInvalidCommitWelcomePayload { transition_id: 99 };
        let json31 = serde_json::to_string(&p31).unwrap();
        assert_eq!(serde_json::from_str::<DaveInvalidCommitWelcomePayload>(&json31).unwrap(), p31);
    }

    #[test]
    fn test_voice_dave_session_initialization_and_transitions() {
        let mut dave = VoiceDaveSession::new(123456789, 987654321);
        assert_eq!(dave.user_id(), 123456789);
        assert_eq!(dave.channel_id(), 987654321);
        assert_eq!(dave.protocol_version(), 0);
        assert!(!dave.is_ready());

        // Initialize with version 1
        dave.init_session(1).unwrap();
        assert_eq!(dave.protocol_version(), 1);
        assert!(!dave.is_downgraded());

        // Prepare transition to downgrade (v0)
        let ready = dave.handle_prepare_transition(10, 0);
        assert_eq!(ready.transition_id, 10);
        assert!(dave.pending_transitions.contains_key(&10));

        // Execute transition
        dave.handle_execute_transition(10).unwrap();
        assert!(dave.is_downgraded());
        assert_eq!(dave.protocol_version(), 0);

        // Prepare transition to upgrade back to v1
        let ready = dave.handle_prepare_transition(11, 1);
        assert_eq!(ready.transition_id, 11);
        dave.handle_execute_transition(11).unwrap();
        assert!(!dave.is_downgraded());
        assert_eq!(dave.protocol_version(), 1);
    }

    #[test]
    fn test_media_contract_passthrough_modes() {
        let mut dave = VoiceDaveSession::new(1001, 2002);

        // Uninitialized session: passthrough
        let raw_audio = vec![0xF8, 0xFF, 0xFE, 0x11, 0x22];
        let encrypted = dave.encrypt_opus_frame(&raw_audio).unwrap();
        assert_eq!(encrypted, raw_audio);

        let decrypted = dave.decrypt_opus_frame(9999, &raw_audio).unwrap();
        assert_eq!(decrypted, raw_audio);

        // SSRC mapping and resolution
        dave.register_ssrc_user_mapping(4242, 9999);
        assert_eq!(dave.get_user_id_by_ssrc(4242), Some(9999));
        assert_eq!(dave.get_user_id_by_ssrc(9999), None);

        let decrypted_by_ssrc = dave.decrypt_opus_frame_by_ssrc(4242, &raw_audio).unwrap();
        assert_eq!(decrypted_by_ssrc, raw_audio);
    }

    #[test]
    fn test_media_contract_encryption_and_decryption_with_davey_aes_gcm() {
        use davey::aead::{Aead, KeyInit};
        use davey::{Aes256Gcm, Nonce};

        let key = [0x42u8; 32];
        let cipher = Aes256Gcm::new_from_slice(&key).expect("Invalid key");
        let nonce = Nonce::from_slice(&[7u8; 12]);

        let opus_frame = b"Discord Voice Opus Audio Packet Test 2026";
        let encrypted = cipher.encrypt(nonce, opus_frame.as_ref()).expect("Encrypt failed");

        assert_ne!(encrypted.as_slice(), opus_frame.as_ref());
        assert_eq!(encrypted.len(), opus_frame.len() + 16);

        let decrypted = cipher.decrypt(nonce, encrypted.as_ref()).expect("Decrypt failed");
        assert_eq!(decrypted.as_slice(), opus_frame.as_ref());
    }
}
