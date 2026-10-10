//! Discord Voice UDP Transport, IP Discovery & RTP Stream (Fase 8.2).
//!
//! Este módulo gerencia:
//! 1. **IP Discovery**: Pacote de 74 bytes enviado ao servidor UDP do Discord para resolver
//!    o endereço IP externo e a porta do cliente (UDP hole punching via NAT).
//! 2. **RTP**: Cabeçalho padrão de 12 bytes (RFC 3550, versão 0x80, tipo de carga 0x78 / 120 Opus),
//!    com incremento estrito de sequência (+1) e timestamp (+960 amostras por quadro de 20 ms a 48 kHz).
//! 3. **Conexão de Voz**: Estrutura `VoiceUdpConnection` que transmite quadros Opus cifrados,
//!    injeta 5 quadros canônicos de silêncio (`0xF8, 0xFF, 0xFE`) ao cessar fala e decifra quadros recebidos
//!    identificando o SSRC de origem.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::{anyhow, bail, Result};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

use crate::protocol::voice_crypto::RtpCrypto;

/// Tamanho exato do pacote de IP Discovery do Discord (74 bytes).
pub const IP_DISCOVERY_PACKET_LEN: usize = 74;

/// Código de operação para requisição de IP Discovery (tipo 0x1).
pub const IP_DISCOVERY_REQUEST_TYPE: u16 = 0x0001;

/// Código de operação para resposta de IP Discovery (tipo 0x2).
pub const IP_DISCOVERY_RESPONSE_TYPE: u16 = 0x0002;

/// Comprimento constante do corpo de IP Discovery excluindo tipo e length (70 bytes).
pub const IP_DISCOVERY_BODY_LEN: u16 = 70;

/// Versão padrão do protocolo RTP (versão 2 com flags zero -> 0x80).
pub const RTP_VERSION_2: u8 = 0x80;

/// Tipo de carga (Payload Type) do Discord para áudio Opus (120 / 0x78).
pub const RTP_PAYLOAD_TYPE_OPUS: u8 = 0x78;

/// Frequência de amostragem padrão de voz do Discord (48 kHz).
pub const OPUS_SAMPLING_RATE_HZ: u32 = 48_000;

/// Duração canônica de um quadro Opus (20 ms).
pub const OPUS_FRAME_DURATION_MS: u64 = 20;

/// Amostras de áudio por quadro de 20 ms a 48 kHz (48000 * 0.020 = 960).
pub const OPUS_SAMPLES_PER_FRAME: u32 = 960;

/// Três bytes canônicos do quadro de silêncio do Opus (evita interpolações espúrias).
pub const SILENCE_FRAME: [u8; 3] = [0xF8, 0xFF, 0xFE];

/// Quantidade oficial de quadros de silêncio enviados ao parar de falar (5 quadros).
pub const SILENCE_FRAME_COUNT: usize = 5;

// ============================================================================
// 1. IP Discovery
// ============================================================================

/// Monta o pacote de requisição de IP Discovery de 74 bytes.
///
/// Estrutura:
/// - bytes 0..2: Tipo da requisição (0x0001 big-endian)
/// - bytes 2..4: Comprimento restante (70 = 0x0046 big-endian)
/// - bytes 4..8: SSRC do cliente (u32 big-endian)
/// - bytes 8..72: 64 bytes zerados (reservados para o IP na resposta)
/// - bytes 72..74: 2 bytes zerados (reservados para a porta na resposta)
pub fn build_ip_discovery_packet(ssrc: u32) -> [u8; IP_DISCOVERY_PACKET_LEN] {
    let mut packet = [0u8; IP_DISCOVERY_PACKET_LEN];
    packet[0..2].copy_from_slice(&IP_DISCOVERY_REQUEST_TYPE.to_be_bytes());
    packet[2..4].copy_from_slice(&IP_DISCOVERY_BODY_LEN.to_be_bytes());
    packet[4..8].copy_from_slice(&ssrc.to_be_bytes());
    packet
}

