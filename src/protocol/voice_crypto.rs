//! Discord Voice Transport Cryptography (Fase 8.2).
//!
//! Implementação dos modos oficiais de criptografia de transporte de voz do Discord:
//! - `aead_aes256_gcm_rtpsize` (Preferido)
//! - `aead_xchacha20_poly1305_rtpsize` (Obrigatório / Fallback universal)
//!
//! Estrutura do pacote criptografado:
//! `[ Cabeçalho RTP / AAD ] + [ Carga útil cifrada + Tag AEAD (16 bytes) ] + [ Nonce sufixo de 32 bits (4 bytes) ]`
//!
//! Nos modos `_rtpsize`:
//! - O trecho não cifrado (AAD) compreende o cabeçalho base RTP (12 bytes), quaisquer CSRCs (CC * 4 bytes)
//!   e o preâmbulo da extensão de cabeçalho (4 bytes, se o bit X estiver ativo).
//! - A carga útil da extensão (se presente) é cifrada juntamente com o áudio Opus e removida na decifração.

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, bail, Result};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};

/// Modos de criptografia de transporte suportados pelo Discord Voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionMode {
    AeadAes256GcmRtpsize,
    AeadXChaCha20Poly1305Rtpsize,
}

impl EncryptionMode {
    pub fn from_str_name(mode: &str) -> Result<Self> {
        match mode.trim().to_ascii_lowercase().as_str() {
            "aead_aes256_gcm_rtpsize" => Ok(Self::AeadAes256GcmRtpsize),
            "aead_xchacha20_poly1305_rtpsize" => Ok(Self::AeadXChaCha20Poly1305Rtpsize),
            other => bail!("Modo de criptografia não suportado pelo Discord Voice: '{other}'"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AeadAes256GcmRtpsize => "aead_aes256_gcm_rtpsize",
            Self::AeadXChaCha20Poly1305Rtpsize => "aead_xchacha20_poly1305_rtpsize",
        }
    }
}

/// Metadados e limites de cabeçalho RTP extraídos para AAD e decifração.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtpAadInfo {
    /// Comprimento total da porção não cifrada usada como AAD (12 + 4*CC + 4 se extensão).
    pub aad_len: usize,
    /// Comprimento em bytes da carga da extensão (se houver) que deve ser descartada após decifrar.
    pub ext_payload_bytes: usize,
    /// Identificador SSRC da fonte de transmissão.
    pub ssrc: u32,
    /// Sequência do pacote RTP.
    pub sequence: u16,
    /// Timestamp de amostragem RTP.
    pub timestamp: u32,
    /// Payload type (ex: 0x78 para Opus).
    pub payload_type: u8,
    /// Indicador se há extensão de cabeçalho.
    pub has_extension: bool,
}

/// Parser de cabeçalho RTP estritamente compatível com RFC 3550 e RFC 3711 (SRTP / Discord rtpsize).
pub fn parse_rtp_aad_and_ext(buf: &[u8]) -> Result<RtpAadInfo> {
    if buf.len() < 12 {
        bail!("Pacote RTP menor que o cabeçalho base de 12 bytes (tamanho: {})", buf.len());
    }

    let b0 = buf[0];
    let version = (b0 >> 6) & 0x03;
    if version != 2 {
        bail!("Versão de cabeçalho RTP inválida: esperada 2 (0x80), encontrada {}", version);
    }

    let has_extension = (b0 & 0x10) != 0;
    let cc = (b0 & 0x0F) as usize;
    let base_header_len = 12 + (4 * cc);

    if buf.len() < base_header_len {
        bail!(
            "Pacote RTP menor do que a lista de CSRCs declarada (requer {}, recebido {})",
            base_header_len,
            buf.len()
        );
    }

    let payload_type = buf[1] & 0x7F;
    let sequence = u16::from_be_bytes([buf[2], buf[3]]);
    let timestamp = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let ssrc = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);

    let mut aad_len = base_header_len;
    let mut ext_payload_bytes = 0;

    if has_extension {
        if buf.len() < base_header_len + 4 {
            bail!("Pacote RTP truncado antes do preâmbulo da extensão de 4 bytes");
        }
        // Preâmbulo da extensão: [profile: 2 bytes, length_in_32bit_words: 2 bytes]
        let ext_words = u16::from_be_bytes([buf[base_header_len + 2], buf[base_header_len + 3]]) as usize;
        ext_payload_bytes = ext_words * 4;
        // No modo rtpsize do Discord/SRTP, o preâmbulo de 4 bytes é incluído no AAD não cifrado.
        aad_len = base_header_len + 4;
    }

    Ok(RtpAadInfo {
        aad_len,
        ext_payload_bytes,
        ssrc,
        sequence,
        timestamp,
        payload_type,
        has_extension,
    })
}

