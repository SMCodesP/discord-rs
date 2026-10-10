//! Integration and Unit Tests for Discord Voice UDP Transport & Cryptography (Fase 8.2).
//!
//! Cobre:
//! 1. IP Discovery: montagem, parsing e handshake sobre socket UDP 127.0.0.1.
//! 2. RTP: cabeçalho de 12 bytes, sequência, timestamp (incremento de 960 amostras), e extensões.
//! 3. Criptografia AEAD: `aead_aes256_gcm_rtpsize` e `aead_xchacha20_poly1305_rtpsize`, ida e volta,
//!    e rejeição estrita de adulteração (cabeçalho, payload, tag, nonce).
//! 4. Conexão de voz: envio de quadros Opus, envio de 5 quadros de silêncio (0xF8, 0xFF, 0xFE) ao cessar fala,
//!    e entrega com o SSRC de origem.

use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;

use discord_rs::protocol::{
    build_ip_discovery_packet, build_rtp_header, discover_ip, parse_ip_discovery_response,
    parse_rtp_aad_and_ext, EncryptionMode, RtpCrypto, VoiceUdpConnection,
    IP_DISCOVERY_PACKET_LEN, OPUS_SAMPLES_PER_FRAME, SILENCE_FRAME, SILENCE_FRAME_COUNT,
};

fn sample_secret_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    for (i, b) in key.iter_mut().enumerate() {
        *b = ((i * 13 + 37) % 256) as u8;
    }
    key
}

// ============================================================================
// 1. Testes de IP Discovery
// ============================================================================

#[test]
fn test_ip_discovery_packet_structure() {
    let ssrc = 0x1337BEEF;
    let packet = build_ip_discovery_packet(ssrc);

    assert_eq!(packet.len(), IP_DISCOVERY_PACKET_LEN);
    // Tipo de requisição = 1 (big-endian)
    assert_eq!(u16::from_be_bytes([packet[0], packet[1]]), 1);
    // Comprimento do corpo = 70 (big-endian)
    assert_eq!(u16::from_be_bytes([packet[2], packet[3]]), 70);
    // SSRC (big-endian)
    assert_eq!(u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]), ssrc);
    // 64 bytes de endereço inicializados com zero
    assert!(packet[8..72].iter().all(|&b| b == 0));
    // 2 bytes de porta inicializados com zero
    assert_eq!(packet[72], 0);
    assert_eq!(packet[73], 0);
}

#[test]
fn test_ip_discovery_response_parsing() {
    let mut response = [0u8; 74];
    response[0..2].copy_from_slice(&2u16.to_be_bytes()); // Type 2 = Response
    response[2..4].copy_from_slice(&70u16.to_be_bytes()); // Length 70
    response[4..8].copy_from_slice(&0xCAFEBABEu32.to_be_bytes()); // SSRC

    // Endereço IP terminado em nulo
    let ip_str = b"177.136.240.12";
    response[8..8 + ip_str.len()].copy_from_slice(ip_str);
    response[8 + ip_str.len()] = 0; // null-terminated

    // Porta UDP 65432
    response[72..74].copy_from_slice(&65432u16.to_be_bytes());

    let (parsed_ip, parsed_port) = parse_ip_discovery_response(&response).expect("parsing válido");
    assert_eq!(parsed_ip, "177.136.240.12");
    assert_eq!(parsed_port, 65432);
}

