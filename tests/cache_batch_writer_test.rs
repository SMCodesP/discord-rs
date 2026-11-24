//! Testes do debouncer de escritas do cache local.
//!
//! Os testes usam o sistema de arquivos real em diretórios temporários
//! isolados e exercitam o `BatchWriter` junto com a leitura do `CacheDirectory`.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use discord_rs::domain::user::User;
use discord_rs::protocol::cache::{BatchWriter, CacheDirectory};

static NEXT_CACHE_DIR: AtomicU64 = AtomicU64::new(0);

fn create_temp_cache_dir(test_name: &str) -> CacheDirectory {
    let unique = NEXT_CACHE_DIR.fetch_add(1, Ordering::Relaxed);
    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let path = std::env::temp_dir().join(format!(
        "discord_rs_batch_writer_{}_{}_{}_{}",
        test_name,
        std::process::id(),
        timestamp,
        unique
    ));
    let _ = fs::remove_dir_all(&path);
    CacheDirectory::with_root(path)
}

async fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if predicate() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    predicate()
}

fn assert_no_temp_files(parent: &Path) {
    let entries = fs::read_dir(parent).expect("cache parent directory must exist");
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        assert!(
            !name.starts_with(".tmp_"),
            "temporary file was not renamed atomically: {name}"
        );
    }
}

#[tokio::test]
async fn debounces_and_coalesces_to_latest_value() -> anyhow::Result<()> {
    let cache = create_temp_cache_dir("debounce");
    let writer = BatchWriter::with_debounce(cache.clone(), Duration::from_millis(150));
    let path = cache.guild_path("guild_1");

    writer.enqueue_bytes(&path, b"v1".to_vec()).await?;
    tokio::time::sleep(Duration::from_millis(40)).await;
    writer.enqueue_bytes(&path, b"v2".to_vec()).await?;
    tokio::time::sleep(Duration::from_millis(40)).await;
    writer.enqueue_bytes(&path, b"v3".to_vec()).await?;

    assert_eq!(writer.pending_count().await, 1);
    assert!(!path.exists(), "debounce must delay the disk write");

    assert!(
        wait_until(Duration::from_secs(2), || path.exists()).await,
        "debounced write must land on disk"
    );
    assert_eq!(fs::read(&path)?, b"v3");
    assert_no_temp_files(path.parent().expect("guild path has parent"));

    writer.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn flush_writes_pending_without_waiting_for_debounce() -> anyhow::Result<()> {
    let cache = create_temp_cache_dir("flush");
    let writer = BatchWriter::with_debounce(cache.clone(), Duration::from_secs(10));

    let raw_path = cache.guild_path("guild_raw");
    let user_path = cache.user_path("user_1");
    let user = User::new("user_1", "synthetic_user");

    writer.enqueue_bytes(&raw_path, b"raw-bytes".to_vec()).await?;
    writer.enqueue_json(&user_path, &user).await?;

    assert!(!raw_path.exists(), "write must still be pending before flush");
    assert!(!user_path.exists(), "write must still be pending before flush");

    writer.flush().await?;

    assert_eq!(fs::read(&raw_path)?, b"raw-bytes");
    let loaded = cache.load_user("user_1").expect("user JSON must be readable");
    assert_eq!(loaded.id, "user_1");
    assert_eq!(loaded.username, "synthetic_user");
    assert_eq!(writer.pending_count().await, 0);

    assert_no_temp_files(raw_path.parent().expect("guild path has parent"));
    assert_no_temp_files(user_path.parent().expect("user path has parent"));

    writer.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn shutdown_flushes_pending_and_rejects_new_writes() -> anyhow::Result<()> {
    let cache = create_temp_cache_dir("shutdown");
    let writer = BatchWriter::with_debounce(cache.clone(), Duration::from_secs(10));
    let path = cache.channel_path("channel_shutdown");

    writer.enqueue_bytes(&path, b"pending-before-shutdown".to_vec()).await?;
    assert!(!path.exists(), "shutdown must be what forces this write");

    writer.shutdown().await?;

    assert_eq!(fs::read(&path)?, b"pending-before-shutdown");
    assert!(
        writer
            .enqueue_bytes(&path, b"after-shutdown".to_vec())
            .await
            .is_err(),
        "enqueue after shutdown must fail"
    );
    assert_eq!(fs::read(&path)?, b"pending-before-shutdown");
    Ok(())
}

#[tokio::test]
async fn max_delay_forces_flush_under_continuous_writes() -> anyhow::Result<()> {
    let cache = create_temp_cache_dir("max_delay");
    let writer = BatchWriter::with_timing(
        cache.clone(),
        Duration::from_millis(100),
        Duration::from_millis(250),
    );
    let path = cache.guild_path("guild_busy");

    let mut observed_flush = false;
    for index in 0..12 {
        writer
            .enqueue_bytes(&path, format!("v{index}").into_bytes())
            .await?;
        tokio::time::sleep(Duration::from_millis(40)).await;
        if path.exists() {
            observed_flush = true;
        }
    }

    assert!(
        observed_flush,
        "max delay must force a flush while writes keep arriving"
    );
    assert_eq!(writer.pending_count().await, 0);

    writer.flush().await?;
    assert_eq!(fs::read(&path)?, b"v11");
    assert_no_temp_files(path.parent().expect("guild path has parent"));

    writer.shutdown().await?;
    Ok(())
}