enum CipherEngine {
    Aes256Gcm(Aes256Gcm),
    XChaCha20Poly1305(XChaCha20Poly1305),
}

/// Manipulador de criptografia e integridade de pacotes RTP para chamadas de voz do Discord.
pub struct RtpCrypto {
    mode: EncryptionMode,
    engine: CipherEngine,
    nonce_counter: u32,
}

impl RtpCrypto {
    /// Inicializa um novo motor criptográfico a partir do modo negociado e da secret key de 32 bytes
    /// recebida no Session Description (Opcode 4).
    pub fn new(mode: &str, secret_key: &[u8]) -> Result<Self> {
        if secret_key.len() != 32 {
            bail!(
                "Tamanho de secret_key inválido: esperado 32 bytes (256 bits), recebido {}",
                secret_key.len()
            );
        }

        let enc_mode = EncryptionMode::from_str_name(mode)?;
        let engine = match enc_mode {
            EncryptionMode::AeadAes256GcmRtpsize => {
                let cipher = Aes256Gcm::new_from_slice(secret_key)
                    .map_err(|e| anyhow!("Falha ao inicializar AES-256-GCM: {e}"))?;
                CipherEngine::Aes256Gcm(cipher)
            }
            EncryptionMode::AeadXChaCha20Poly1305Rtpsize => {
                let cipher = XChaCha20Poly1305::new_from_slice(secret_key)
                    .map_err(|e| anyhow!("Falha ao inicializar XChaCha20-Poly1305: {e}"))?;
                CipherEngine::XChaCha20Poly1305(cipher)
            }
        };

        Ok(Self {
            mode: enc_mode,
            engine,
            nonce_counter: 0,
        })
    }

    /// Retorna o modo de criptografia ativo.
    pub fn mode(&self) -> EncryptionMode {
        self.mode
    }

    /// Retorna o valor atual do contador incremental de nonce.
    pub fn current_nonce(&self) -> u32 {
        self.nonce_counter
    }

    /// Define manualmente o contador de nonce (útil para testes ou sincronização).
    pub fn set_nonce_counter(&mut self, val: u32) {
        self.nonce_counter = val;
    }

    fn next_nonce_bytes(&mut self) -> (u32, [u8; 4]) {
        let val = self.nonce_counter;
        self.nonce_counter = self.nonce_counter.wrapping_add(1);
        (val, val.to_be_bytes())
    }

    fn make_aes_nonce(nonce_bytes: [u8; 4]) -> [u8; 12] {
        let mut n = [0u8; 12];
        n[0..4].copy_from_slice(&nonce_bytes);
        n
    }

    fn make_xchacha_nonce(nonce_bytes: [u8; 4]) -> [u8; 24] {
        let mut n = [0u8; 24];
        n[0..4].copy_from_slice(&nonce_bytes);
        n
    }

    /// Criptografa uma carga útil Opus dado um cabeçalho RTP (AAD).
    ///
    /// Anexa o nonce de 32 bits (4 bytes) ao final do pacote.
    /// Retorna o pacote completo pronto para envio UDP: `[ header ] + [ ciphertext + tag ] + [ nonce ]`.
    pub fn encrypt(&mut self, header: &[u8], payload: &[u8]) -> Result<Vec<u8>> {
        let (_, nonce_bytes) = self.next_nonce_bytes();

        let ciphertext = match &self.engine {
            CipherEngine::Aes256Gcm(c) => {
                let n = Self::make_aes_nonce(nonce_bytes);
                c.encrypt(
                    Nonce::from_slice(&n),
                    Payload {
                        msg: payload,
                        aad: header,
                    },
                )
                .map_err(|e| anyhow!("Erro ao cifrar via AES-256-GCM: {e}"))?
            }
            CipherEngine::XChaCha20Poly1305(c) => {
                let n = Self::make_xchacha_nonce(nonce_bytes);
                c.encrypt(
                    XNonce::from_slice(&n),
                    Payload {
                        msg: payload,
                        aad: header,
                    },
                )
                .map_err(|e| anyhow!("Erro ao cifrar via XChaCha20-Poly1305: {e}"))?
            }
        };

        let mut packet = Vec::with_capacity(header.len() + ciphertext.len() + 4);
        packet.extend_from_slice(header);
        packet.extend_from_slice(&ciphertext);
        packet.extend_from_slice(&nonce_bytes);
        Ok(packet)
    }

