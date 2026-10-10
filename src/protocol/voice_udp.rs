//! Discord Voice UDP Transport, IP Discovery, RTP Streaming & Keepalive (Fase 8.2).
//!
//! Este módulo gerencia:
//! 1. **Handshake em Duas Fases**:
//!    - Fase 1: `VoiceUdpSocket::connect(ip, port, ssrc)` abre o socket UDP e conecta ao endpoint do Discord.
//!    - IP Discovery no mesmo socket: `socket.discover_ip()` (garante que o mapeamento NAT seja idêntico).
//!    - Fase 2: `socket.into_connection(crypto)` devolve a `VoiceUdpConnection` quando o Session Description chegar.
//! 2. **IP Discovery**: Pacote de 74 bytes (tipo 0x1 req / 0x2 resp, tamanho 70, SSRC, IP nulo de 64 bytes e porta).
//! 3. **RTP**: Cabeçalho padrão de 12 bytes (RFC 3550, versão 0x80, tipo 0x78 / 120 Opus), incremento estrito
//!    de sequência (+1) e timestamp (+960 amostras por 20 ms a 48 kHz).
//! 4. **Keepalive UDP**: Envio periódico de pacotes de 8 bytes (contador u64 little-endian) a cada ~5 s
//!    para manter abertos os pinholes de NAT, e descarte transparente de respostas no caminho de recepção.
//! 5. **Recepção e Filtragem RTCP**: Entrega de `ssrc`, `sequence`, `timestamp` e `payload` Opus,
//!    com descarte explícito de pacotes RTCP (payload types 200-204 no byte 1).
//! 6. **Silêncio**: Envio dos 5 quadros canônicos de silêncio (`0xF8, 0xFF, 0xFE`) ao cessar fala.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
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

/// Intervalo padrão de keepalive UDP para manter o mapeamento de NAT ativo (5 segundos).
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(5);

/// Tamanho do pacote de keepalive UDP do Discord (8 bytes).
pub const KEEPALIVE_PACKET_LEN: usize = 8;

// ============================================================================
// 1. IP Discovery
// ============================================================================

/// Monta o pacote de requisição de IP Discovery de 74 bytes.
pub fn build_ip_discovery_packet(ssrc: u32) -> [u8; IP_DISCOVERY_PACKET_LEN] {
    let mut packet = [0u8; IP_DISCOVERY_PACKET_LEN];
    packet[0..2].copy_from_slice(&IP_DISCOVERY_REQUEST_TYPE.to_be_bytes());
    packet[2..4].copy_from_slice(&IP_DISCOVERY_BODY_LEN.to_be_bytes());
    packet[4..8].copy_from_slice(&ssrc.to_be_bytes());
    packet
}

/// Realiza o parsing do pacote de resposta de IP Discovery retornado pelo Discord.
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

    let ip_slice = &buf[8..72];
    let null_pos = ip_slice.iter().position(|&b| b == 0).unwrap_or(ip_slice.len());
    let ip_str = std::str::from_utf8(&ip_slice[..null_pos])
        .map_err(|e| anyhow!("Falha ao decodificar IP UTF-8 da resposta: {e}"))?
        .trim()
        .to_string();

    if ip_str.is_empty() {
        bail!("Endereço IP retornado pelo servidor está vazio");
    }

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

    if socket.peer_addr().is_ok() {
        let mut attempts = 0;
        loop {
            attempts += 1;
            socket.send(&req).await?;

            let mut resp = [0u8; 128];
            match tokio::time::timeout(Duration::from_millis(1500), socket.recv(&mut resp)).await {
                Ok(Ok(n)) => return parse_ip_discovery_response(&resp[..n]),
                Ok(Err(e)) => bail!("Erro de I/O no socket UDP durante IP Discovery: {e}"),
                Err(_) if attempts < 3 => continue,
                Err(_) => bail!("Timeout de IP Discovery após 3 tentativas"),
            }
        }
    } else {
        bail!("UdpSocket precisa estar conectado ao endpoint de voz para usar discover_ip()");
    }
}

