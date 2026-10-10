//! Comprehensive Integration and Unit Tests for Discord Voice UDP & Cryptography (Fase 8.2).
//!
//! Cobre:
//! 1. IP Discovery: montagem de pacote de 74 bytes, parsing de resposta e handshake sobre socket UDP 127.0.0.1.
//! 2. Handshake em Duas Fases: `VoiceUdpSocket::connect` -> `discover_ip()` -> `into_connection(crypto)` no MESMO socket.
//! 3. RTP: cabeçalho canônico de 12 bytes, incremento de sequência (+1) e timestamp (+960 amostras), e extensões.
//! 4. Criptografia AEAD: `aead_aes256_gcm_rtpsize` e `aead_xchacha20_poly1305_rtpsize`, ida e volta,
//!    e rejeição estrita de adulteração (cabeçalho, payload, tag, nonce).
//! 5. Keepalive UDP: envio de pacote de 8 bytes (contador incremental little-endian) e descarte transparente na recepção.
//! 6. Filtragem RTCP: pacotes RTCP (payload types 200-204) explicitamente ignorados na recepção.
//! 7. Recepção com Metadados: entrega completa de `ssrc`, `sequence`, `timestamp` e `payload` Opus para o mixer.
//! 8. Silêncio Opus: envio dos 5 quadros canônicos de silêncio (0xF8, 0xFF, 0xFE) ao parar de falar.

use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;

use discord_rs::protocol::{
    build_ip_discovery_packet, build_keepalive_packet, build_rtp_header, discover_ip,
    parse_ip_discovery_response, parse_rtp_aad_and_ext, EncryptionMode, RtpCrypto,
    VoicePacket, VoiceUdpConnection, VoiceUdpSocket, IP_DISCOVERY_PACKET_LEN,
    KEEPALIVE_PACKET_LEN, OPUS_SAMPLES_PER_FRAME, SILENCE_FRAME, SILENCE_FRAME_COUNT,
};

fn sample_secret_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    for (i, b) in key.iter_mut().enumerate() {
        *b = ((i * 13 + 37) % 256) as u8;
    }
    key
}

// ============================================================================
// 1. IP Discovery e Handshake em Duas Fases
// ============================================================================

#[test]
fn test_ip_discovery_packet_structure() {
    let ssrc = 0x1337BEEF;
    let packet = build_ip_discovery_packet(ssrc);

    assert_eq!(packet.len(), IP_DISCOVERY_PACKET_LEN);
    assert_eq!(u16::from_be_bytes([packet[0], packet[1]]), 1);
    assert_eq!(u16::from_be_bytes([packet[2], packet[3]]), 70);
    assert_eq!(u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]), ssrc);
    assert!(packet[8..72].iter().all(|&b| b == 0));
    assert_eq!(packet[72], 0);
    assert_eq!(packet[73], 0);
}

#[test]
fn test_ip_discovery_response_parsing() {
    let mut response = [0u8; 74];
    response[0..2].copy_from_slice(&2u16.to_be_bytes());
    response[2..4].copy_from_slice(&70u16.to_be_bytes());
    response[4..8].copy_from_slice(&0xCAFEBABEu32.to_be_bytes());

    let ip_str = b"177.136.240.12";
    response[8..8 + ip_str.len()].copy_from_slice(ip_str);
    response[8 + ip_str.len()] = 0;

    response[72..74].copy_from_slice(&65432u16.to_be_bytes());

    let (parsed_ip, parsed_port) = parse_ip_discovery_response(&response).expect("parsing válido");
    assert_eq!(parsed_ip, "177.136.240.12");
    assert_eq!(parsed_port, 65432);
}

