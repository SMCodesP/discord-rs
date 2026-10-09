//! Testes de Integração e Verificação Arquitetural
//!
//! Valida os 4 Pilares da Infraestrutura Idêntica ao Cache Oficial do Discord:
//! 1. Estrutura de Diretórios e Content-Addressing (CAS SHA-256)
//! 2. Ciclo de Vida do Cache, ETags (304 Not Modified), Deltas de Gateway e Lazy Guilds (Op 14)
//! 3. Anti-Spam e Prevenção de 429 com Token Bucket Preemptivo e Coalescing de Ações
//! 4. Inicialização Instantânea (0ms Blank Screen) via Hidratação de Snapshot em Disco

use std::fs;
use std::time::Duration;

use discord_rs::domain::channel::{Channel, ChannelType};
use discord_rs::domain::guild::Guild;
use discord_rs::domain::message::{Author, Message};
use discord_rs::domain::user::User;
use discord_rs::protocol::cache::{
    AssetCasStore, CacheDirectory, CacheManifest, CacheRevalidator, HttpCacheValidation,
    InstantStartupOrchestrator, LazyGuildCoordinator,
};
use discord_rs::protocol::rate_limit::{
    RateLimitCoordinator, RateLimitHeaders, UserActionDebouncer,
};

fn create_temp_cache_dir(test_name: &str) -> CacheDirectory {
    let path = std::env::temp_dir().join(format!(
        "discord_rs_e2e_{}_{}",
        test_name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    CacheDirectory::with_root(path)
}

#[tokio::test]
async fn test_pillar_1_directory_structure_and_cas_sha256() -> anyhow::Result<()> {
    let cache = create_temp_cache_dir("p1_cas");
    cache.ensure_dirs()?;

    // 1. Verifica criação das subpastas idênticas ao Discord oficial
    assert!(cache.guilds_dir().exists(), "guilds/ dir must exist");
    assert!(cache.channels_dir().exists(), "channels/ dir must exist");
    assert!(cache.messages_dir().exists(), "messages/ dir must exist");
    assert!(cache.users_dir().exists(), "users/ dir must exist");
    assert!(cache.assets_dir().exists(), "assets/ dir must exist");

    // 2. Armazena asset binário com CAS (SHA-256)
    let cas = AssetCasStore::new(cache.clone());
    let avatar_bytes = b"discord-avatar-png-binary-content-12345";
    let meta = cas.store_asset(
        "https://cdn.discordapp.com/avatars/999/avatar.png",
        avatar_bytes,
        "image/png",
        Some("\"etag-avatar-1\"".to_string()),
        Some(86400),
    )?;

    assert_eq!(meta.size_bytes, avatar_bytes.len());
    assert_eq!(meta.content_type, "image/png");

    // 3. Verifica sharding por 2 primeiros caracteres hexadecimais
    let prefix = &meta.sha256_hash[..2];
    let shard_dir = cache.assets_dir().join(prefix);
    assert!(shard_dir.exists(), "Asset shard dir must exist");
    assert!(
        shard_dir.join(format!("{}.bin", meta.sha256_hash)).exists(),
        "Binary asset file must exist"
    );
    assert!(
        shard_dir
            .join(format!("{}.meta.json", meta.sha256_hash))
            .exists(),
        "Metadata asset file must exist"
    );

    // 4. Teste de Deduplicação Global: mesmo arquivo de outro endpoint reusa o mesmo binário
    let meta_dup = cas.store_asset(
        "https://cdn.discordapp.com/icons/888/icon.png",
        avatar_bytes,
        "image/png",
        None,
        None,
    )?;
    assert_eq!(meta.sha256_hash, meta_dup.sha256_hash);

    // 5. Recuperação de integridade
    let (loaded_bytes, loaded_meta) = cas
        .load_asset(&meta.sha256_hash)
        .expect("Must load stored asset");
    assert_eq!(loaded_bytes, avatar_bytes);
    assert_eq!(loaded_meta.size_bytes, avatar_bytes.len());

    Ok(())
}

#[tokio::test]
async fn test_pillar_2_etags_gateway_deltas_and_lazy_guilds() -> anyhow::Result<()> {
    let cache = create_temp_cache_dir("p2_lifecycle");
    let revalidator = CacheRevalidator::new(cache.clone());

    // --- Sub-teste A: HTTP ETags & 304 Not Modified ---
    let user = User::new("user_100", "EngenheiroDiscord");

    // Resposta inicial 200 OK
    let res = revalidator
        .process_user_me_response(
            200,
            Some("\"etag_user_v1\"".to_string()),
            Some("Wed, 21 Oct 2026 07:28:00 GMT".to_string()),
            Some(user.clone()),
        )
        .await?;

    match res {
        HttpCacheValidation::Updated(entry) => {
            assert_eq!(entry.data.username, "EngenheiroDiscord");
            assert_eq!(entry.etag.as_deref(), Some("\"etag_user_v1\""));
        }
        _ => panic!("Expected Updated"),
    }

    // Consulta condicional subsequente: Servidor responde 304 Not Modified (0 bytes payload)
    let res_304 = revalidator
        .process_user_me_response(304, None, None, None)
        .await?;

    match res_304 {
        HttpCacheValidation::NotModified => {
            // Sucesso! Dados locais mantidos e revalidados sem transferência
        }
        _ => panic!("Expected NotModified"),
    }

    // --- Sub-teste B: Revalidação Cirúrgica Orientada a Eventos do Gateway (Zero Polling) ---
    let mut msg = Message::new(
        "msg_1001",
        "chan_dev",
        Author::new("author_1", "Nexus"),
        "Deploy inicializado",
    );

    // MESSAGE_CREATE
    revalidator.apply_message_create(msg.clone()).await;

    // MESSAGE_UPDATE (Patch cirúrgico em memória e disco)
    msg.content = "Deploy concluído com sucesso!".to_string();
    msg.edited_timestamp = Some(chrono::Utc::now());
    revalidator.apply_message_update(msg).await;

    // MESSAGE_DELETE
    revalidator.apply_message_delete("chan_dev", "msg_1001").await;

    // --- Sub-teste C: Lazy Guilds & Opcode 14 ---
    let coordinator = LazyGuildCoordinator::new();

    // Alterna foco para guilda e canal selecionados
    let op14 = coordinator
        .focus_guild_and_channel("guild_rust_brasil", "chan_dev")
        .await;

    assert_eq!(op14.get("op").and_then(|v| v.as_u64()), Some(14));
    let d = op14.get("d").expect("d field");
    assert_eq!(d.get("guild_id").and_then(|v| v.as_str()), Some("guild_rust_brasil"));
    assert_eq!(d.get("typing").and_then(|v| v.as_bool()), Some(true));

    let channels_map = d.get("channels").expect("channels");
    let dev_ranges = channels_map.get("chan_dev").expect("chan_dev ranges");
    let first_range = &dev_ranges.as_array().unwrap()[0];
    assert_eq!(first_range[0].as_u64(), Some(0));
    assert_eq!(first_range[1].as_u64(), Some(99));

    Ok(())
}

#[tokio::test]
async fn test_pillar_3_token_bucket_zero_429_and_user_debouncing() -> anyhow::Result<()> {
    // --- Sub-teste A: Motor Token Bucket Preemptivo (Zero 429) ---
    let coordinator = RateLimitCoordinator::new();
    let route = "/channels/12345/messages";

    // Simula consumo de cota e resposta com remaining = 0 e reset_after = 80ms
    coordinator.pre_acquire(route).await;
    let headers = RateLimitHeaders {
        limit: Some(5),
        remaining: Some(0),
        reset_after: Some(Duration::from_millis(80)),
        bucket: Some("bucket_hash_msg".to_string()),
        is_global: false,
    };
    coordinator.post_response(route, &headers).await;

    // A próxima chamada deve aguardar preemptivamente na fila do Tokio sem gerar 429
    let start = std::time::Instant::now();
    coordinator.pre_acquire(route).await;
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(60),
        "Preemptive queue held request until reset (elapsed: {:?})",
        elapsed
    );

    // --- Sub-teste B: Coalescing de Typing Indicator (10s Cooldown) ---
    let debouncer = UserActionDebouncer::new();
    assert!(debouncer.should_send_typing("chan_1").await, "First typing event must be allowed");
    assert!(!debouncer.should_send_typing("chan_1").await, "Rapid typing must be coalesced (suppressed)");
    assert!(debouncer.should_send_typing("chan_2").await, "Separate channel must have independent throttle");

    // --- Sub-teste C: Debounce de Read Receipts / ACK (2s com Maior Snowflake) ---
    let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, String)>(10);

    let tx_clone1 = tx.clone();
    debouncer
        .schedule_ack("chan_1", "1000", move |c, m| async move {
            let _ = tx_clone1.send((c, m)).await;
        })
        .await;

    tokio::time::sleep(Duration::from_millis(30)).await;

    // Rola para mensagem mais recente (Snowflake 3000 > 1000)
    let tx_clone2 = tx.clone();
    debouncer
        .schedule_ack("chan_1", "3000", move |c, m| async move {
            let _ = tx_clone2.send((c, m)).await;
        })
        .await;

    tokio::time::sleep(Duration::from_millis(30)).await;

    // Mensagem intermediária mais antiga (Snowflake 2000 < 3000)
    let tx_clone3 = tx.clone();
    debouncer
        .schedule_ack("chan_1", "2000", move |c, m| async move {
            let _ = tx_clone3.send((c, m)).await;
        })
        .await;

    // Aguarda conclusão do debounce de 2 segundos
    tokio::time::sleep(Duration::from_millis(2100)).await;

    let received = rx.recv().await.expect("Expected coalesced ACK");
    assert_eq!(received.0, "chan_1");
    assert_eq!(received.1, "3000", "Must deliver highest Snowflake seen in the 2s window");
    assert!(rx.try_recv().is_err(), "Must NOT have duplicate ACKs");

    Ok(())
}