/// Variante de IP discovery que aceita endereço remoto explicitamente.
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
// 3. Pacote de Voz e Keepalive
// ============================================================================

/// Representação completa de um pacote de voz Opus recebido do Discord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoicePacket {
    /// SSRC da fonte de transmissão.
    pub ssrc: u32,
    /// Número de sequência do pacote RTP (útil para reordenação e detecção de jitter).
    pub sequence: u16,
    /// Timestamp de amostragem RTP (útil para sincronização e detecção de gaps/PLC).
    pub timestamp: u32,
    /// Carga útil de áudio Opus decifrada.
    pub payload: Vec<u8>,
}

/// Monta um pacote de keepalive UDP com contador de 64 bits em little-endian.
/// Fonte de referência: `discordgo/voice.go` (`udpKeepAlive`), `binary.LittleEndian.PutUint64`.
pub fn build_keepalive_packet(sequence: u64) -> [u8; KEEPALIVE_PACKET_LEN] {
    sequence.to_le_bytes()
}

// ============================================================================
// 4. API em Duas Fases: VoiceUdpSocket -> VoiceUdpConnection
// ============================================================================

/// Fase 1 do Handshake de Voz: Socket UDP conectado ao endpoint de voz do Discord.
///
/// Permite executar o `discover_ip()` no MESMO socket antes da chegada da chave secreta
/// (Session Description / Opcode 4), garantindo a preservação do mapeamento de NAT.
pub struct VoiceUdpSocket {
    socket: Arc<UdpSocket>,
    remote_addr: SocketAddr,
    ssrc: u32,
}

impl VoiceUdpSocket {
    /// Fase 1: Conecta ao endpoint UDP informado no payload `Ready` (Opcode 2).
    pub async fn connect(ip: &str, port: u16, ssrc: u32) -> Result<Self> {
        let addr_str = format!("{}:{}", ip, port);
        let remote_addr: SocketAddr = match addr_str.parse() {
            Ok(addr) => addr,
            Err(_) => match tokio::net::lookup_host(&addr_str).await?.next() {
                Some(addr) => addr,
                None => bail!("Falha ao resolver endereço de voz '{addr_str}'"),
            },
        };

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
        })
    }

    /// Cria a partir de um `UdpSocket` já previamente aberto e conectado.
    pub fn from_socket(socket: UdpSocket, remote_addr: SocketAddr, ssrc: u32) -> Self {
        Self {
            socket: Arc::new(socket),
            remote_addr,
            ssrc,
        }
    }

    /// Executa o IP Discovery usando o próprio socket conectado.
    pub async fn discover_ip(&self) -> Result<(String, u16)> {
        discover_ip(&self.socket, self.ssrc).await
    }

    /// SSRC configurado.
    pub fn ssrc(&self) -> u32 {
        self.ssrc
    }

    /// Endereço remoto do servidor de voz.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }

    /// Referência compartilhada do socket UDP subjacente.
    pub fn socket(&self) -> &Arc<UdpSocket> {
        &self.socket
    }

    /// Fase 2: Transfere o socket para uma `VoiceUdpConnection` quando o
    /// Session Description (Opcode 4) entregar o `RtpCrypto` (mode + secret_key).
    pub fn into_connection(self, crypto: RtpCrypto) -> VoiceUdpConnection {
        VoiceUdpConnection::from_shared_socket(self.socket, self.remote_addr, self.ssrc, crypto)
    }
}

// ============================================================================
// 5. Metades de Envio e Recepção (Sem Duplicação de Código)
// ============================================================================

/// Metade de envio da conexão de voz.
///
/// Implementa a montagem de RTP, criptografia, envio de áudio Opus, envio de silêncio e keepalives.
pub struct VoiceUdpSender {
    socket: Arc<UdpSocket>,
    remote_addr: SocketAddr,
    ssrc: u32,
    sequence: u16,
    timestamp: u32,
    crypto: Arc<Mutex<RtpCrypto>>,
    keepalive_seq: Arc<AtomicU64>,
}