/// Realiza o parsing do pacote de resposta de IP Discovery retornado pelo Discord.
///
/// Extrai o IP público externo (string ASCII terminada em nulo) e a porta UDP (u16 big-endian).
pub fn parse_ip_discovery_response(buf: &[u8]) -> Result<(String, u16)> {
    if buf.len() < IP_DISCOVERY_PACKET_LEN {
        bail!(
            "Pacote de IP Discovery menor que o esperado (recebido {}, esperado {})",
            buf.len(),
            IP_DISCOVERY_PACKET_LEN
        );
    }

    let pkt_type = u16::from_be_bytes([buf[0], buf[1]]);
    if pkt_type != IP_DISCOVERY_RESPONSE_TYPE {
        bail!(
            "Tipo inválido na resposta de IP Discovery: esperado 0x0002, recebido 0x{:04X}",
            pkt_type
        );
    }

    let pkt_len = u16::from_be_bytes([buf[2], buf[3]]);
    if pkt_len != IP_DISCOVERY_BODY_LEN {
        bail!(
            "Comprimento inválido na resposta de IP Discovery: esperado 70, recebido {}",
            pkt_len
        );
    }

    // Endereço IP: 64 bytes terminados em nulo entre 8..72
    let ip_slice = &buf[8..72];
    let null_pos = ip_slice.iter().position(|&b| b == 0).unwrap_or(ip_slice.len());
    let ip_str = std::str::from_utf8(&ip_slice[..null_pos])
        .map_err(|e| anyhow!("Falha ao decodificar IP UTF-8 da resposta: {e}"))?
        .trim()
        .to_string();

    if ip_str.is_empty() {
        bail!("Endereço IP retornado pelo servidor está vazio");
    }

    // Porta: 2 bytes big-endian em 72..74
    let port = u16::from_be_bytes([buf[72], buf[73]]);
    if port == 0 {
        bail!("Porta UDP retornada na descoberta de IP é zero");
    }

    Ok((ip_str, port))
}

/// Contrato obrigatório com VoiceGW:
/// Executa a descoberta de IP e porta pública através de um `tokio::net::UdpSocket` conectado.
pub async fn discover_ip(socket: &UdpSocket, ssrc: u32) -> Result<(String, u16)> {
    let req = build_ip_discovery_packet(ssrc);

    // Se o socket já estiver conectado via `socket.connect`, enviamos direto:
    if socket.peer_addr().is_ok() {
        let mut attempts = 0;
        loop {
            attempts += 1;
            socket.send(&req).await?;

            let mut resp = [0u8; 128];
            match tokio::time::timeout(Duration::from_millis(1500), socket.recv(&mut resp)).await {
                Ok(Ok(n)) => return parse_ip_discovery_response(&resp[..n]),
                Ok(Err(e)) => bail!("Erro de I/O no socket UDP durante IP Discovery: {e}"),
                Err(_) if attempts < 3 => continue, // Tenta novamente
                Err(_) => bail!("Timeout de IP Discovery após 3 tentativas"),
            }
        }
    } else {
        bail!("UdpSocket precisa estar conectado ao endpoint de voz para usar discover_ip()");
    }
}

/// Variante de IP discovery que aceita endereço remoto explicitamente (se socket não estiver conectado).
pub async fn discover_ip_to(socket: &UdpSocket, target: SocketAddr, ssrc: u32) -> Result<(String, u16)> {
    let req = build_ip_discovery_packet(ssrc);
    let mut attempts = 0;
    loop {
        attempts += 1;
        socket.send_to(&req, target).await?;

        let mut resp = [0u8; 128];
        match tokio::time::timeout(Duration::from_millis(1500), socket.recv_from(&mut resp)).await {
            Ok(Ok((n, _from))) => return parse_ip_discovery_response(&resp[..n]),
            Ok(Err(e)) => bail!("Erro de I/O no socket UDP durante IP Discovery: {e}"),
            Err(_) if attempts < 3 => continue,
            Err(_) => bail!("Timeout de IP Discovery após 3 tentativas para {target}"),
        }
    }
}

// ============================================================================
// 2. RTP Headers & Builders
// ============================================================================