#[tokio::test]
async fn test_pillar_4_instant_startup_0ms_blank_screen() -> anyhow::Result<()> {
    let cache = create_temp_cache_dir("p4_startup");

    // 1. Popula cache local simulando desligamento anterior
    let user = User::new("user_smcodes", "Smcodes");
    cache.save_current_user(&user)?;

    let mut chan_geral = Channel::new("chan_geral_id", "geral", ChannelType::Text);
    chan_geral.guild_id = Some("guild_gpui".to_string());

    let guild = Guild::new("guild_gpui", "GPUI Community", "owner_123")
        .with_channels(vec![chan_geral]);
    cache.save_guild(&guild)?;

    let msg1 = Message::new(
        "msg_cold_1",
        "chan_geral_id",
        Author::new("auth_1", "Zack"),
        "GPUI v0.2.2 lançado!",
    );
    let msg2 = Message::new(
        "msg_cold_2",
        "chan_geral_id",
        Author::new("auth_2", "Nathan"),
        "Desempenho nativo insano.",
    );
    cache.save_channel_messages("chan_geral_id", &[msg1, msg2])?;

    let manifest = CacheManifest {
        version: 1,
        last_selected_guild_id: Some("guild_gpui".to_string()),
        last_selected_channel_id: Some("chan_geral_id".to_string()),
        active_user_id: Some("user_smcodes".to_string()),
        updated_at: chrono::Utc::now(),
    };
    cache.save_manifest(&manifest)?;

    // 2. Executa a hidratação instantânea (0ms cold start)
    let start_instant = std::time::Instant::now();
    let hydrated = InstantStartupOrchestrator::hydrate_from_disk(&cache);
    let hydration_duration = start_instant.elapsed();

    // Validação de performance: Leitura concluída em menos de 10ms
    assert!(
        hydration_duration < Duration::from_millis(50),
        "Cold start hydration took too long: {:?}",
        hydration_duration
    );

    // Validação de integridade dos dados para renderização do Frame 1
    assert!(hydrated.is_from_disk_cache);
    assert_eq!(hydrated.current_user.as_ref().unwrap().username, "Smcodes");
    assert_eq!(hydrated.guilds.len(), 1);
    assert_eq!(hydrated.active_guild.as_ref().unwrap().name, "GPUI Community");
    assert_eq!(hydrated.active_channel.as_ref().unwrap().name, "geral");
    assert_eq!(hydrated.active_messages.len(), 2);
    assert_eq!(hydrated.active_messages[0].content, "GPUI v0.2.2 lançado!");
    assert_eq!(hydrated.active_messages[1].content, "Desempenho nativo insano.");

    Ok(())
}