impl VoiceUdpSender {
    /// Retorna o SSRC local configurado.
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

    /// Retorna o endereço remoto.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }

    /// Retorna uma referência ao socket UDP subjacente.
    pub fn socket(&self) -> &Arc<UdpSocket> {
        &self.socket
    }

    /// Envia um quadro de áudio Opus já codificado (`&[u8]`).
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

    /// Envia os 5 quadros canônicos de silêncio (`0xF8, 0xFF, 0xFE`) ao parar de falar.
    pub async fn send_silence_frames(&mut self) -> Result<()> {
        for _ in 0..SILENCE_FRAME_COUNT {
            self.send_opus_frame(&SILENCE_FRAME).await?;
        }
        Ok(())
    }

    /// Atalho para cessar fala.
    pub async fn stop_speaking(&mut self) -> Result<()> {
        self.send_silence_frames().await
    }

    /// Envia um pacote de keepalive UDP com contador de 8 bytes (uint64 little-endian).
    pub async fn send_keepalive(&self) -> Result<()> {
        let seq = self.keepalive_seq.fetch_add(1, Ordering::Relaxed);
        let packet = build_keepalive_packet(seq);
        self.socket.send(&packet).await?;
        Ok(())
    }

    /// Inicia uma tarefa assíncrona periódica em segundo plano que emite keepalives UDP a cada `interval`.
    pub fn spawn_keepalive_task(&self, interval: Duration) -> tokio::task::JoinHandle<()> {
        let socket = self.socket.clone();
        let seq_counter = self.keepalive_seq.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let seq = seq_counter.fetch_add(1, Ordering::Relaxed);
                let packet = build_keepalive_packet(seq);
                if socket.send(&packet).await.is_err() {
                    break;
                }
            }
        })
    }
}

/// Metade de recepção da conexão de voz.
///
/// Decifra pacotes RTP e filtra transparentemente:
/// - Pacotes de keepalive (8 bytes);
/// - Pacotes RTCP (RFC 3550, tipos de carga 200..=204 no byte 1);
/// - Pacotes não-RTP v2.
pub struct VoiceUdpReceiver {
    socket: Arc<UdpSocket>,
    crypto: Arc<Mutex<RtpCrypto>>,
}

impl VoiceUdpReceiver {
    /// Recebe o próximo pacote de voz entregando todos os metadados:
    /// `ssrc`, `sequence`, `timestamp` e `payload` Opus decifrado.
    pub async fn recv_packet(&self) -> Result<VoicePacket> {
        let mut buf = [0u8; 2048];
        loop {
            let len = self.socket.recv(&mut buf).await?;

            // 1. Descartar pacotes menores que 12 bytes.
            if len < 12 {
                // Se for pacote de keepalive (8 bytes), ignorar silenciosamente
                if len == KEEPALIVE_PACKET_LEN {
                    log::trace!("Keepalive UDP recebido (8 bytes), ignorado");
                }
                continue;
            }

            // 2. Descartar pacotes que não são RTP v2 (versão deve ser 2 -> 0x80)
            if (buf[0] & 0xC0) != 0x80 {
                continue;
            }

            // 3. Filtrar explicitamente pacotes RTCP (RFC 3550 seção 6: tipos 200 a 204 no byte 1).
            // SR=200, RR=201, SDES=202, BYE=203, APP=204
            let pt = buf[1];
            if (200..=204).contains(&pt) {
                log::trace!("Pacote RTCP recebido (tipo {pt}), ignorado explicitamente");
                continue;
            }

            // 4. Decifrar a carga útil e extrair metadados
            let crypto = self.crypto.lock().await;
            match crypto.decrypt_full(&buf[..len]) {
                Ok((info, opus)) => {
                    return Ok(VoicePacket {
                        ssrc: info.ssrc,
                        sequence: info.sequence,
                        timestamp: info.timestamp,
                        payload: opus,
                    });
                }
                Err(err) => {
                    log::debug!("Descartando pacote UDP que falhou na decifração: {err}");
                    continue;
                }
            }
        }
    }