#[tokio::test]
async fn test_two_phase_handshake_same_socket_flow() {
    let ssrc = 0x88776655;

    let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_socket.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let mut buf = [0u8; 128];
        // 1. Recebe IP Discovery
        let (n, client_addr) = server_socket.recv_from(&mut buf).await.unwrap();
        assert_eq!(n, 74);
        assert_eq!(u16::from_be_bytes([buf[0], buf[1]]), 1);
        assert_eq!(u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]), ssrc);

        // Responde IP Discovery
        let mut resp = [0u8; 74];
        resp[0..2].copy_from_slice(&2u16.to_be_bytes());
        resp[2..4].copy_from_slice(&70u16.to_be_bytes());
        resp[4..8].copy_from_slice(&ssrc.to_be_bytes());
        let public_ip = b"189.40.120.5";
        resp[8..8 + public_ip.len()].copy_from_slice(public_ip);
        resp[8 + public_ip.len()] = 0;
        resp[72..74].copy_from_slice(&51234u16.to_be_bytes());
        server_socket.send_to(&resp, client_addr).await.unwrap();

        // 2. Recebe pacote de áudio no MESMO socket/endereço do discovery
        let mut audio_buf = [0u8; 256];
        let (an, from) = server_socket.recv_from(&mut audio_buf).await.unwrap();
        assert_eq!(from, client_addr, "O envio deve vir da mesma porta/mapeamento NAT do discovery");
        audio_buf[..an].to_vec()
    });

    // Fase 1: VoiceUdpSocket::connect e discover_ip()
    let voice_sock = VoiceUdpSocket::connect("127.0.0.1", server_addr.port(), ssrc)
        .await
        .unwrap();

    let (disc_ip, disc_port) = voice_sock.discover_ip().await.unwrap();
    assert_eq!(disc_ip, "189.40.120.5");
    assert_eq!(disc_port, 51234);

    // Fase 2: Recebendo a Session Description, converte o mesmo socket para VoiceUdpConnection
    let key = sample_secret_key();
    let crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
    let mut connection = voice_sock.into_connection(crypto);

    let test_opus = b"testing seamless two-phase socket transition";
    connection.send_opus_frame(test_opus).await.unwrap();

    let packet_at_server = server_task.await.unwrap();
    assert!(packet_at_server.len() > 12);

    let verify_crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
    let (rx_ssrc, decrypted) = verify_crypto.decrypt_packet(&packet_at_server).unwrap();
    assert_eq!(rx_ssrc, ssrc);
    assert_eq!(decrypted, test_opus);
}

#[tokio::test]
async fn test_discover_ip_contract_standalone() {
    let ssrc: u32 = 0x55667788;
    let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_socket.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let mut buf = [0u8; 128];
        let (n, client_addr) = server_socket.recv_from(&mut buf).await.unwrap();
        assert_eq!(n, 74);

        let mut resp = [0u8; 74];
        resp[0..2].copy_from_slice(&2u16.to_be_bytes());
        resp[2..4].copy_from_slice(&70u16.to_be_bytes());
        resp[4..8].copy_from_slice(&ssrc.to_be_bytes());
        let public_ip = b"203.0.113.88";
        resp[8..8 + public_ip.len()].copy_from_slice(public_ip);
        resp[8 + public_ip.len()] = 0;
        resp[72..74].copy_from_slice(&43210u16.to_be_bytes());
        server_socket.send_to(&resp, client_addr).await.unwrap();
    });

    let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client_socket.connect(server_addr).await.unwrap();

    let (ip, port) = discover_ip(&client_socket, ssrc).await.unwrap();
    server_task.await.unwrap();

    assert_eq!(ip, "203.0.113.88");
    assert_eq!(port, 43210);
}

// ============================================================================
// 2. Testes de RTP
// ============================================================================

#[test]
fn test_rtp_header_canonical_opus_format() {
    let seq = 100;
    let ts = 960 * 10;
    let ssrc = 0x99887766;

    let header = build_rtp_header(seq, ts, ssrc);
    assert_eq!(header.len(), 12);
    assert_eq!(header[0], 0x80);
    assert_eq!(header[1], 0x78);
    assert_eq!(u16::from_be_bytes([header[2], header[3]]), seq);
    assert_eq!(u32::from_be_bytes([header[4], header[5], header[6], header[7]]), ts);
    assert_eq!(u32::from_be_bytes([header[8], header[9], header[10], header[11]]), ssrc);

    let info = parse_rtp_aad_and_ext(&header).expect("parsing de cabeçalho válido");
    assert_eq!(info.aad_len, 12);
    assert_eq!(info.ext_payload_bytes, 0);
    assert_eq!(info.sequence, seq);
    assert_eq!(info.timestamp, ts);
    assert_eq!(info.ssrc, ssrc);
    assert_eq!(info.payload_type, 0x78);
    assert!(!info.has_extension);
}

#[test]
fn test_rtp_header_with_csrc_and_extension() {
    let mut raw = Vec::new();
    raw.push(0x91); // V=2, X=1, CC=1
    raw.push(0x78); // PT=120
    raw.extend_from_slice(&250u16.to_be_bytes());
    raw.extend_from_slice(&48000u32.to_be_bytes());
    raw.extend_from_slice(&0x11111111u32.to_be_bytes());

    raw.extend_from_slice(&0x22222222u32.to_be_bytes());

    raw.extend_from_slice(&0xBEDEu16.to_be_bytes());
    raw.extend_from_slice(&2u16.to_be_bytes());

    let info = parse_rtp_aad_and_ext(&raw).expect("parsing de extensão e CSRC");
    assert_eq!(info.aad_len, 20);
    assert_eq!(info.ext_payload_bytes, 8);
    assert!(info.has_extension);
    assert_eq!(info.ssrc, 0x11111111);
}