    /// Criptografa um pacote RTP não criptografado completo contendo cabeçalho e áudio Opus.
    pub fn encrypt_packet(&mut self, unencrypted_packet: &[u8]) -> Result<Vec<u8>> {
        let info = parse_rtp_aad_and_ext(unencrypted_packet)?;
        let aad = &unencrypted_packet[..info.aad_len];
        let payload = &unencrypted_packet[info.aad_len..];
        self.encrypt(aad, payload)
    }

    /// Decifra um pacote de voz recebido sobre UDP e retorna a carga de áudio Opus.
    ///
    /// Valida o AAD, o nonce anexado de 4 bytes e a tag de autenticação de 16 bytes.
    /// Em caso de pacote corrompido ou adulterado, retorna erro de autenticação.
    pub fn decrypt(&self, packet: &[u8]) -> Result<Vec<u8>> {
        let (_, opus) = self.decrypt_packet(packet)?;
        Ok(opus)
    }

    /// Decifra um pacote de voz recebido sobre UDP e retorna uma tupla com o SSRC de origem e o quadro Opus.
    pub fn decrypt_packet(&self, packet: &[u8]) -> Result<(u32, Vec<u8>)> {
        let (info, opus) = self.decrypt_full(packet)?;
        Ok((info.ssrc, opus))
    }

    /// Decifra o pacote e retorna todos os metadados do cabeçalho RTP juntamente com o áudio decifrado.
    pub fn decrypt_full(&self, packet: &[u8]) -> Result<(RtpAadInfo, Vec<u8>)> {
        let info = parse_rtp_aad_and_ext(packet)?;

        // Tamanho mínimo: AAD + 16 bytes (tag AEAD) + 4 bytes (nonce counter)
        let min_required = info.aad_len + 16 + 4;
        if packet.len() < min_required {
            bail!(
                "Pacote truncado: tamanho {} menor que o mínimo necessário ({})",
                packet.len(),
                min_required
            );
        }

        let aad = &packet[..info.aad_len];
        let nonce_slice = &packet[packet.len() - 4..];
        let mut nonce_bytes = [0u8; 4];
        nonce_bytes.copy_from_slice(nonce_slice);

        let ciphertext = &packet[info.aad_len..packet.len() - 4];

        let plain = match &self.engine {
            CipherEngine::Aes256Gcm(c) => {
                let n = Self::make_aes_nonce(nonce_bytes);
                c.decrypt(
                    Nonce::from_slice(&n),
                    Payload {
                        msg: ciphertext,
                        aad,
                    },
                )
                .map_err(|e| anyhow!("Falha na decifração AES-256-GCM (pacote adulterado ou tag inválida): {e}"))?
            }
            CipherEngine::XChaCha20Poly1305(c) => {
                let n = Self::make_xchacha_nonce(nonce_bytes);
                c.decrypt(
                    XNonce::from_slice(&n),
                    Payload {
                        msg: ciphertext,
                        aad,
                    },
                )
                .map_err(|e| {
                    anyhow!("Falha na decifração XChaCha20-Poly1305 (pacote adulterado ou tag inválida): {e}")
                })?
            }
        };

        // No modo rtpsize com extensão de cabeçalho, a carga da extensão cifrada fica no início
        // do payload e precisa ser descartada para entregar apenas o áudio Opus.
        if info.ext_payload_bytes > 0 {
            if plain.len() < info.ext_payload_bytes {
                bail!(
                    "Payload decifrado menor que a carga de extensão declarada ({} < {})",
                    plain.len(),
                    info.ext_payload_bytes
                );
            }
            let opus = plain[info.ext_payload_bytes..].to_vec();
            Ok((info, opus))
        } else {
            Ok((info, plain))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_secret_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = (i * 7 + 13) as u8;
        }
        k
    }

    fn sample_rtp_header(seq: u16, ts: u32, ssrc: u32) -> [u8; 12] {
        let mut h = [0u8; 12];
        h[0] = 0x80;
        h[1] = 0x78;
        h[2..4].copy_from_slice(&seq.to_be_bytes());
        h[4..8].copy_from_slice(&ts.to_be_bytes());
        h[8..12].copy_from_slice(&ssrc.to_be_bytes());
        h
    }

    #[test]
    fn test_aes256_gcm_encrypt_decrypt_roundtrip() {
        let key = test_secret_key();
        let mut crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();

        let header = sample_rtp_header(1001, 48000, 0x12345678);
        let opus_data = b"discord opus audio voice payload 20ms test frame";

        let encrypted = crypto.encrypt(&header, opus_data).unwrap();

        // Header deve permanecer intacto e em texto claro nos primeiros 12 bytes
        assert_eq!(&encrypted[..12], &header);
        // O sufixo deve conter o nonce de 4 bytes (contador 0 -> 0x00000000)
        assert_eq!(&encrypted[encrypted.len() - 4..], &[0, 0, 0, 0]);

        // Decifrar
        let (ssrc, decrypted) = crypto.decrypt_packet(&encrypted).unwrap();
        assert_eq!(ssrc, 0x12345678);
        assert_eq!(decrypted, opus_data);
    }

