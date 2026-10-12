//! Discord Voice Call Probe (Ferramenta de Diagnóstico e Depuração de Chamadas).
//!
//! Conecta ao Gateway principal do Discord, autentica silenciosamente via token persistido
//! no app (carregado de `~/.config/discord-rs/auth.json` ou da variável `DISCORD_TOKEN`),
//! solicita entrada no canal de voz via Opcode 4 (Voice State Update), realiza a negociação
//! com o Voice Gateway v8 (incluindo DAVE MLS E2EE se suportado) e streaming RTP sobre UDP.
//!
//! # Uso:
//! ```bash
//! cargo run --example voice_call_probe -- <channel_id> [guild_id]
//! ```
//!
//! # Segurança:
//! O token NUNCA é passado via argumentos de linha de comando (visível em `ps`) e NUNCA
//! é impresso em stdout ou stderr.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use tokio::time::sleep;

use discord_rs::domain::User;
use discord_rs::media::devices::MediaDeviceManager;
use discord_rs::protocol::auth::TokenStorage;
use discord_rs::protocol::discord_api::DiscordApiClient;
use discord_rs::protocol::transport::DiscordTransport;
use discord_rs::protocol::voice_session::{
    AudioBridge, AudioBridgeCommand, VoiceSession, VoiceSessionConfig, VoiceSessionEvent,
    VoiceSessionState,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Inicializa logger padrão se RUST_LOG estiver configurado
    let _ = env_logger::try_init();

    println!("====================================================================");
    println!(" discord-rs Voice Call Probe (Diagnóstico de Voz & DAVE E2EE)");
    println!("====================================================================");

    // 1. Parser de argumentos CLI (apenas channel_id obrigatório e guild_id opcional)
    let mut args = std::env::args().skip(1);
    let channel_id_str = match args.next() {
        Some(arg) if !arg.trim().is_empty() => arg,
        _ => {
            eprintln!("Uso: cargo run --example voice_call_probe -- <channel_id> [guild_id]");
            eprintln!();
            eprintln!("Parâmetros:");
            eprintln!("  <channel_id> : ID numérico de 64 bits do canal de voz ou DM.");
            eprintln!("  [guild_id]   : (Opcional) ID da guilda/servidor se não for DM.");
            eprintln!();
            eprintln!("Autenticação:");
            eprintln!("  O token é lido silenciosamente de ~/.config/discord-rs/auth.json");
            eprintln!("  ou da variável de ambiente DISCORD_TOKEN.");
            std::process::exit(1);
        }
    };

    let channel_id: u64 = channel_id_str
        .parse()
        .context("channel_id inválido: deve ser um inteiro numérico de 64 bits (Snowflake)")?;

    let guild_id: Option<u64> = args.next().and_then(|s| s.parse().ok());

    println!("[PROBE] Alvo configurado:");
    println!("  Canal de Voz: {channel_id}");
    println!("  Guilda:       {:?}", guild_id);

    // 2. Leitura silenciosa do token de autenticação (Zero exposição em linha de comando ou stdout)
    let token = TokenStorage::load_from_env()
        .or_else(|| TokenStorage::default().load().ok().flatten())
        .context(
            "Nenhum token de autenticação encontrado. Faça login pelo cliente ou defina DISCORD_TOKEN.",
        )?;

    if !token.is_valid_format() {
        bail!("Token encontrado possui formato inválido (comprimento inferior ao padrão Discord).");
    }

    println!("[PROBE] Token de autenticação carregado com sucesso da memória/storage local.");

    // 3. Conexão ao Gateway principal do Discord
    let client = Arc::new(DiscordApiClient::new(token));
    println!("[PROBE] Conectando ao Gateway principal do Discord (WebSocket v10)...");

    client
        .connect()
        .await
        .context("Falha ao iniciar conexão com o Gateway principal")?;

    // Aguarda identificação e dados do usuário atual via REST
    let current_user: User = client
        .get_current_user()
        .await
        .context("Falha ao obter dados do usuário autenticado no Discord")?;

    let user_id: u64 = current_user
        .id
        .parse()
        .context("ID do usuário retornado pelo Discord é inválido")?;

    println!(
        "[PROBE] Usuário autenticado: {} (ID: {})",
        current_user.username, user_id
    );

    // 4. Instanciação da ponte de áudio (Hardware se disponível, ou pipeline de descarte)
    let device_manager = MediaDeviceManager::new();
    let has_hardware_audio = device_manager.list_input_devices().map(|d| !d.is_empty()).unwrap_or(false)
        && device_manager.list_output_devices().map(|d| !d.is_empty()).unwrap_or(false);

    let audio_bridge = if has_hardware_audio {
        println!("[PROBE] Dispositivos de hardware detectados: iniciando AudioEngine em thread dedicada.");
        AudioBridge::new_thread()
    } else {
        println!("[PROBE] Sem dispositivos de áudio de hardware: operando com gerador de silêncio e descarte.");
        let (_audio_out_tx, audio_out_rx) = tokio::sync::mpsc::channel(64);
        let (_audio_spk_tx, audio_spk_rx) = tokio::sync::mpsc::channel(16);
        let (audio_cmd_tx, mut audio_cmd_rx) = tokio::sync::mpsc::channel(64);

        tokio::spawn(async move {
            while let Some(cmd) = audio_cmd_rx.recv().await {
                match cmd {
                    AudioBridgeCommand::StartSession { responder } => {
                        let _ = responder.send(Ok(()));
                    }
                    AudioBridgeCommand::ReceiveOpusPacket { .. } => {}
                    AudioBridgeCommand::StopSession => {}
                    AudioBridgeCommand::SetMuted(_) => {}
                    AudioBridgeCommand::SetDeafened(_) => {}
                    AudioBridgeCommand::SwitchInputDevice { responder, device_id } => {
                        println!("[PROBE] -> Troca de dispositivo de entrada: {:?}", device_id);
                        let _ = responder.send(Ok(()));
                    }
                    AudioBridgeCommand::SwitchOutputDevice { responder, device_id } => {
                        println!("[PROBE] -> Troca de dispositivo de saída: {:?}", device_id);
                        let _ = responder.send(Ok(()));
                    }
                    AudioBridgeCommand::Shutdown => break,
                }
            }
        });

        AudioBridge::from_channels(audio_out_rx, audio_spk_rx, audio_cmd_tx)
    };

    // 5. Configuração e inicialização da VoiceSession com DAVE por padrão
    let session_config = VoiceSessionConfig {
        guild_id,
        channel_id,
        user_id,
        self_mute: false,
        self_deaf: false,
    };

    let session = Arc::new(VoiceSession::new(session_config, client.clone(), audio_bridge));
    let mut events_rx = session.subscribe_events();
    let mut state_rx = session.subscribe_state();

    println!("[PROBE] Iniciando VoiceSession em segundo plano...");
    session
        .start()
        .await
        .context("Falha ao disparar inicialização da VoiceSession")?;

    // 6. Loop de monitoramento de progresso passo a passo e estatísticas periódicas
    let mut stats_interval = tokio::time::interval(Duration::from_secs(1));
    stats_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut last_stats_instant = Instant::now();
    let mut last_sent_pkts = 0u64;
    let mut last_recv_pkts = 0u64;

    let probe_task = async {
        loop {
            tokio::select! {
                // Eventos de progresso do handshake e DAVE
                Ok(event) = events_rx.recv() => {
                    match event {
                        VoiceSessionEvent::VoiceStateSent => {
                            println!("[PROBE] -> Passo 1: Voice state enviado (Opcode 4) ao Gateway principal.");
                        }
                        VoiceSessionEvent::VoiceServerReceived { endpoint } => {
                            println!(
                                "[PROBE] -> Passo 2: Voice server recebido (Endpoint: {}, Token: [REDACTED]).",
                                endpoint
                            );
                        }
                        VoiceSessionEvent::ReadyReceived { ssrc, ip, port } => {
                            println!(
                                "[PROBE] -> Passo 3: Voice Gateway conectado e autenticado (Ready: SSRC={}, IP={}, Porta={}).",
                                ssrc, ip, port
                            );
                        }
                        VoiceSessionEvent::IpDiscovered { ip, port } => {
                            println!(
                                "[PROBE] -> Passo 4: IP discovery concluído sobre socket UDP (IP externo: {}, Porta: {}).",
                                ip, port
                            );
                        }
                        VoiceSessionEvent::SessionDescriptionReceived { mode, dave_protocol_version } => {
                            println!(
                                "[PROBE] -> Passo 5: Protocol select enviado e SessionDescription recebido (Modo: {}, DAVE Version: {:?}).",
                                mode, dave_protocol_version
                            );
                            if let Some(v) = dave_protocol_version {
                                println!("[PROBE] -> Estado DAVE: protocolo DAVE negociado na versão {} (E2EE ativo).", v);
                            } else {
                                println!("[PROBE] -> Estado DAVE: versão 0 (transporte legatário / passthrough).");
                            }
                        }
                        VoiceSessionEvent::DaveTransitionExecuted { transition_id } => {
                            println!(
                                "[PROBE] -> Estado DAVE: transição de época MLS executada com sucesso (Transition ID: {}).",
                                transition_id
                            );
                        }
                        VoiceSessionEvent::Speaking { ssrc, user_id, is_speaking } => {
                            let status = if is_speaking { "falando" } else { "silencioso" };
                            match user_id {
                                Some(uid) => println!("[PROBE] -> Speaking: usuário {} (SSRC {}) está {}.", uid, ssrc, status),
                                None => println!("[PROBE] -> Speaking: SSRC {} está {}.", ssrc, status),
                            }
                        }
                        VoiceSessionEvent::HeartbeatAck { ping_ms } => {
                            println!("[PROBE] -> Heartbeat ACK recebido: RTT = {}ms.", ping_ms);
                        }
                        VoiceSessionEvent::Disconnected { code, reason } => {
                            println!(
                                "[PROBE] -> Desconectado: Close Code: {:?}, Motivo: \"{}\"",
                                code, reason
                            );
                            break;
                        }
                    }
                }

                // Mudanças de estado de alto nível da sessão
                Ok(()) = state_rx.changed() => {
                    let state = state_rx.borrow().clone();
                    match state {
                        VoiceSessionState::Connecting => {
                            println!("[ESTADO] Sessão de voz conectando...");
                        }
                        VoiceSessionState::Connected => {
                            println!("[ESTADO] *** SESSÃO DE VOZ CONECTADA E ATIVA (Streaming RTP/Opus) ***");
                        }
                        VoiceSessionState::Reconnecting => {
                            println!("[ESTADO] Sessão de voz reconectando...");
                        }
                        VoiceSessionState::Disconnected { ref reason } => {
                            println!("[ESTADO] Sessão de voz finalizada: {reason}");
                            break;
                        }
                    }
                }

                // Estatísticas periódicas de transmissão e recepção RTP (pacotes/segundo)
                _ = stats_interval.tick() => {
                    if session.is_connected() {
                        let now = Instant::now();
                        let elapsed = now.duration_since(last_stats_instant).as_secs_f64();
                        if elapsed > 0.0 {
                            let stats = session.packet_stats();
                            let sent_diff = stats.sent_packets.saturating_sub(last_sent_pkts);
                            let recv_diff = stats.recv_packets.saturating_sub(last_recv_pkts);

                            let sent_rate = sent_diff as f64 / elapsed;
                            let recv_rate = recv_diff as f64 / elapsed;

                            println!(
                                "[RTP STATS] Tx: {:.1} pkts/s (Total: {}) | Rx: {:.1} pkts/s (Total: {})",
                                sent_rate, stats.sent_packets, recv_rate, stats.recv_packets
                            );

                            last_sent_pkts = stats.sent_packets;
                            last_recv_pkts = stats.recv_packets;
                            last_stats_instant = now;
                        }
                    }
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    };

    // 7. Encerramento gracioso ao receber Ctrl+C
    tokio::select! {
        res = probe_task => {
            if let Err(e) = res {
                eprintln!("[PROBE] Erro na execução da sessão: {e:#}");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n[PROBE] Sinal de interrupção (Ctrl+C) recebido. Encerrando sessão limpamente...");
        }
    }

    // Desconecta a sessão de voz e sai do canal no gateway principal
    println!("[PROBE] Desconectando e liberando canal de voz...");
    session.disconnect().await.ok();

    // Aguarda um instante para o envio do VoiceStateUpdate nulo
    sleep(Duration::from_millis(300)).await;
    println!("[PROBE] Probe encerrado com sucesso.");

    Ok(())
}
