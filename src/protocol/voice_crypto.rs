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
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use anyhow::{Context, Result, anyhow, bail};
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
    /// Indicador de padding RTP (`P`), removido depois da decifração.
    pub has_padding: bool,
}

/// Parser de cabeçalho RTP estritamente compatível com RFC 3550 e RFC 3711 (SRTP / Discord rtpsize).
pub fn parse_rtp_aad_and_ext(buf: &[u8]) -> Result<RtpAadInfo> {
    if buf.len() < 12 {
        bail!(
            "Pacote RTP menor que o cabeçalho base de 12 bytes (tamanho: {})",
            buf.len()
        );
    }

    let b0 = buf[0];
    let version = (b0 >> 6) & 0x03;
    if version != 2 {
        bail!(
            "Versão de cabeçalho RTP inválida: esperada 2 (0x80), encontrada {}",
            version
        );
    }

    let has_extension = (b0 & 0x10) != 0;
    let has_padding = (b0 & 0x20) != 0;
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
        let ext_words =
            u16::from_be_bytes([buf[base_header_len + 2], buf[base_header_len + 3]]) as usize;
        ext_payload_bytes = ext_words * 4;
        // No modo rtpsize do Discord/SRTP, o preâmbulo de 4 bytes é incluído no AAD não cifrado.
        aad_len = base_header_len + 4;
        let declared_end = aad_len
            .checked_add(ext_payload_bytes)
            .context("Tamanho de extensão RTP excede o limite de endereçamento")?;
        if buf.len() < declared_end {
            bail!(
                "Pacote RTP truncado antes do fim da extensão declarada (requer {}, recebido {})",
                declared_end,
                buf.len()
            );
        }
    }

    Ok(RtpAadInfo {
        aad_len,
        ext_payload_bytes,
        ssrc,
        sequence,
        timestamp,
        payload_type,
        has_extension,
        has_padding,
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
    nonce_exhausted: bool,
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
            nonce_exhausted: false,
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

    /// Define manualmente o contador de nonce exclusivamente para testes.
    #[cfg(test)]
    pub fn set_nonce_counter(&mut self, val: u32) {
        self.nonce_counter = val;
        self.nonce_exhausted = false;
    }

    fn next_nonce_bytes(&mut self) -> Result<(u32, [u8; 4])> {
        if self.nonce_exhausted {
            bail!("Contador de nonce RTP esgotado; reconecte a sessão para obter uma nova chave");
        }

        let val = self.nonce_counter;
        if val == u32::MAX {
            self.nonce_exhausted = true;
        } else {
            self.nonce_counter += 1;
        }
        Ok((val, val.to_be_bytes()))
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
        let (_, nonce_bytes) = self.next_nonce_bytes()?;

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
        let encrypted_payload_start = info
            .aad_len
            .checked_add(info.ext_payload_bytes)
            .context("Tamanho de extensão RTP excede o limite de endereçamento")?;
        if unencrypted_packet.len() < encrypted_payload_start {
            bail!(
                "Pacote RTP truncado antes do fim da extensão declarada (requer {}, recebido {})",
                encrypted_payload_start,
                unencrypted_packet.len()
            );
        }
        if info.has_padding {
            let media_and_padding = &unencrypted_packet[info.aad_len + info.ext_payload_bytes..];
            let padding = *media_and_padding
                .last()
                .context("Pacote RTP com padding não contém o octeto de contagem")?
                as usize;
            if padding == 0 || padding > media_and_padding.len() {
                bail!("Contador de padding RTP inválido: {padding}");
            }
        }

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

        // Tamanho mínimo: AAD + extensão cifrada + tag AEAD + sufixo de nonce.
        // Pacotes com padding precisam de pelo menos um octeto para o contador P.
        let min_required =
            info.aad_len + info.ext_payload_bytes + 16 + 4 + usize::from(info.has_padding);
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
                .map_err(|e| {
                    anyhow!(
                        "Falha na decifração AES-256-GCM (pacote adulterado ou tag inválida): {e}"
                    )
                })?
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

        // No modo rtpsize, o preâmbulo da extensão é AAD e o corpo RFC 5285
        // (incluindo cabeçalhos one-byte/two-byte) é cifrado no início do payload.
        let mut opus = if info.ext_payload_bytes > 0 {
            if plain.len() < info.ext_payload_bytes {
                bail!(
                    "Payload decifrado menor que a carga de extensão declarada ({} < {})",
                    plain.len(),
                    info.ext_payload_bytes
                );
            }
            &plain[info.ext_payload_bytes..]
        } else {
            plain.as_slice()
        };

        // RFC 3550: o último octeto do payload decifrado contém a quantidade de
        // bytes de padding, incluindo ele próprio. O padding não vai ao Opus/DAVE.
        if info.has_padding {
            let padding = *opus
                .last()
                .context("Pacote RTP com padding não contém o octeto de contagem")?
                as usize;
            if padding == 0 || padding > opus.len() {
                bail!(
                    "Contador de padding RTP inválido: {padding} para {} bytes",
                    opus.len()
                );
            }
            opus = &opus[..opus.len() - padding];
        }

        Ok((info, opus.to_vec()))
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

    #[test]
    fn test_aes_gcm_matches_independent_nonce_and_aad_vector() {
        // Vetor gerado fora deste crate com Python/cryptography AESGCM:
        // nonce completo = 0A0B0C0D || 00..00 (12 bytes), AAD = cabeçalho RTP.
        let key: Vec<u8> = (0..32).collect();
        let header = [
            0x80, 0x78, 0x01, 0x02, 0x00, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
        ];
        let plaintext = b"opus-regression-frame";
        let expected = hex::decode(
            "b92ca6f0954d17358ad81eb3dc70278dd0604d5366cd5b33ddeedab96b4da31c08d368d61c",
        )
        .unwrap();

        let mut crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
        crypto.set_nonce_counter(0x0A0B0C0D);
        let packet = crypto.encrypt(&header, plaintext).unwrap();

        assert_eq!(&packet[..12], &header);
        assert_eq!(&packet[12..packet.len() - 4], expected.as_slice());
        assert_eq!(&packet[packet.len() - 4..], &[0x0A, 0x0B, 0x0C, 0x0D]);
        assert_eq!(crypto.decrypt(&packet).unwrap(), plaintext);
    }

    #[test]
    fn test_xchacha20_matches_independent_nonce_and_aad_vector() {
        // Vetor gerado fora deste crate por HChaCha20 + ChaCha20Poly1305:
        // nonce completo = 0A0B0C0D || 00..00 (24 bytes), AAD = cabeçalho RTP.
        let key: Vec<u8> = (0..32).collect();
        let header = [
            0x80, 0x78, 0x01, 0x02, 0x00, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
        ];
        let plaintext = b"opus-regression-frame";
        let expected = hex::decode(
            "1240aa350a87378c929eb1c6e12db4c8fa41511aa02fcd0e4a47ca29b64dbffaf77092ada4",
        )
        .unwrap();

        let mut crypto = RtpCrypto::new("aead_xchacha20_poly1305_rtpsize", &key).unwrap();
        crypto.set_nonce_counter(0x0A0B0C0D);
        let packet = crypto.encrypt(&header, plaintext).unwrap();

        assert_eq!(&packet[..12], &header);
        assert_eq!(&packet[12..packet.len() - 4], expected.as_slice());
        assert_eq!(&packet[packet.len() - 4..], &[0x0A, 0x0B, 0x0C, 0x0D]);
        assert_eq!(crypto.decrypt(&packet).unwrap(), plaintext);
    }

    #[test]
    fn test_csrc_one_byte_extension_and_rtp_padding_are_stripped() {
        let key = test_secret_key();
        let mut crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
        let mut packet = Vec::new();

        // V=2, P=1, X=1, CC=3. AAD = 12 + 3*4 CSRC + 4 preâmbulo = 28 bytes.
        packet.push(0xB3);
        packet.push(0x78);
        packet.extend_from_slice(&7u16.to_be_bytes());
        packet.extend_from_slice(&960u32.to_be_bytes());
        packet.extend_from_slice(&0x12345678u32.to_be_bytes());
        for csrc in [0xAAAAAAAAu32, 0xBBBBBBBB, 0xCCCCCCCC] {
            packet.extend_from_slice(&csrc.to_be_bytes());
        }
        packet.extend_from_slice(&0xBEDEu16.to_be_bytes());
        packet.extend_from_slice(&2u16.to_be_bytes());

        // RFC 5285 one-byte header: ID=1, len=(0x2+1)=3 bytes, seguido de
        // três dados e um octeto de alinhamento. O corpo inteiro é cifrado.
        packet.extend_from_slice(&[0x12, 0xAA, 0xBB, 0xCC, 0x00, 0x00, 0x00, 0x00]);
        packet.extend_from_slice(&[0xF8, 0xFF, 0xFE]);
        packet.extend_from_slice(&[0x00, 0x00, 0x00, 0x04]); // P: quatro bytes, incluindo este

        let encrypted = crypto.encrypt_packet(&packet).unwrap();
        assert_eq!(&encrypted[..28], &packet[..28]);
        let (info, opus) = crypto.decrypt_full(&encrypted).unwrap();
        assert_eq!(info.aad_len, 28);
        assert_eq!(info.ext_payload_bytes, 8);
        assert!(info.has_padding);
        assert_eq!(opus, [0xF8, 0xFF, 0xFE]);
    }

    #[test]
    fn test_truncated_extension_and_invalid_padding_are_rejected() {
        let key = test_secret_key();
        let mut crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
        let mut truncated = sample_rtp_header(1, 0, 1).to_vec();
        truncated[0] = 0x90;
        truncated.extend_from_slice(&0xBEDEu16.to_be_bytes());
        truncated.extend_from_slice(&1u16.to_be_bytes());
        assert!(crypto.encrypt_packet(&truncated).is_err());

        let mut padded_header = sample_rtp_header(2, 960, 1);
        padded_header[0] |= 0x20;
        let encrypted = crypto
            .encrypt(&padded_header, &[0xF8, 0xFF, 0xFE, 0x00])
            .unwrap();
        assert!(crypto.decrypt(&encrypted).is_err());
    }

    #[test]
    fn test_nonce_counter_never_wraps_and_reuses_a_nonce() {
        let key = test_secret_key();
        let mut crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
        let header = sample_rtp_header(1, 0, 1);
        crypto.set_nonce_counter(u32::MAX);

        let last = crypto.encrypt(&header, b"last frame").unwrap();
        assert_eq!(&last[last.len() - 4..], &[0xFF, 0xFF, 0xFF, 0xFF]);
        assert!(crypto.encrypt(&header, b"must not reuse nonce").is_err());

        crypto.set_nonce_counter(0);
        assert!(crypto.encrypt(&header, b"new session").is_ok());
    }
}