    #[test]
    fn test_xchacha20_poly1305_encrypt_decrypt_roundtrip() {
        let key = test_secret_key();
        let mut crypto = RtpCrypto::new("aead_xchacha20_poly1305_rtpsize", &key).unwrap();

        let header = sample_rtp_header(505, 96000, 0x87654321);
        let opus_data = b"another opus payload with different test parameters";

        let encrypted = crypto.encrypt(&header, opus_data).unwrap();

        assert_eq!(&encrypted[..12], &header);
        assert_eq!(&encrypted[encrypted.len() - 4..], &[0, 0, 0, 0]);

        let (ssrc, decrypted) = crypto.decrypt_packet(&encrypted).unwrap();
        assert_eq!(ssrc, 0x87654321);
        assert_eq!(decrypted, opus_data);
    }

    #[test]
    fn test_tampered_packet_rejected_aes_gcm() {
        let key = test_secret_key();
        let mut crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();

        let header = sample_rtp_header(10, 960, 0xABC);
        let opus = b"top secret voice message";

        let encrypted = crypto.encrypt(&header, opus).unwrap();

        // 1. Adulterar o cabeçalho (AAD)
        let mut tampered_header = encrypted.clone();
        tampered_header[3] ^= 0xFF; // altera sequence
        assert!(crypto.decrypt(&tampered_header).is_err());

        // 2. Adulterar a carga cifrada
        let mut tampered_body = encrypted.clone();
        tampered_body[15] ^= 0xAA;
        assert!(crypto.decrypt(&tampered_body).is_err());

        // 3. Adulterar o nonce de 4 bytes
        let mut tampered_nonce = encrypted.clone();
        let last = tampered_nonce.len() - 1;
        tampered_nonce[last] ^= 0x01;
        assert!(crypto.decrypt(&tampered_nonce).is_err());
    }

    #[test]
    fn test_tampered_packet_rejected_xchacha20() {
        let key = test_secret_key();
        let mut crypto = RtpCrypto::new("aead_xchacha20_poly1305_rtpsize", &key).unwrap();

        let header = sample_rtp_header(20, 1920, 0xDEF);
        let opus = b"another voice data packet";

        let encrypted = crypto.encrypt(&header, opus).unwrap();

        // Adulterar tag/corpo
        let mut tampered = encrypted.clone();
        tampered[18] ^= 0x55;
        assert!(crypto.decrypt(&tampered).is_err());
    }

    #[test]
    fn test_extension_preamble_aad_and_payload_stripping() {
        let key = test_secret_key();
        let mut crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();

        // Criar pacote com bit X (extensão) ativo: byte 0 = 0x90
        // Base header = 12 bytes
        // Preâmbulo da extensão: profile = 0xBEDE (2 bytes), length in 32-bit words = 1 (2 bytes) -> 4 bytes de extensão
        let mut unencrypted = Vec::new();
        unencrypted.push(0x90); // Version 2, Extension=1, CC=0
        unencrypted.push(0x78); // Payload type 120
        unencrypted.extend_from_slice(&123u16.to_be_bytes()); // Seq
        unencrypted.extend_from_slice(&456u32.to_be_bytes()); // Timestamp
        unencrypted.extend_from_slice(&789u32.to_be_bytes()); // SSRC

        // Preâmbulo da extensão (4 bytes)
        unencrypted.extend_from_slice(&0xBEDEu16.to_be_bytes()); // Profile
        unencrypted.extend_from_slice(&1u16.to_be_bytes()); // 1 word = 4 bytes de carga de extensão

        // Carga da extensão (4 bytes)
        unencrypted.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]);

        // Áudio Opus
        let raw_opus = b"opus audio data after rtp header extension";
        unencrypted.extend_from_slice(raw_opus);

        // Criptografar usando encrypt_packet
        let encrypted = crypto.encrypt_packet(&unencrypted).unwrap();

        // O AAD deve ser 16 bytes: 12 bytes de cabeçalho base + 4 bytes de preâmbulo
        assert_eq!(&encrypted[..16], &unencrypted[..16]);

        // Decifrar: a carga da extensão (0x11, 0x22, 0x33, 0x44) deve ser removida
        // e o resultado final deve ser apenas raw_opus!
        let (ssrc, decrypted_opus) = crypto.decrypt_packet(&encrypted).unwrap();
        assert_eq!(ssrc, 789);
        assert_eq!(decrypted_opus, raw_opus);
    }
}
