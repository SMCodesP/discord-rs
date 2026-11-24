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
//! - Opcode 28: MLS Commit Welcome (Client -> Server, Binary)
//! - Opcode 29: MLS Announce Commit Transition (Server -> Client, Binary)
//! - Opcode 30: MLS Welcome (Binary)
//! - Opcode 31: MLS Invalid Commit Welcome (JSON)
//!
//! Exposes the media transformation contract:
//! - `encrypt_opus_frame(&mut self, frame: &[u8]) -> Result<Vec<u8>>`
//! - `decrypt_opus_frame(&mut self, sender_user_id: u64, frame: &[u8]) -> Result<Vec<u8>>`
//! - Fail-closed media while a negotiated DAVE session has no active MLS ratchet.
//! - SSRC <-> User ID mapping for media routing.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU16;

use davey::{
    DAVE_PROTOCOL_VERSION, DaveSession, MediaType, OPUS_SILENCE_PACKET, ProposalsOperationType,
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
                | Self::MlsAnnounceCommitTransition
                | Self::MlsWelcome
        )
    }

    /// Indicates whether the official wire representation is JSON.
    /// Opcode 29 is binary; legacy JSON commit payloads are not accepted.
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

/// Opcode 29: MLS Announce Commit Transition (Server -> Client Binary)
///
/// Kept as a serializable compatibility type for callers that inspected older
/// payloads. The Voice Gateway only accepts Opcode 29 as a binary WebSocket
/// message: `transition_id (u16 BE) || MLSMessage commit`.
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
    send_context_active: bool,
    ssrc_to_user_id: HashMap<u32, u64>,
    expected_user_ids: HashSet<u64>,
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
            send_context_active: false,
            ssrc_to_user_id: HashMap::new(),
            expected_user_ids: HashSet::from([user_id]),
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
        !self.is_downgraded
            && self.protocol_version > 0
            && self.send_context_active
            && self.session.as_ref().map(|s| s.is_ready()).unwrap_or(false)
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

        if version == 0 {
            self.protocol_version = 0;
            self.pending_transitions.clear();
            self.ssrc_to_user_id.clear();
            self.expected_user_ids.clear();
            self.expected_user_ids.insert(self.user_id);
            self.is_downgraded = true;
            self.send_context_active = false;
            self.session = None;
            return Ok(());
        }

        let non_zero_version =
            NonZeroU16::new(version).context("DAVE protocol version must be greater than 0")?;
        let dave_session = DaveSession::new(non_zero_version, self.user_id, self.channel_id, None)
            .map_err(|e| {
                self.protocol_version = version;
                self.pending_transitions.clear();
                self.ssrc_to_user_id.clear();
                self.expected_user_ids.clear();
                self.expected_user_ids.insert(self.user_id);
                self.is_downgraded = false;
                self.send_context_active = false;
                self.session = None;
                anyhow::anyhow!("Failed to initialize DaveSession: {:?}", e)
            })?;

        self.protocol_version = version;
        self.pending_transitions.clear();
        self.ssrc_to_user_id.clear();
        self.expected_user_ids.clear();
        self.expected_user_ids.insert(self.user_id);
        self.session = Some(dave_session);
        self.is_downgraded = false;
        self.send_context_active = false;
        Ok(())
    }

    fn ensure_session_preserving_participants(&mut self, version: u16) -> Result<()> {
        if self.session.is_some() {
            return Ok(());
        }
        let mappings = std::mem::take(&mut self.ssrc_to_user_id);
        let expected = self.expected_user_ids.clone();
        self.init_session(version)?;
        self.ssrc_to_user_id = mappings;
        self.expected_user_ids = expected;
        Ok(())
    }

    fn reinit_session_preserving_external_sender(&mut self, version: u16) -> Result<()> {
        let non_zero_version =
            NonZeroU16::new(version).context("DAVE protocol version must be greater than 0")?;
        let result = if let Some(session) = self.session.as_mut() {
            session
                .reinit(non_zero_version, self.user_id, self.channel_id, None)
                .map_err(|e| anyhow::anyhow!("Failed to reinitialize DaveSession: {:?}", e))
        } else {
            self.ensure_session_preserving_participants(version)
        };

        if let Err(err) = result {
            // davey may already have reset its internal group before failing.
            // Never leave wrapper metadata claiming that an old ratchet is usable.
            self.protocol_version = version;
            self.is_downgraded = false;
            self.send_context_active = false;
            self.session = None;
            return Err(err);
        }

        self.protocol_version = version;
        self.is_downgraded = false;
        self.send_context_active = false;
        Ok(())
    }

    // ========================================================================
    // SSRC <-> USER ID MAPPING
    // ========================================================================

    /// Registers an SSRC to User ID mapping.
    pub fn register_ssrc_user_mapping(&mut self, ssrc: u32, user_id: u64) {
        // Speaking só relaciona SSRC e User ID; não prova que o usuário entrou
        // após o último Client Disconnect. Expected users vêm de 11/13.
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

    /// Registers um participante anunciado pelo opcode 11 Clients Connect.
    pub fn register_user_connected(&mut self, user_id: u64) {
        self.expected_user_ids.insert(user_id);
    }

    /// Remove um participante anunciado pelo opcode 13 e todos os seus SSRCs.
    pub fn register_user_disconnected(&mut self, user_id: u64) {
        self.expected_user_ids.remove(&user_id);
        self.ssrc_to_user_id
            .retain(|_, mapped_uid| *mapped_uid != user_id);
    }

    /// IDs que o gateway confirmou como participantes atuais da chamada.
    pub fn expected_user_ids(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self.expected_user_ids.iter().copied().collect();
        ids.sort_unstable();
        ids
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
            self.ensure_session_preserving_participants(self.protocol_version)?;
        }

        let result = (|| {
            let session = self
                .session
                .as_mut()
                .context("DAVE session not initialized")?;
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
        })();

        if let Err(err) = result {
            // set_external_sender can delete the pending group before rejecting
            // malformed bytes. Drop the whole context so no stale ratchet remains.
            self.send_context_active = false;
            self.is_downgraded = false;
            self.session = None;
            return Err(err);
        }
        result
    }

    /// Creates and returns a new serialized MLS Key Package for Opcode 26.
    pub fn create_key_package(&mut self) -> Result<Vec<u8>> {
        let session = self
            .session
            .as_mut()
            .context("DAVE session not initialized")?;
        session
            .create_key_package()
            .map_err(|e| anyhow::anyhow!("Failed to create MLS key package: {:?}", e))
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
        let op = match operation_type {
            0 => ProposalsOperationType::APPEND,
            1 => ProposalsOperationType::REVOKE,
            other => bail!("Tipo de operação MLS proposals inválido: {other}"),
        };

        // The whitepaper requires pending members without a local MLS group to
        // ignore proposals until the external sender creates that group.
        let has_local_group = self
            .session
            .as_ref()
            .and_then(|session| session.group())
            .is_some();
        if !has_local_group {
            return Ok(None);
        }

        // O DAVE exige validar Add proposals contra Clients Connect/Disconnect.
        // O argumento opcional é reservado para testes/consumidores que já validaram
        // a lista fora desta sessão; em produção usamos o conjunto mantido aqui.
        let expected_ids = self.expected_user_ids();
        let expected_ids = expected_user_ids.unwrap_or(&expected_ids);
        let session = self
            .session
            .as_mut()
            .context("DAVE session not initialized")?;
        let commit_welcome_opt = session
            .process_proposals(op, proposals_data, Some(expected_ids))
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

    fn reset_mls_for_readd(&mut self) -> Result<()> {
        let version = NonZeroU16::new(self.protocol_version)
            .context("Não é possível recuperar DAVE sem uma versão de protocolo ativa")?;
        let result = match self.session.as_mut() {
            Some(session) => session
                .reinit(version, self.user_id, self.channel_id, None)
                .map_err(|e| anyhow::anyhow!("Failed to reset MLS state for re-add: {:?}", e)),
            None => Err(anyhow::anyhow!("DAVE session not initialized")),
        };

        self.send_context_active = false;
        if let Err(err) = result {
            self.session = None;
            self.is_downgraded = false;
            return Err(err);
        }
        Ok(())
    }

    /// Handles Opcode 29 (MLS Announce Commit Transition).
    ///
    /// Processes an announced commit. If processing fails, resets the MLS state
    /// as required by DAVE and returns Opcode 31 so the caller can request re-add
    /// and send a fresh Opcode 26 Key Package.
    pub fn handle_announce_commit(
        &mut self,
        transition_id: u16,
        commit: &[u8],
    ) -> Result<Option<DaveInvalidCommitWelcomePayload>> {
        self.pending_transitions
            .insert(transition_id, self.protocol_version);
        let result = match self.session.as_mut() {
            Some(session) => session
                .process_commit(commit)
                .map_err(|e| anyhow::anyhow!("Failed to process MLS commit: {:?}", e)),
            None => Err(anyhow::anyhow!("DAVE session not initialized")),
        };

        match result {
            Ok(_) => {
                // davey já atualizou o encryptor; bloqueamos o envio até o
                // Opcode 22 confirmar a execução da transição.
                self.send_context_active = false;
                log::info!(
                    "Successfully processed DAVE commit transition id: {}",
                    transition_id
                );
                Ok(None)
            }
            Err(e) => {
                log::warn!(
                    "Failed to process MLS commit transition id {}: {:?}. Requesting re-add.",
                    transition_id,
                    e
                );
                self.pending_transitions.remove(&transition_id);
                self.reset_mls_for_readd()?;
                Ok(Some(DaveInvalidCommitWelcomePayload { transition_id }))
            }
        }
    }

    /// Handles Opcode 30 (MLS Welcome).
    ///
    /// Processes a Welcome message when added to the MLS group. If processing fails,
    /// resets the MLS state and returns Opcode 31 so the caller can request re-add
    /// and send a fresh Opcode 26 Key Package.
    pub fn handle_welcome(
        &mut self,
        transition_id: u16,
        welcome: &[u8],
    ) -> Result<Option<DaveInvalidCommitWelcomePayload>> {
        self.pending_transitions
            .insert(transition_id, self.protocol_version);
        let result = match self.session.as_mut() {
            Some(session) => session
                .process_welcome(welcome)
                .map_err(|e| anyhow::anyhow!("Failed to process MLS Welcome: {:?}", e)),
            None => Err(anyhow::anyhow!("DAVE session not initialized")),
        };

        match result {
            Ok(_) => {
                self.send_context_active = false;
                log::info!(
                    "Successfully processed DAVE Welcome message id: {}",
                    transition_id
                );
                Ok(None)
            }
            Err(e) => {
                log::warn!(
                    "Failed to process MLS Welcome message id {}: {:?}. Requesting re-add.",
                    transition_id,
                    e
                );
                self.pending_transitions.remove(&transition_id);
                self.reset_mls_for_readd()?;
                Ok(Some(DaveInvalidCommitWelcomePayload { transition_id }))
            }
        }
    }

    /// Handles Opcode 21 (DAVE Prepare Transition).
    ///
    /// Records the pending transition and returns Opcode 23. Receivers enter
    /// passthrough immediately for a downgrade, while senders keep the old
    /// context until Opcode 22. Transition ID 0 is executed by the caller
    /// immediately after this method, as required by the sole-member reset.
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
        self.pending_transitions
            .insert(transition_id, protocol_version);
        if protocol_version == 0 {
            if let Some(ref mut sess) = self.session {
                sess.set_passthrough_mode(true, None);
            }
        }
        DaveTransitionReadyPayload { transition_id }
    }

    pub fn has_pending_transition(&self, transition_id: u16) -> bool {
        self.pending_transitions.contains_key(&transition_id)
    }

    /// Handles Opcode 22 (DAVE Execute Transition).
    ///
    /// Executes the pending transition, transitioning between MLS and passthrough modes.
    pub fn handle_execute_transition(&mut self, transition_id: u16) -> Result<()> {
        log::info!("Executing DAVE transition id: {}", transition_id);
        let Some(target_version) = self.pending_transitions.remove(&transition_id) else {
            // Em transport-only não há contexto MLS para executar; tolerar o
            // notification de mock/legado. Com DAVE ativo, é um erro de estado.
            if self.protocol_version == 0 && self.session.is_none() {
                return Ok(());
            }
            bail!("DAVE Execute Transition sem preparo para id {transition_id}");
        };

        if target_version == 0 {
            self.is_downgraded = true;
            self.protocol_version = 0;
            self.send_context_active = false;
            // Mantemos os ratchets do davey por 10 s para decifrar quadros em
            // trânsito. O passthrough indefinido só repassa frames não-E2EE.
            if let Some(ref mut sess) = self.session {
                sess.set_passthrough_mode(true, None);
            }
            log::warn!("DAVE protocol downgraded to version 0 (passthrough active)");
            return Ok(());
        }

        if self.session.is_none() {
            self.ensure_session_preserving_participants(target_version)?;
        } else if self.session.as_ref().map(|s| s.protocol_version().get()) != Some(target_version)
        {
            bail!("Mudança de versão DAVE requer Prepare Epoch antes de Execute Transition");
        }

        self.is_downgraded = false;
        self.protocol_version = target_version;
        self.send_context_active = true;
        if let Some(ref mut sess) = self.session {
            // Após o execute, novos frames não-E2EE ainda podem chegar durante
            // a janela de transição; depois dela, somente frames DAVE são aceitos.
            sess.set_passthrough_mode(false, Some(10));
        }
        log::info!(
            "DAVE protocol upgraded to version {} (10s passthrough grace)",
            target_version
        );
        Ok(())
    }

    /// Handles Opcode 24 (DAVE Prepare Epoch).
    ///
    /// `epoch = 1` recreates the MLS group and returns a framed Opcode 26 Key
    /// Package. The caller must send it before processing new proposals.
    pub fn handle_prepare_epoch(
        &mut self,
        epoch: u64,
        protocol_version: u16,
    ) -> Result<Option<Vec<u8>>> {
        log::debug!(
            "DAVE Prepare Epoch: epoch {}, version {}",
            epoch,
            protocol_version
        );
        if protocol_version == 0 {
            bail!("Prepare Epoch recebeu protocol_version 0; downgrades usam Opcode 21");
        }

        if epoch == 1 {
            // davey::reinit preserva o External Sender e recria o pending group;
            // init_session criaria uma sessão sem grupo e as propostas morreriam.
            self.reinit_session_preserving_external_sender(protocol_version)?;
            let key_package = self.create_key_package()?;
            return Ok(Some(format_client_binary_message(
                DaveOpcode::MlsKeyPackage,
                &key_package,
            )));
        }

        if self.session.is_none() {
            bail!("Prepare Epoch > 1 recebido antes da criação do grupo MLS");
        }
        if self.session.as_ref().map(|s| s.protocol_version().get()) != Some(protocol_version) {
            bail!("Mudança de versão DAVE em epoch > 1 não é suportada pelo davey 0.1.4");
        }
        Ok(None)
    }

    // ========================================================================
    // MEDIA TRANSFORMATION CONTRACT (Audio -> DAVE -> RTP)
    // ========================================================================

    /// Encrypts an outgoing Opus frame using DAVE E2EE.
    ///
    /// The canonical three-byte silence frame always passes through. Raw audio
    /// is allowed only for transport-only sessions (version 0/downgrade). A
    /// negotiated DAVE session fails closed until Opcode 22 activates the new
    /// sender context and the MLS ratchet is ready.
    pub fn encrypt_opus_frame(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        if frame == OPUS_SILENCE_PACKET || self.protocol_version == 0 || self.is_downgraded {
            return Ok(frame.to_vec());
        }
        if !self.is_ready() {
            bail!("DAVE negociado, mas o MLS ainda não está pronto para cifrar áudio");
        }

        let session = self
            .session
            .as_mut()
            .context("DAVE negociado sem sessão MLS ativa")?;
        session
            .encrypt_opus(frame)
            .map(|enc| enc.into_owned())
            .map_err(|e| anyhow::anyhow!("DAVE Opus encryption error: {:?}", e))
    }

    /// Decrypts an incoming Opus frame from a remote participant.
    ///
    /// The davey decryptor implements the protocol frame check, transition grace
    /// and retention of previous epochs. A failed E2EE decryption is never
    /// replaced with raw bytes, so ciphertext cannot reach the Opus decoder.
    pub fn decrypt_opus_frame(&mut self, sender_user_id: u64, frame: &[u8]) -> Result<Vec<u8>> {
        if frame == OPUS_SILENCE_PACKET {
            return Ok(frame.to_vec());
        }

        // During downgrade/transition the old ratchets may still decrypt frames
        // in flight, and davey decides whether a non-protocol frame may pass.
        if self.session.as_ref().map(|session| session.is_ready()).unwrap_or(false) {
            return self
                .session
                .as_mut()
                .context("DAVE session disappeared during decryption")?
                .decrypt(sender_user_id, MediaType::AUDIO, frame)
                .map_err(|e| {
                    anyhow::anyhow!(
                        "DAVE Opus decryption error for user {}: {:?}",
                        sender_user_id,
                        e
                    )
                });
        }

        if self.protocol_version == 0 || self.is_downgraded {
            if frame.ends_with(&[0xFA, 0xFA]) {
                bail!("Frame DAVE recebido sem ratchet ativo para decifração");
            }
            return Ok(frame.to_vec());
        }

        bail!("DAVE negociado, mas o MLS ainda não está pronto para decifrar áudio")
    }

    /// Fail-closed alias retained for callers that explicitly selected the
    /// production media contract. The default methods now have these semantics.
    pub fn encrypt_opus_frame_strict(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        self.encrypt_opus_frame(frame)
    }

    /// Fail-closed alias retained for callers that explicitly selected the
    /// production media contract.
    pub fn decrypt_opus_frame_strict(
        &mut self,
        sender_user_id: u64,
        frame: &[u8],
    ) -> Result<Vec<u8>> {
        self.decrypt_opus_frame(sender_user_id, frame)
    }

    /// Decrypts an incoming Opus frame using the sender's SSRC.
    ///
    /// Canonical silence is accepted before SSRC resolution. Every other frame
    /// from an unknown SSRC is dropped whenever DAVE requires a sender key.
    pub fn decrypt_opus_frame_by_ssrc(&mut self, ssrc: u32, frame: &[u8]) -> Result<Vec<u8>> {
        if frame == OPUS_SILENCE_PACKET {
            return Ok(frame.to_vec());
        }
        if let Some(user_id) = self.get_user_id_by_ssrc(ssrc) {
            return self.decrypt_opus_frame(user_id, frame);
        }

        if self.protocol_version == 0 || self.is_downgraded {
            if frame.ends_with(&[0xFA, 0xFA]) {
                bail!("Frame DAVE com SSRC {ssrc} sem mapeamento User ID");
            }
            return Ok(frame.to_vec());
        }
        bail!("SSRC {ssrc} sem mapeamento User ID para decifração DAVE")
    }

    pub fn decrypt_opus_frame_by_ssrc_strict(
        &mut self,
        ssrc: u32,
        frame: &[u8],
    ) -> Result<Vec<u8>> {
        self.decrypt_opus_frame_by_ssrc(ssrc, frame)
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
        assert_eq!(
            DaveOpcode::from_u8(29),
            Some(DaveOpcode::MlsAnnounceCommitTransition)
        );
        assert_eq!(DaveOpcode::from_u8(30), Some(DaveOpcode::MlsWelcome));
        assert_eq!(
            DaveOpcode::from_u8(31),
            Some(DaveOpcode::MlsInvalidCommitWelcome)
        );
        assert_eq!(DaveOpcode::from_u8(99), None);

        // Binary check
        assert!(DaveOpcode::MlsExternalSender.is_binary());
        assert!(DaveOpcode::MlsKeyPackage.is_binary());
        assert!(DaveOpcode::MlsProposals.is_binary());
        assert!(DaveOpcode::MlsCommitWelcome.is_binary());
        assert!(DaveOpcode::MlsAnnounceCommitTransition.is_binary());
        assert!(DaveOpcode::MlsWelcome.is_binary());

        // JSON check
        assert!(DaveOpcode::PrepareTransition.is_json());
        assert!(DaveOpcode::ExecuteTransition.is_json());
        assert!(DaveOpcode::TransitionReady.is_json());
        assert!(DaveOpcode::PrepareEpoch.is_json());
        // O formato oficial é binário; JSON não é uma representação aceita.
        assert!(!DaveOpcode::MlsAnnounceCommitTransition.is_json());
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
        let p21 = DavePrepareTransitionPayload {
            transition_id: 1,
            protocol_version: 1,
        };
        let json21 = serde_json::to_string(&p21).unwrap();
        assert_eq!(
            serde_json::from_str::<DavePrepareTransitionPayload>(&json21).unwrap(),
            p21
        );

        // Opcode 22
        let p22 = DaveExecuteTransitionPayload { transition_id: 1 };
        let json22 = serde_json::to_string(&p22).unwrap();
        assert_eq!(
            serde_json::from_str::<DaveExecuteTransitionPayload>(&json22).unwrap(),
            p22
        );

        // Opcode 23
        let p23 = DaveTransitionReadyPayload { transition_id: 1 };
        let json23 = serde_json::to_string(&p23).unwrap();
        assert_eq!(
            serde_json::from_str::<DaveTransitionReadyPayload>(&json23).unwrap(),
            p23
        );

        // Opcode 24
        let p24 = DavePrepareEpochPayload {
            epoch: 42,
            protocol_version: 1,
        };
        let json24 = serde_json::to_string(&p24).unwrap();
        assert_eq!(
            serde_json::from_str::<DavePrepareEpochPayload>(&json24).unwrap(),
            p24
        );

        // Opcode 31
        let p31 = DaveInvalidCommitWelcomePayload { transition_id: 99 };
        let json31 = serde_json::to_string(&p31).unwrap();
        assert_eq!(
            serde_json::from_str::<DaveInvalidCommitWelcomePayload>(&json31).unwrap(),
            p31
        );
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
    fn test_two_voice_dave_sessions_offline_mls_group_constraints() {
        // User A (user_id: 111111) and User B (user_id: 222222) in the same channel (channel_id: 999999)
        let channel_id = 999999;
        let mut dave_a = VoiceDaveSession::new(111111, channel_id);
        let mut dave_b = VoiceDaveSession::new(222222, channel_id);

        // 1. Both sessions initialize with DAVE Protocol Version 1
        dave_a.init_session(1).unwrap();
        dave_b.init_session(1).unwrap();

        assert_eq!(dave_a.protocol_version(), 1);
        assert_eq!(dave_b.protocol_version(), 1);
        assert!(!dave_a.is_ready());
        assert!(!dave_b.is_ready());

        // 2. Both sessions can generate valid MLS KeyPackages (Opcode 26) independently
        let key_package_a = dave_a
            .create_key_package()
            .expect("User A KeyPackage generation failed");
        let key_package_b = dave_b
            .create_key_package()
            .expect("User B KeyPackage generation failed");

        assert!(!key_package_a.is_empty());
        assert!(!key_package_b.is_empty());
        assert_ne!(key_package_a, key_package_b);

        // 3. Proposals received before a local group exists are ignored, as
        // required by the whitepaper's Initial Group Creation section.
        let proposals_result = dave_a.handle_proposals(0, &key_package_b, None);
        assert!(proposals_result.unwrap().is_none());

        // 4. Negotiated DAVE media fails closed until the MLS ratchet is active.
        let sample_opus = vec![0xF8, 0xFF, 0xFE, 0xAA, 0xBB, 0xCC];
        assert!(dave_a.encrypt_opus_frame(&sample_opus).is_err());
        assert!(dave_b.decrypt_opus_frame(dave_a.user_id(), &sample_opus).is_err());
    }

    fn test_secret_external_sender() -> (Vec<u8>, davey::SigningKeyPair) {
        let key = davey::SigningKeyPair::generate();
        let identity = 0x1234_5678_9ABC_DEF0u64.to_be_bytes();
        let mut package = Vec::new();
        // SignaturePublicKey is opaque<V>; 65 bytes uses the two-byte VLBytes form.
        package.extend_from_slice(&[0x40, key.public.len() as u8]);
        package.extend_from_slice(&key.public);
        // CredentialType::Basic followed by opaque<V> identity.
        package.extend_from_slice(&[0x00, 0x01, identity.len() as u8]);
        package.extend_from_slice(&identity);
        (package, key)
    }

    #[test]
    fn test_epoch_one_reset_preserves_external_sender_and_local_group() {
        let (external_sender, _) = test_secret_external_sender();
        let mut dave = VoiceDaveSession::new(100, 200);
        dave.init_session(1).unwrap();
        let key_package = dave.handle_external_sender(&external_sender).unwrap();
        assert_eq!(key_package[0], DaveOpcode::MlsKeyPackage as u8);
        assert!(dave.session.as_ref().unwrap().group().is_some());

        let reset_key_package = dave.handle_prepare_epoch(1, 1).unwrap().unwrap();
        assert_eq!(reset_key_package[0], DaveOpcode::MlsKeyPackage as u8);
        assert!(dave.session.as_ref().unwrap().group().is_some());
        assert!(!dave.is_ready());
    }

    #[test]
    fn test_invalid_external_sender_leaves_no_stale_ready_context() {
        let mut dave = VoiceDaveSession::new(100, 200);
        dave.init_session(1).unwrap();
        assert!(dave.handle_external_sender(&[0xAA, 0xBB]).is_err());
        assert!(dave.session.is_none());
        assert!(!dave.is_ready());
        assert!(dave.encrypt_opus_frame(b"must not pass through").is_err());
    }

    #[test]
    fn test_unsupported_reinitialization_fails_closed() {
        let mut dave = VoiceDaveSession::new(100, 200);
        dave.init_session(1).unwrap();
        assert!(dave.init_session(2).is_err());
        assert!(dave.session.is_none());
        assert!(!dave.is_ready());
        assert!(dave.encrypt_opus_frame(b"must not pass through").is_err());
    }

    #[test]
    fn test_proposals_without_local_group_are_ignored() {
        let mut dave = VoiceDaveSession::new(100, 200);
        dave.init_session(1).unwrap();
        assert!(dave.handle_proposals(0, &[0xAA], None).unwrap().is_none());
    }

    #[test]
    fn test_client_connect_disconnect_updates_expected_users_and_ssrcs() {
        let mut dave = VoiceDaveSession::new(100, 200);
        dave.register_user_connected(300);
        dave.register_ssrc_user_mapping(10, 300);
        dave.register_ssrc_user_mapping(11, 300);
        dave.register_ssrc_user_mapping(12, 400);
        assert_eq!(dave.expected_user_ids(), vec![100, 300]);

        dave.register_user_disconnected(300);
        assert_eq!(dave.expected_user_ids(), vec![100]);
        assert_eq!(dave.get_user_id_by_ssrc(10), None);
        assert_eq!(dave.get_user_id_by_ssrc(11), None);
        assert_eq!(dave.get_user_id_by_ssrc(12), Some(400));
    }

    #[test]
    fn test_prepare_epoch_one_recreates_group_and_returns_key_package() {
        let mut dave = VoiceDaveSession::new(100, 200);
        dave.init_session(1).unwrap();
        let frame = dave.handle_prepare_epoch(1, 1).unwrap().unwrap();
        assert_eq!(frame[0], DaveOpcode::MlsKeyPackage as u8);
        assert!(frame.len() > 1);
        assert!(!dave.is_ready());
        assert!(dave.handle_prepare_epoch(2, 0).is_err());
    }

    #[test]
    fn test_invalid_welcome_resets_mls_and_allows_fresh_key_package() {
        let mut dave = VoiceDaveSession::new(100, 200);
        dave.init_session(1).unwrap();
        dave.register_user_connected(300);

        let invalid = dave.handle_welcome(9, &[0xAA, 0xBB]).unwrap().unwrap();
        assert_eq!(invalid.transition_id, 9);
        assert!(!dave.is_ready());
        assert_eq!(dave.expected_user_ids(), vec![100, 300]);
        assert!(!dave.create_key_package().unwrap().is_empty());
    }

    #[test]
    fn test_execute_transition_requires_matching_prepare() {
        let mut dave = VoiceDaveSession::new(100, 200);
        dave.init_session(1).unwrap();
        assert!(dave.handle_execute_transition(77).is_err());
    }

    #[test]
    fn test_invalid_proposal_operation_is_rejected() {
        let mut dave = VoiceDaveSession::new(100, 200);
        dave.init_session(1).unwrap();
        assert!(dave.handle_proposals(2, &[], None).is_err());
    }

    #[test]
    fn test_exact_silence_frame_is_the_only_unconditional_media_passthrough() {
        let mut dave = VoiceDaveSession::new(100, 200);
        let silence = OPUS_SILENCE_PACKET.to_vec();
        assert_eq!(dave.encrypt_opus_frame(&silence).unwrap(), silence);
        assert_eq!(dave.decrypt_opus_frame(300, &silence).unwrap(), silence);
        assert_eq!(
            dave.decrypt_opus_frame_by_ssrc(999, &silence).unwrap(),
            silence
        );
    }

    #[test]
    fn test_production_media_fails_closed_until_mls_is_ready() {
        let mut dave = VoiceDaveSession::new(100, 200);
        dave.init_session(1).unwrap();
        let frame = b"not silence".to_vec();
        assert!(dave.encrypt_opus_frame(&frame).is_err());
        assert!(dave.decrypt_opus_frame(300, &frame).is_err());
        assert!(dave.decrypt_opus_frame_by_ssrc(999, &frame).is_err());
        assert!(dave.encrypt_opus_frame_strict(&frame).is_err());
        assert!(dave.decrypt_opus_frame_strict(300, &frame).is_err());
        assert!(dave.decrypt_opus_frame_by_ssrc_strict(999, &frame).is_err());

        let silence = OPUS_SILENCE_PACKET.to_vec();
        assert_eq!(dave.encrypt_opus_frame_strict(&silence).unwrap(), silence);
        assert_eq!(
            dave.decrypt_opus_frame_by_ssrc_strict(999, &silence)
                .unwrap(),
            silence
        );

        let mut prefixed = silence.clone();
        prefixed.push(0);
        assert!(dave.encrypt_opus_frame_strict(&prefixed).is_err());
    }

    #[test]
    fn test_version_zero_allows_production_passthrough() {
        let mut dave = VoiceDaveSession::new(100, 200);
        let frame = b"transport-only opus".to_vec();
        assert_eq!(dave.encrypt_opus_frame_strict(&frame).unwrap(), frame);
        assert_eq!(dave.decrypt_opus_frame_strict(300, &frame).unwrap(), frame);
    }
}