#[tokio::test]
async fn test_ip_discovery_loopback_socket_flow() {
    let ssrc = 0x42424242;

    // Servidor simulador em loopback
    let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_socket.local_addr().unwrap();

    // Cliente conectando ao servidor
    let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client_socket.connect(server_addr).await.unwrap();

    let server_task = tokio::spawn(async move {
        let mut buf = [0u8; 128];
        let (n, client_addr) = server_socket.recv_from(&mut buf).await.unwrap();
        assert_eq!(n, 74);
        assert_eq!(u16::from_be_bytes([buf[0], buf[1]]), 1);
        assert_eq!(u16::from_be_bytes([buf[2], buf[3]]), 70);
        assert_eq!(u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]), ssrc);

        // Responder com IP público fictício do cliente
        let mut resp = [0u8; 74];
        resp[0..2].copy_from_slice(&2u16.to_be_bytes());
        resp[2..4].copy_from_slice(&70u16.to_be_bytes());
        resp[4..8].copy_from_slice(&ssrc.to_be_bytes());
        let public_ip = b"200.180.99.10";
        resp[8..8 + public_ip.len()].copy_from_slice(public_ip);
        resp[8 + public_ip.len()] = 0;
        resp[72..74].copy_from_slice(&51820u16.to_be_bytes());

        server_socket.send_to(&resp, client_addr).await.unwrap();
    });

    let (discovered_ip, discovered_port) = discover_ip(&client_socket, ssrc).await.unwrap();
    server_task.await.unwrap();

    assert_eq!(discovered_ip, "200.180.99.10");
    assert_eq!(discovered_port, 51820);
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
    assert_eq!(header[0], 0x80); // Version 2, sem padding, sem extensão, CC=0
    assert_eq!(header[1], 0x78); // Payload type 120 (Opus)
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
    // Pacote com 1 CSRC (CC=1 -> byte 0 bits 0..3 = 1) e Extensão (bit 4 = 1 -> 0x91)
    let mut raw = Vec::new();
    raw.push(0x91); // V=2, X=1, CC=1
    raw.push(0x78); // PT=120
    raw.extend_from_slice(&250u16.to_be_bytes()); // Seq
    raw.extend_from_slice(&48000u32.to_be_bytes()); // Timestamp
    raw.extend_from_slice(&0x11111111u32.to_be_bytes()); // SSRC

    // 1 CSRC (4 bytes)
    raw.extend_from_slice(&0x22222222u32.to_be_bytes());

    // Preâmbulo da extensão (4 bytes): profile 0xBEDE, length = 2 words (8 bytes de extensão)
    raw.extend_from_slice(&0xBEDEu16.to_be_bytes());
    raw.extend_from_slice(&2u16.to_be_bytes());

    let info = parse_rtp_aad_and_ext(&raw).expect("parsing de extensão e CSRC");
    // AAD = 12 (base) + 4 (1 CSRC) + 4 (preâmbulo de extensão) = 20 bytes
    assert_eq!(info.aad_len, 20);
    // Extensão tem 2 words * 4 = 8 bytes
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

    // 1. Verifica estrutura do pacote cifrado
    assert_eq!(&encrypted[..12], &header); // AAD / Header em claro no início
    assert_eq!(encrypted.len(), 12 + opus_payload.len() + 16 + 4); // header + ciphertext + tag(16) + nonce(4)
    assert_eq!(&encrypted[encrypted.len() - 4..], &[0, 0, 0, 0]); // Nonce counter inicial = 0

    // 2. Decifração bem-sucedida
    let (origin_ssrc, decrypted) = crypto.decrypt_packet(&encrypted).unwrap();
    assert_eq!(origin_ssrc, 0x12345678);
    assert_eq!(decrypted, opus_payload);

    // 3. Rejeição de Adulteração: modificado no cabeçalho (AAD)
    let mut tampered_aad = encrypted.clone();
    tampered_aad[2] ^= 0x01; // altera número de sequência
    assert!(
        crypto.decrypt(&tampered_aad).is_err(),
        "Pacote com AAD adulterado deve ser rejeitado"
    );

    // 4. Rejeição de Adulteração: modificado na carga útil cifrada
    let mut tampered_payload = encrypted.clone();
    tampered_payload[15] ^= 0xFF;
    assert!(
        crypto.decrypt(&tampered_payload).is_err(),
        "Pacote com ciphertext adulterado deve ser rejeitado"
    );

    // 5. Rejeição de Adulteração: modificado no nonce de 4 bytes
    let mut tampered_nonce = encrypted.clone();
    let last_idx = tampered_nonce.len() - 1;
    tampered_nonce[last_idx] ^= 0x01;
    assert!(
        crypto.decrypt(&tampered_nonce).is_err(),
        "Pacote com nonce adulterado deve ser rejeitado"
    );
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

    // Rejeição de adulteração
    let mut tampered = encrypted.clone();
    tampered[14] ^= 0x42;
    assert!(crypto.decrypt(&tampered).is_err());
}