    /// Recebe o próximo pacote com timeout configurável.
    pub async fn recv_packet_timeout(&self, timeout: Duration) -> Result<VoicePacket> {
        tokio::time::timeout(timeout, self.recv_packet())
            .await
            .map_err(|_| anyhow!("Timeout ao aguardar pacote de voz Opus sobre UDP"))?
    }

    /// Retorna tupla com `(ssrc, sequence, timestamp, opus_payload)`.
    pub async fn recv_opus_packet(&self) -> Result<(u32, u16, u32, Vec<u8>)> {
        let pkt = self.recv_packet().await?;
        Ok((pkt.ssrc, pkt.sequence, pkt.timestamp, pkt.payload))
    }

    /// Mantido por compatibilidade estrita com o contrato original:
    /// Retorna tupla `(ssrc, opus_payload)`.
    pub async fn recv_opus_frame(&self) -> Result<(u32, Vec<u8>)> {
        let pkt = self.recv_packet().await?;
        Ok((pkt.ssrc, pkt.payload))
    }

    /// Recebe quadro com timeout (compatibilidade).
    pub async fn recv_opus_frame_timeout(&self, timeout: Duration) -> Result<(u32, Vec<u8>)> {
        let pkt = self.recv_packet_timeout(timeout).await?;
        Ok((pkt.ssrc, pkt.payload))
    }
}

// ============================================================================
// 6. Conexão Principal Unificada (VoiceUdpConnection)
// ============================================================================

/// Conexão de transporte UDP unificada.
///
/// Delega as operações de envio para [`VoiceUdpSender`] e de recepção para [`VoiceUdpReceiver`],
/// garantindo ausência total de duplicação de lógica.
pub struct VoiceUdpConnection {
    sender: VoiceUdpSender,
    receiver: VoiceUdpReceiver,
}

impl VoiceUdpConnection {
    /// Estabelece uma conexão direta conectando o socket e aplicando a chave criptográfica.
    pub async fn connect(ip: &str, port: u16, ssrc: u32, crypto: RtpCrypto) -> Result<Self> {
        let sock = VoiceUdpSocket::connect(ip, port, ssrc).await?;
        Ok(sock.into_connection(crypto))
    }

    /// Cria a conexão a partir de um `UdpSocket` já existente.
    pub fn from_socket(socket: UdpSocket, remote_addr: SocketAddr, ssrc: u32, crypto: RtpCrypto) -> Self {
        Self::from_shared_socket(Arc::new(socket), remote_addr, ssrc, crypto)
    }

    /// Cria a conexão a partir de um `Arc<UdpSocket>` compartilhado.
    pub fn from_shared_socket(
        socket: Arc<UdpSocket>,
        remote_addr: SocketAddr,
        ssrc: u32,
        crypto: RtpCrypto,
    ) -> Self {
        let crypto_arc = Arc::new(Mutex::new(crypto));
        let keepalive_seq = Arc::new(AtomicU64::new(0));

        let sender = VoiceUdpSender {
            socket: socket.clone(),
            remote_addr,
            ssrc,
            sequence: 0,
            timestamp: 0,
            crypto: crypto_arc.clone(),
            keepalive_seq,
        };

        let receiver = VoiceUdpReceiver {
            socket,
            crypto: crypto_arc,
        };

        Self { sender, receiver }
    }

    /// SSRC configurado na conexão.
    pub fn ssrc(&self) -> u32 {
        self.sender.ssrc()
    }

    /// Sequência atual do transmissor.
    pub fn sequence(&self) -> u16 {
        self.sender.sequence()
    }

    /// Timestamp atual do transmissor.
    pub fn timestamp(&self) -> u32 {
        self.sender.timestamp()
    }

    /// Endereço remoto do servidor de voz.
    pub fn remote_addr(&self) -> SocketAddr {
        self.sender.remote_addr()
    }