/// Constrói o cabeçalho base RTP canônico de 12 bytes do Discord para áudio Opus.
///
/// - Byte 0: 0x80 (Versão 2, sem padding, sem extensão, CC = 0)
/// - Byte 1: 0x78 (Tipo de carga 120 = Opus)
/// - Bytes 2..4: Sequence number (u16 big-endian)
/// - Bytes 4..8: Timestamp (u32 big-endian, incrementa de 960 por quadro de 20ms)
/// - Bytes 8..12: SSRC (u32 big-endian)
pub fn build_rtp_header(sequence: u16, timestamp: u32, ssrc: u32) -> [u8; 12] {
    let mut h = [0u8; 12];
    h[0] = RTP_VERSION_2;
    h[1] = RTP_PAYLOAD_TYPE_OPUS;
    h[2..4].copy_from_slice(&sequence.to_be_bytes());
    h[4..8].copy_from_slice(&timestamp.to_be_bytes());
    h[8..12].copy_from_slice(&ssrc.to_be_bytes());
    h
}

// ============================================================================
// 3. Conexão de Transporte de Voz (VoiceUdpConnection)
// ============================================================================

/// Conexão de transporte UDP para chamadas de voz e vídeo do Discord.
///
/// Gerencia a transmissão e recepção de pacotes RTP sobre UDP com criptografia de transporte
/// (`aead_aes256_gcm_rtpsize` ou `aead_xchacha20_poly1305_rtpsize`).
pub struct VoiceUdpConnection {
    socket: Arc<UdpSocket>,
    remote_addr: SocketAddr,
    ssrc: u32,
    sequence: u16,
    timestamp: u32,
    crypto: Arc<Mutex<RtpCrypto>>,
}

impl VoiceUdpConnection {
    /// Estabelece uma conexão UDP com os parâmetros recebidos do payload `Ready` (Opcode 2) do Gateway:
    /// `(ip, port, ssrc)` e o motor criptográfico inicializado a partir do `Session Description` (Opcode 4).
    pub async fn connect(ip: &str, port: u16, ssrc: u32, crypto: RtpCrypto) -> Result<Self> {
        let addr_str = format!("{}:{}", ip, port);
        let remote_addr: SocketAddr = addr_str
            .parse()
            .map_err(|e| anyhow!("Endereço de voz inválido '{addr_str}': {e}"))?;

        let bind_addr = if remote_addr.is_ipv6() {
            "[::]:0"
        } else {
            "0.0.0.0:0"
        };

        let socket = UdpSocket::bind(bind_addr).await?;
        socket.connect(remote_addr).await?;

        Ok(Self {
            socket: Arc::new(socket),
            remote_addr,
            ssrc,
            sequence: 0,
            timestamp: 0,
            crypto: Arc::new(Mutex::new(crypto)),
        })
    }

    /// Cria a conexão a partir de um `UdpSocket` já previamente aberto (ex: após IP Discovery).
    pub fn from_socket(socket: UdpSocket, remote_addr: SocketAddr, ssrc: u32, crypto: RtpCrypto) -> Self {
        Self {
            socket: Arc::new(socket),
            remote_addr,
            ssrc,
            sequence: 0,
            timestamp: 0,
            crypto: Arc::new(Mutex::new(crypto)),
        }
    }

    /// Retorna o SSRC local configurado para esta transmissão.
    pub fn ssrc(&self) -> u32 {
        self.ssrc
    }

    /// Retorna o número de sequência atual.
    pub fn sequence(&self) -> u16 {
        self.sequence
    }

    /// Retorna o timestamp de amostragem atual.
    pub fn timestamp(&self) -> u32 {
        self.timestamp
    }

    /// Retorna o endereço remoto do servidor de voz.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }

    /// Envia um quadro de áudio Opus já codificado (`&[u8]`).
    ///
    /// Monta o cabeçalho RTP de 12 bytes, incrementa sequência (+1) e timestamp (+960),
    /// criptografa via AEAD com nonce de 32 bits e envia sobre o socket UDP.
    pub async fn send_opus_frame(&mut self, opus_data: &[u8]) -> Result<()> {
        let seq = self.sequence;
        let ts = self.timestamp;

        // Incremento conforme a especificação do Discord para quadros de 20ms a 48kHz
        self.sequence = self.sequence.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(OPUS_SAMPLES_PER_FRAME);

        let header = build_rtp_header(seq, ts, self.ssrc);
        let mut crypto = self.crypto.lock().await;
        let packet = crypto.encrypt(&header, opus_data)?;
        drop(crypto);

        self.socket.send(&packet).await?;
        Ok(())
    }

    /// Envia os 5 quadros oficiais de silêncio canônico (`0xF8, 0xFF, 0xFE`) ao parar de falar.
    ///
    /// Necessário para evitar interpolação indevida do codec Opus pelo SFU do Discord
    /// em transmissões subsequentes.
    pub async fn send_silence_frames(&mut self) -> Result<()> {
        for _ in 0..SILENCE_FRAME_COUNT {
            self.send_opus_frame(&SILENCE_FRAME).await?;
        }
        Ok(())
    }

    /// Atalho para parar de falar, enviando os 5 quadros de silêncio.
    pub async fn stop_speaking(&mut self) -> Result<()> {
        self.send_silence_frames().await
    }

    /// Recebe o próximo quadro Opus decifrado, retornando uma tupla `(ssrc_origem, opus_payload)`.
    ///
    /// Filtra pacotes inválidos ou truncados e decifra a carga útil com verificação de autenticidade.
    pub async fn recv_opus_frame(&self) -> Result<(u32, Vec<u8>)> {
        let mut buf = [0u8; 2048];
        loop {
            let len = self.socket.recv(&mut buf).await?;
            if len < 12 {
                continue;
            }

            // Apenas pacotes RTP v2 (0x80)
            if (buf[0] & 0xC0) != 0x80 {
                continue;
            }

            let crypto = self.crypto.lock().await;
            match crypto.decrypt_packet(&buf[..len]) {
                Ok((origin_ssrc, opus_bytes)) => return Ok((origin_ssrc, opus_bytes)),
                Err(err) => {
                    log::debug!("Descartando pacote UDP que falhou na decifração: {err}");
                    continue;
                }
            }
        }
    }

    /// Recebe o próximo quadro Opus com um timeout configurável.
    pub async fn recv_opus_frame_timeout(&self, timeout: Duration) -> Result<(u32, Vec<u8>)> {
        tokio::time::timeout(timeout, self.recv_opus_frame())
            .await
            .map_err(|_| anyhow!("Timeout ao aguardar quadro de voz Opus sobre UDP"))?
    }

    /// Divide a conexão em metades de envio e recepção para permitir tarefas concorrentes de áudio.
    pub fn split(self) -> (VoiceUdpSender, VoiceUdpReceiver) {
        let sender = VoiceUdpSender {
            socket: self.socket.clone(),
            ssrc: self.ssrc,
            sequence: self.sequence,
            timestamp: self.timestamp,
            crypto: self.crypto.clone(),
        };

        let receiver = VoiceUdpReceiver {
            socket: self.socket,
            crypto: self.crypto,
        };

        (sender, receiver)
    }
}

/// Metade de envio da conexão de voz.
pub struct VoiceUdpSender {
    socket: Arc<UdpSocket>,
    ssrc: u32,
    sequence: u16,
    timestamp: u32,
    crypto: Arc<Mutex<RtpCrypto>>,
}

impl VoiceUdpSender {
    pub async fn send_opus_frame(&mut self, opus_data: &[u8]) -> Result<()> {
        let seq = self.sequence;
        let ts = self.timestamp;

        self.sequence = self.sequence.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(OPUS_SAMPLES_PER_FRAME);

        let header = build_rtp_header(seq, ts, self.ssrc);
        let mut crypto = self.crypto.lock().await;
        let packet = crypto.encrypt(&header, opus_data)?;
        drop(crypto);

        self.socket.send(&packet).await?;
        Ok(())
    }