// ============================================================================
// 3. Testes de Criptografia AEAD e Rejeição de Adulteração
// ============================================================================

#[test]
fn test_crypto_aes256_gcm_roundtrip_and_tamper_rejection() {
    let key = sample_secret_key();
    let mut crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
    assert_eq!(crypto.mode(), EncryptionMode::AeadAes256GcmRtpsize);

    let header = build_rtp_header(1, 960, 0x12345678);
    let opus_payload = b"test audio opus frame for aes-gcm encryption 20ms duration";

    let encrypted = crypto.encrypt(&header, opus_payload).unwrap();

    assert_eq!(&encrypted[..12], &header);
    assert_eq!(encrypted.len(), 12 + opus_payload.len() + 16 + 4);
    assert_eq!(&encrypted[encrypted.len() - 4..], &[0, 0, 0, 0]);

    let (origin_ssrc, decrypted) = crypto.decrypt_packet(&encrypted).unwrap();
    assert_eq!(origin_ssrc, 0x12345678);
    assert_eq!(decrypted, opus_payload);

    // Rejeição de adulteração
    let mut tampered_aad = encrypted.clone();
    tampered_aad[2] ^= 0x01;
    assert!(crypto.decrypt(&tampered_aad).is_err());

    let mut tampered_payload = encrypted.clone();
    tampered_payload[15] ^= 0xFF;
    assert!(crypto.decrypt(&tampered_payload).is_err());

    let mut tampered_nonce = encrypted.clone();
    let last_idx = tampered_nonce.len() - 1;
    tampered_nonce[last_idx] ^= 0x01;
    assert!(crypto.decrypt(&tampered_nonce).is_err());
}

#[test]
fn test_crypto_xchacha20_poly1305_roundtrip_and_tamper_rejection() {
    let key = sample_secret_key();
    let mut crypto = RtpCrypto::new("aead_xchacha20_poly1305_rtpsize", &key).unwrap();
    assert_eq!(crypto.mode(), EncryptionMode::AeadXChaCha20Poly1305Rtpsize);

    let header = build_rtp_header(77, 960 * 77, 0x98765432);
    let opus_payload = b"another voice frame encrypted using xchacha20 poly1305 rtpsize";

    let encrypted = crypto.encrypt(&header, opus_payload).unwrap();

    assert_eq!(&encrypted[..12], &header);
    assert_eq!(encrypted.len(), 12 + opus_payload.len() + 16 + 4);

    let (origin_ssrc, decrypted) = crypto.decrypt_packet(&encrypted).unwrap();
    assert_eq!(origin_ssrc, 0x98765432);
    assert_eq!(decrypted, opus_payload);

    let mut tampered = encrypted.clone();
    tampered[14] ^= 0x42;
    assert!(crypto.decrypt(&tampered).is_err());
}

#[test]
fn test_crypto_rtpsize_with_header_extension_stripping() {
    let key = sample_secret_key();
    let mut crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();

    let mut unencrypted = Vec::new();
    unencrypted.push(0x90);
    unencrypted.push(0x78);
    unencrypted.extend_from_slice(&10u16.to_be_bytes());
    unencrypted.extend_from_slice(&9600u32.to_be_bytes());
    unencrypted.extend_from_slice(&0xFEEDBEEFu32.to_be_bytes());

    unencrypted.extend_from_slice(&0x1000u16.to_be_bytes());
    unencrypted.extend_from_slice(&1u16.to_be_bytes());

    unencrypted.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);

    let raw_opus = b"real opus payload that will be returned clean";
    unencrypted.extend_from_slice(raw_opus);

    let encrypted = crypto.encrypt_packet(&unencrypted).unwrap();
    assert_eq!(&encrypted[..16], &unencrypted[..16]);

    let (ssrc, decrypted) = crypto.decrypt_packet(&encrypted).unwrap();
    assert_eq!(ssrc, 0xFEEDBEEF);
    assert_eq!(decrypted, raw_opus);
}

// ============================================================================
// 4. Testes de Keepalive, RTCP Filtering e Entrega de Sequence/Timestamp
// ============================================================================