    /// Retorna uma referência ao socket UDP subjacente.
    pub fn socket(&self) -> &Arc<UdpSocket> {
        self.sender.socket()
    }

    /// Envia um quadro de áudio Opus codificado (`&[u8]`).
    pub async fn send_opus_frame(&mut self, opus_data: &[u8]) -> Result<()> {
        self.sender.send_opus_frame(opus_data).await
    }

    /// Envia os 5 quadros canônicos de silêncio (`0xF8, 0xFF, 0xFE`).
    pub async fn send_silence_frames(&mut self) -> Result<()> {
        self.sender.send_silence_frames().await
    }

    /// Atalho para cessar fala enviando silêncio.
    pub async fn stop_speaking(&mut self) -> Result<()> {
        self.sender.stop_speaking().await
    }

    /// Envia um pacote de keepalive UDP (8 bytes).
    pub async fn send_keepalive(&self) -> Result<()> {
        self.sender.send_keepalive().await
    }

    /// Dispara a tarefa periódica de keepalive UDP a cada `interval` (ex: 5s).
    pub fn spawn_keepalive_task(&self, interval: Duration) -> tokio::task::JoinHandle<()> {
        self.sender.spawn_keepalive_task(interval)
    }

    /// Recebe o próximo pacote com todos os metadados (`VoicePacket`).
    pub async fn recv_packet(&self) -> Result<VoicePacket> {
        self.receiver.recv_packet().await
    }

    /// Recebe o próximo pacote com timeout configurável.
    pub async fn recv_packet_timeout(&self, timeout: Duration) -> Result<VoicePacket> {
        self.receiver.recv_packet_timeout(timeout).await
    }

    /// Recebe tupla com `(ssrc, sequence, timestamp, opus_payload)`.
    pub async fn recv_opus_packet(&self) -> Result<(u32, u16, u32, Vec<u8>)> {
        self.receiver.recv_opus_packet().await
    }

    /// Mantido para compatibilidade com o contrato original: retorna `(ssrc, opus_payload)`.
    pub async fn recv_opus_frame(&self) -> Result<(u32, Vec<u8>)> {
        self.receiver.recv_opus_frame().await
    }

    /// Recebe quadro com timeout (compatibilidade).
    pub async fn recv_opus_frame_timeout(&self, timeout: Duration) -> Result<(u32, Vec<u8>)> {
        self.receiver.recv_opus_frame_timeout(timeout).await
    }

    /// Divide a conexão em metades concorrentes de envio e recepção.
    pub fn split(self) -> (VoiceUdpSender, VoiceUdpReceiver) {
        (self.sender, self.receiver)
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
        assert_eq!(&pkt[0..2], &1u16.to_be_bytes());
        assert_eq!(&pkt[2..4], &70u16.to_be_bytes());
        assert_eq!(&pkt[4..8], &ssrc.to_be_bytes());
        assert_eq!(&pkt[8..72], &[0u8; 64]);
        assert_eq!(&pkt[72..74], &[0u8; 2]);
    }

    #[test]
    fn test_parse_ip_discovery_response() {
        let mut resp = [0u8; 74];
        resp[0..2].copy_from_slice(&2u16.to_be_bytes());
        resp[2..4].copy_from_slice(&70u16.to_be_bytes());
        resp[4..8].copy_from_slice(&0x12345678u32.to_be_bytes());

        let ip_bytes = b"203.0.113.195";
        resp[8..8 + ip_bytes.len()].copy_from_slice(ip_bytes);
        resp[8 + ip_bytes.len()] = 0;

        resp[72..74].copy_from_slice(&54321u16.to_be_bytes());

        let (ip, port) = parse_ip_discovery_response(&resp).unwrap();
        assert_eq!(ip, "203.0.113.195");
        assert_eq!(port, 54321);
    }

    #[test]
    fn test_build_keepalive_packet() {
        let seq = 42u64;
        let pkt = build_keepalive_packet(seq);
        assert_eq!(pkt.len(), 8);
        assert_eq!(u64::from_le_bytes(pkt), 42);
    }