    pub async fn send_silence_frames(&mut self) -> Result<()> {
        for _ in 0..SILENCE_FRAME_COUNT {
            self.send_opus_frame(&SILENCE_FRAME).await?;
        }
        Ok(())
    }

    pub async fn stop_speaking(&mut self) -> Result<()> {
        self.send_silence_frames().await
    }
}

/// Metade de recepção da conexão de voz.
pub struct VoiceUdpReceiver {
    socket: Arc<UdpSocket>,
    crypto: Arc<Mutex<RtpCrypto>>,
}

impl VoiceUdpReceiver {
    pub async fn recv_opus_frame(&self) -> Result<(u32, Vec<u8>)> {
        let mut buf = [0u8; 2048];
        loop {
            let len = self.socket.recv(&mut buf).await?;
            if len < 12 || (buf[0] & 0xC0) != 0x80 {
                continue;
            }

            let crypto = self.crypto.lock().await;
            if let Ok((origin_ssrc, opus_bytes)) = crypto.decrypt_packet(&buf[..len]) {
                return Ok((origin_ssrc, opus_bytes));
            }
        }
    }
}

// ============================================================================
// Testes Unitários
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_secret_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = (i * 11 + 3) as u8;
        }
        k
    }

    #[test]
    fn test_build_ip_discovery_packet() {
        let ssrc = 0xDEADBEEF;
        let pkt = build_ip_discovery_packet(ssrc);

        assert_eq!(pkt.len(), 74);
        assert_eq!(&pkt[0..2], &1u16.to_be_bytes()); // Type 1
        assert_eq!(&pkt[2..4], &70u16.to_be_bytes()); // Length 70
        assert_eq!(&pkt[4..8], &ssrc.to_be_bytes()); // SSRC
        assert_eq!(&pkt[8..72], &[0u8; 64]); // Endereço zerado
        assert_eq!(&pkt[72..74], &[0u8; 2]); // Porta zerada
    }

    #[test]
    fn test_parse_ip_discovery_response() {
        let mut resp = [0u8; 74];
        resp[0..2].copy_from_slice(&2u16.to_be_bytes()); // Type 2
        resp[2..4].copy_from_slice(&70u16.to_be_bytes()); // Length 70
        resp[4..8].copy_from_slice(&0x12345678u32.to_be_bytes()); // SSRC

        // IP "203.0.113.195\0"
        let ip_bytes = b"203.0.113.195";
        resp[8..8 + ip_bytes.len()].copy_from_slice(ip_bytes);
        resp[8 + ip_bytes.len()] = 0; // Null terminator

        // Porta 54321
        resp[72..74].copy_from_slice(&54321u16.to_be_bytes());

        let (ip, port) = parse_ip_discovery_response(&resp).unwrap();
        assert_eq!(ip, "203.0.113.195");
        assert_eq!(port, 54321);
    }

    #[test]
    fn test_parse_ip_discovery_response_invalid_type() {
        let mut resp = [0u8; 74];
        resp[0..2].copy_from_slice(&1u16.to_be_bytes()); // Type 1 inválido para resposta
        resp[2..4].copy_from_slice(&70u16.to_be_bytes());
        assert!(parse_ip_discovery_response(&resp).is_err());
    }

    #[tokio::test]
    async fn test_ip_discovery_loopback_socket() {
        // Simulação do servidor UDP do Discord em loopback 127.0.0.1
        let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_socket.local_addr().unwrap();

        let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client_socket.connect(server_addr).await.unwrap();

        let test_ssrc: u32 = 987654321;

        // Tarefa que atua como o servidor UDP de voz
        let server_handle = tokio::spawn(async move {
            let mut buf = [0u8; 128];
            let (n, from) = server_socket.recv_from(&mut buf).await.unwrap();
            assert_eq!(n, 74);
            assert_eq!(&buf[0..2], &1u16.to_be_bytes());
            assert_eq!(&buf[4..8], &test_ssrc.to_be_bytes());

            // Responde com type 2, length 70, ssrc, ip="198.51.100.77", port=41234
            let mut resp = [0u8; 74];
            resp[0..2].copy_from_slice(&2u16.to_be_bytes());
            resp[2..4].copy_from_slice(&70u16.to_be_bytes());
            resp[4..8].copy_from_slice(&test_ssrc.to_be_bytes());
            let ip = b"198.51.100.77";
            resp[8..8 + ip.len()].copy_from_slice(ip);
            resp[8 + ip.len()] = 0;
            resp[72..74].copy_from_slice(&41234u16.to_be_bytes());

            server_socket.send_to(&resp, from).await.unwrap();
        });

        let (discovered_ip, discovered_port) = discover_ip(&client_socket, test_ssrc).await.unwrap();
        server_handle.await.unwrap();

        assert_eq!(discovered_ip, "198.51.100.77");
        assert_eq!(discovered_port, 41234);
    }

    #[test]
    fn test_rtp_header_format_and_increments() {
        let seq = 42;
        let ts = 960 * 5;
        let ssrc = 0xAABBCCDD;

        let header = build_rtp_header(seq, ts, ssrc);
        assert_eq!(header[0], 0x80); // Version 2
        assert_eq!(header[1], 0x78); // Payload type 120 (Opus)
        assert_eq!(u16::from_be_bytes([header[2], header[3]]), seq);
        assert_eq!(u32::from_be_bytes([header[4], header[5], header[6], header[7]]), ts);
        assert_eq!(u32::from_be_bytes([header[8], header[9], header[10], header[11]]), ssrc);
    }

    #[tokio::test]
    async fn test_voice_udp_send_receive_roundtrip_aes_gcm() {
        let key = test_secret_key();
        let crypto_send = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
        let crypto_recv = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();

        let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_socket.local_addr().unwrap();

        let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let client_addr = client_socket.local_addr().unwrap();

        client_socket.connect(server_addr).await.unwrap();
        server_socket.connect(client_addr).await.unwrap();

        let ssrc = 0x11223344;
        let mut sender = VoiceUdpConnection::from_socket(client_socket, server_addr, ssrc, crypto_send);
        let receiver = VoiceUdpConnection::from_socket(server_socket, client_addr, ssrc, crypto_recv);

        let test_opus = b"real opus audio encoded frame 20ms";
        sender.send_opus_frame(test_opus).await.unwrap();

        assert_eq!(sender.sequence(), 1);
        assert_eq!(sender.timestamp(), 960);

        let (origin_ssrc, recv_opus) = receiver
            .recv_opus_frame_timeout(Duration::from_millis(500))
            .await
            .unwrap();

        assert_eq!(origin_ssrc, ssrc);
        assert_eq!(recv_opus, test_opus);
    }

    #[tokio::test]
    async fn test_voice_udp_send_silence_frames() {
        let key = test_secret_key();
        let crypto_send = RtpCrypto::new("aead_xchacha20_poly1305_rtpsize", &key).unwrap();
        let crypto_recv = RtpCrypto::new("aead_xchacha20_poly1305_rtpsize", &key).unwrap();

        let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_socket.local_addr().unwrap();

        let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let client_addr = client_socket.local_addr().unwrap();

        client_socket.connect(server_addr).await.unwrap();
        server_socket.connect(client_addr).await.unwrap();

        let ssrc = 0x55667788;
        let mut sender = VoiceUdpConnection::from_socket(client_socket, server_addr, ssrc, crypto_send);
        let receiver = VoiceUdpConnection::from_socket(server_socket, client_addr, ssrc, crypto_recv);

        // Dispara os 5 quadros de silêncio ao parar de falar
        sender.stop_speaking().await.unwrap();

        assert_eq!(sender.sequence(), 5);
        assert_eq!(sender.timestamp(), 960 * 5);

        for _ in 0..SILENCE_FRAME_COUNT {
            let (origin_ssrc, recv_opus) = receiver
                .recv_opus_frame_timeout(Duration::from_millis(500))
                .await
                .unwrap();
            assert_eq!(origin_ssrc, ssrc);
            assert_eq!(recv_opus, SILENCE_FRAME);
        }
    }
}