#[test]
fn test_crypto_rtpsize_with_header_extension_stripping() {
    let key = sample_secret_key();
    let mut crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();

    // Monta pacote com cabeçalho de extensão:
    // 12 bytes base + 4 bytes preâmbulo (AAD = 16 bytes)
    // 4 bytes de carga de extensão + áudio Opus
    let mut unencrypted = Vec::new();
    unencrypted.push(0x90); // V=2, X=1, CC=0
    unencrypted.push(0x78);
    unencrypted.extend_from_slice(&10u16.to_be_bytes());
    unencrypted.extend_from_slice(&9600u32.to_be_bytes());
    unencrypted.extend_from_slice(&0xFEEDBEEFu32.to_be_bytes());

    // Preâmbulo: Profile 0x1000, 1 word de extensão = 4 bytes
    unencrypted.extend_from_slice(&0x1000u16.to_be_bytes());
    unencrypted.extend_from_slice(&1u16.to_be_bytes());

    // Carga da extensão (4 bytes que devem ser cifrados e removidos na decifração)
    unencrypted.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);

    // Áudio Opus puro
    let raw_opus = b"real opus payload that will be returned clean";
    unencrypted.extend_from_slice(raw_opus);

    // Cifra o pacote completo
    let encrypted = crypto.encrypt_packet(&unencrypted).unwrap();

    // O AAD em claro compreende os primeiros 16 bytes (12 bytes base + 4 bytes preâmbulo)
    assert_eq!(&encrypted[..16], &unencrypted[..16]);

    // Ao decifrar, a carga da extensão deve ser descartada e apenas o Opus deve ser retornado
    let (ssrc, decrypted) = crypto.decrypt_packet(&encrypted).unwrap();
    assert_eq!(ssrc, 0xFEEDBEEF);
    assert_eq!(decrypted, raw_opus);
}

// ============================================================================
// 4. Testes de VoiceUdpConnection e Quadros de Silêncio
// ============================================================================

#[tokio::test]
async fn test_voice_connection_send_recv_and_silence_frames() {
    let key = sample_secret_key();
    let crypto_sender = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();
    let crypto_receiver = RtpCrypto::new("aead_aes256_gcm_rtpsize", &key).unwrap();

    let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr: SocketAddr = server_socket.local_addr().unwrap();

    let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let client_addr: SocketAddr = client_socket.local_addr().unwrap();

    client_socket.connect(server_addr).await.unwrap();
    server_socket.connect(client_addr).await.unwrap();

    let ssrc = 0x55443322;
    let mut sender = VoiceUdpConnection::from_socket(client_socket, server_addr, ssrc, crypto_sender);
    let receiver = VoiceUdpConnection::from_socket(server_socket, client_addr, ssrc, crypto_receiver);

    // 1. Enviar quadro Opus de fala ativo
    let speech_frame = b"user speaking voice frame 20ms audio";
    sender.send_opus_frame(speech_frame).await.unwrap();

    assert_eq!(sender.sequence(), 1);
    assert_eq!(sender.timestamp(), OPUS_SAMPLES_PER_FRAME);

    let (origin_ssrc, received_opus) = receiver
        .recv_opus_frame_timeout(Duration::from_millis(500))
        .await
        .unwrap();

    assert_eq!(origin_ssrc, ssrc);
    assert_eq!(received_opus, speech_frame);

    // 2. Parar de falar: deve enviar os 5 quadros canônicos de silêncio (0xF8, 0xFF, 0xFE)
    sender.stop_speaking().await.unwrap();

    // Sequência deve ter avançado +5 (total 6) e timestamp +5*960 (total 6*960)
    assert_eq!(sender.sequence(), 6);
    assert_eq!(sender.timestamp(), OPUS_SAMPLES_PER_FRAME * 6);

    for _ in 0..SILENCE_FRAME_COUNT {
        let (silence_ssrc, silence_opus) = receiver
            .recv_opus_frame_timeout(Duration::from_millis(500))
            .await
            .unwrap();
        assert_eq!(silence_ssrc, ssrc);
        assert_eq!(silence_opus, SILENCE_FRAME);
    }
}