    #[tokio::test]
    async fn test_two_phase_handshake_flow() {
        // Simulação do servidor UDP do Discord em loopback
        let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_socket.local_addr().unwrap();

        let ssrc: u32 = 0xABCD1234;

        // Tarefa servidora
        let server_handle = tokio::spawn(async move {
            let mut buf = [0u8; 128];
            // 1. Recebe requisição de IP Discovery (74 bytes)
            let (n, client_addr) = server_socket.recv_from(&mut buf).await.unwrap();
            assert_eq!(n, 74);

            // Responde IP Discovery
            let mut resp = [0u8; 74];
            resp[0..2].copy_from_slice(&2u16.to_be_bytes());
            resp[2..4].copy_from_slice(&70u16.to_be_bytes());
            resp[4..8].copy_from_slice(&ssrc.to_be_bytes());
            let ip = b"198.51.100.99";
            resp[8..8 + ip.len()].copy_from_slice(ip);
            resp[8 + ip.len()] = 0;
            resp[72..74].copy_from_slice(&55555u16.to_be_bytes());
            server_socket.send_to(&resp, client_addr).await.unwrap();

            // 2. Recebe quadro de voz cifrado
            let mut voice_buf = [0u8; 256];
            let (vn, _) = server_socket.recv_from(&mut voice_buf).await.unwrap();
            assert!(vn > 12);
            voice_buf[..vn].to_vec()
        });

        // Fase 1: abre VoiceUdpSocket e faz discovery no MESMO socket
        let voice_sock = VoiceUdpSocket::connect("127.0.0.1", server_addr.port(), ssrc)
            .await
            .unwrap();

        let (ext_ip, ext_port) = voice_sock.discover_ip().await.unwrap();
        assert_eq!(ext_ip, "198.51.100.99");
        assert_eq!(ext_port, 55555);

        // Fase 2: com a chegada da Session Description, converte para VoiceUdpConnection
        let key = test_secret_key();
        let crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
        let mut conn = voice_sock.into_connection(crypto);

        let test_opus = b"testing two phase handshake voice transmission";
        conn.send_opus_frame(test_opus).await.unwrap();

        let received_by_server = server_handle.await.unwrap();
        assert!(received_by_server.len() > 12);

        // Decifrar o pacote que o servidor recebeu
        let crypto_verify = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
        let (rx_ssrc, decrypted) = crypto_verify.decrypt_packet(&received_by_server).unwrap();
        assert_eq!(rx_ssrc, ssrc);
        assert_eq!(decrypted, test_opus);
    }

    #[tokio::test]
    async fn test_rtcp_packets_and_keepalive_ignored_in_receiver() {
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

        // 1. Enviar um keepalive de 8 bytes
        sender.send_keepalive().await.unwrap();

        // 2. Enviar pacotes RTCP (payload types 200 Sender Report e 201 Receiver Report)
        let mut rtcp_sr = [0u8; 28];
        rtcp_sr[0] = 0x80;
        rtcp_sr[1] = 200; // RTCP SR
        sender.socket().send(&rtcp_sr).await.unwrap();

        let mut rtcp_rr = [0u8; 32];
        rtcp_rr[0] = 0x80;
        rtcp_rr[1] = 201; // RTCP RR
        sender.socket().send(&rtcp_rr).await.unwrap();

        // 3. Enviar um quadro Opus legítimo
        let opus_data = b"legitimate opus audio packet after rtcp and keepalive";
        sender.send_opus_frame(opus_data).await.unwrap();

        // O receiver deve ignorar o keepalive (8 bytes) e os dois pacotes RTCP (200 e 201),
        // entregando diretamente o quadro Opus com sequence=0 e timestamp=0!
        let packet = receiver
            .recv_packet_timeout(Duration::from_millis(500))
            .await
            .unwrap();

        assert_eq!(packet.ssrc, ssrc);
        assert_eq!(packet.sequence, 0);
        assert_eq!(packet.timestamp, 0);
        assert_eq!(packet.payload, opus_data);
    }
}