#[tokio::test]
async fn test_keepalive_packet_and_filtering() {
    let key = sample_secret_key();
    let crypto_sender = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
    let crypto_receiver = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();

    let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr: SocketAddr = server_socket.local_addr().unwrap();

    let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let client_addr: SocketAddr = client_socket.local_addr().unwrap();

    client_socket.connect(server_addr).await.unwrap();
    server_socket.connect(client_addr).await.unwrap();

    let ssrc = 0x77777777;
    let mut sender = VoiceUdpConnection::from_socket(client_socket, server_addr, ssrc, crypto_sender);
    let receiver = VoiceUdpConnection::from_socket(server_socket, client_addr, ssrc, crypto_receiver);

    // 1. Testa montagem do keepalive de 8 bytes
    let keepalive_bytes = build_keepalive_packet(10);
    assert_eq!(keepalive_bytes.len(), KEEPALIVE_PACKET_LEN);
    assert_eq!(u64::from_le_bytes(keepalive_bytes), 10);

    // 2. Emite keepalive
    sender.send_keepalive().await.unwrap();

    // 3. Emite pacotes RTCP para simular tráfego de controle do SFU (200=SR, 201=RR, 202=SDES, 203=BYE, 204=APP)
    for rtcp_type in 200..=204 {
        let mut rtcp_pkt = [0u8; 24];
        rtcp_pkt[0] = 0x80;
        rtcp_pkt[1] = rtcp_type;
        // Envia pacote RTCP direto pelo socket
        sender.socket().send(&rtcp_pkt).await.unwrap();
        // Emite outro keepalive intercalado
        sender.send_keepalive().await.unwrap();
    }

    // 4. Envia pacote de áudio Opus real
    let test_opus = b"real opus audio received cleanly despite keepalives and rtcp";
    sender.send_opus_frame(test_opus).await.unwrap();

    // 5. O receiver deve ignorar todos os keepalives e RTCPs e entregar o pacote Opus
    let VoicePacket {
        ssrc: rx_ssrc,
        sequence: rx_seq,
        timestamp: rx_ts,
        payload: rx_payload,
    } = receiver
        .recv_packet_timeout(Duration::from_millis(500))
        .await
        .unwrap();

    assert_eq!(rx_ssrc, ssrc);
    assert_eq!(rx_seq, 0);
    assert_eq!(rx_ts, 0);
    assert_eq!(rx_payload, test_opus);
}

#[tokio::test]
async fn test_voice_connection_sequence_timestamp_and_silence_frames() {
    let key = sample_secret_key();
    let crypto_sender = RtpCrypto::new("aead_xchacha20_poly1305_rtpsize", &key).unwrap();
    let crypto_receiver = RtpCrypto::new("aead_xchacha20_poly1305_rtpsize", &key).unwrap();

    let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_socket.local_addr().unwrap();

    let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let client_addr = client_socket.local_addr().unwrap();

    client_socket.connect(server_addr).await.unwrap();
    server_socket.connect(client_addr).await.unwrap();

    let ssrc = 0x12344321;
    let mut sender = VoiceUdpConnection::from_socket(client_socket, server_addr, ssrc, crypto_sender);
    let receiver = VoiceUdpConnection::from_socket(server_socket, client_addr, ssrc, crypto_receiver);

    // Enviar dois quadros consecutivos e verificar sequência/timestamp
    let frame1 = b"opus frame 1";
    let frame2 = b"opus frame 2";

    sender.send_opus_frame(frame1).await.unwrap();
    sender.send_opus_frame(frame2).await.unwrap();

    // Receiver consome frame 1
    let (s1, seq1, ts1, p1) = receiver.recv_opus_packet().await.unwrap();
    assert_eq!(s1, ssrc);
    assert_eq!(seq1, 0);
    assert_eq!(ts1, 0);
    assert_eq!(p1, frame1);

    // Receiver consome frame 2
    let (s2, seq2, ts2, p2) = receiver.recv_opus_packet().await.unwrap();
    assert_eq!(s2, ssrc);
    assert_eq!(seq2, 1);
    assert_eq!(ts2, OPUS_SAMPLES_PER_FRAME);
    assert_eq!(p2, frame2);

    // Parar de falar -> 5 quadros de silêncio (0xF8, 0xFF, 0xFE)
    sender.stop_speaking().await.unwrap();

    for i in 0..SILENCE_FRAME_COUNT {
        let (silence_ssrc, silence_seq, silence_ts, silence_payload) =
            receiver.recv_opus_packet().await.unwrap();
        assert_eq!(silence_ssrc, ssrc);
        assert_eq!(silence_seq, 2 + i as u16);
        assert_eq!(silence_ts, OPUS_SAMPLES_PER_FRAME * (2 + i as u32));
        assert_eq!(silence_payload, SILENCE_FRAME);
    }
}
