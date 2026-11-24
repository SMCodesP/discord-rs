use discord_rs::protocol::{
    parse_gifs_response, GifItem, GifQuery, GifsClient, GifsClientConfig,
    DEFAULT_GIFS_LOCALE, DEFAULT_SEARCH_MEDIA_FORMAT, DEFAULT_TRENDING_MEDIA_FORMAT,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone)]
struct CapturedRequest {
    method: String,
    path: String,
    query: Vec<(String, String)>,
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> std::io::Result<CapturedRequest> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let header_end;
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed before headers",
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }

    let headers = String::from_utf8_lossy(&buf[..header_end]);
    let mut lines = headers.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/");
    let url = reqwest::Url::parse(&format!("http://localhost{}", target))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let path = url.path().to_string();
    let query = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<Vec<_>>();

    let content_length = lines
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);

    let mut body_read = buf.len().saturating_sub(header_end);
    while body_read < content_length {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body_read += n;
    }

    Ok(CapturedRequest {
        method,
        path,
        query,
    })
}

async fn spawn_json_server(
    status: &str,
    body: serde_json::Value,
    max_requests: usize,
) -> anyhow::Result<(String, Arc<Mutex<Vec<CapturedRequest>>>, tokio::task::JoinHandle<()>)> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);
    let body = body.to_string();
    let status = status.to_string();

    let handle = tokio::spawn(async move {
        for _ in 0..max_requests {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let Ok(req) = read_http_request(&mut stream).await else {
                break;
            };
            captured_clone.lock().unwrap().push(req);

            let response = format!(
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                body.as_bytes().len(),
                body
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        }
    });

    Ok((format!("http://{}", addr), captured, handle))
}

fn sample_gif(id: &str, title: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "title": title,
        "url": format!("https://media.example/{}.mp4", id),
        "src": format!("https://media.example/{}.webm", id),
        "width": 320,
        "height": 240
    })
}

fn query_value<'a>(req: &'a CapturedRequest, key: &str) -> Option<&'a str> {
    req.query
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

#[test]
fn parser_extracts_width_height_and_required_fields() {
    let payload = serde_json::json!([
        {
            "id": "gif-1",
            "title": "Wave",
            "url": "https://cdn.example/wave.mp4",
            "src": "https://cdn.example/wave.webm",
            "width": 498,
            "height": 280,
            "item_url": "https://tenor.com/view/wave"
        },
        {
            "id": "missing-dims",
            "title": "Bad",
            "url": "https://cdn.example/bad.mp4",
            "src": "https://cdn.example/bad.webm"
        }
    ]);

    let gifs = parse_gifs_response(&payload);
    assert_eq!(gifs.len(), 1);
    assert_eq!(
        gifs[0],
        GifItem {
            id: "gif-1".to_string(),
            title: "Wave".to_string(),
            url: "https://cdn.example/wave.mp4".to_string(),
            preview_url: "https://cdn.example/wave.webm".to_string(),
            width: 498,
            height: 280,
            tenor_url: Some("https://tenor.com/view/wave".to_string()),
        }
    );
}

#[tokio::test]
async fn trending_uses_endpoint_locale_media_format_and_cache() {
    let body = serde_json::json!([sample_gif("t1", "Trending One")]);
    let (base, captured, server) = spawn_json_server("200 OK", body, 2).await.unwrap();
    let client = GifsClient::with_config(
        reqwest::Client::new(),
        GifsClientConfig {
            base_url: base,
            cache_ttl: chrono::Duration::seconds(60),
            debounce: Duration::from_millis(10),
        },
    );

    let mut q = GifQuery::trending();
    q.locale = "en-US".to_string();
    q.media_format = "mp4".to_string();
    let first = client.trending(q.clone()).await.unwrap();
    let second = client.trending(q).await.unwrap();

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(client.cache_len().await, 1);

    server.abort();

    let reqs = captured.lock().unwrap();
    assert_eq!(reqs.len(), 1, "second trending call should hit cache");
    assert_eq!(reqs[0].method, "GET");
    assert_eq!(reqs[0].path, "/gifs/trending");
    assert_eq!(query_value(&reqs[0], "locale"), Some("en-US"));
    assert_eq!(query_value(&reqs[0], "media_format"), Some("mp4"));
    assert!(query_value(&reqs[0], "q").is_none());
}

#[tokio::test]
async fn search_uses_endpoint_query_locale_media_format_and_cache() {
    let body = serde_json::json!([sample_gif("s1", "Cat")]);
    let (base, captured, server) = spawn_json_server("200 OK", body, 2).await.unwrap();
    let client = GifsClient::with_config(
        reqwest::Client::new(),
        GifsClientConfig {
            base_url: base,
            cache_ttl: chrono::Duration::seconds(60),
            debounce: Duration::from_millis(10),
        },
    );

    let mut q = GifQuery::search("rust lang");
    q.locale = "pt-BR".to_string();
    q.media_format = "webm".to_string();
    q.limit = Some(5);

    let first = client.search(q.clone()).await.unwrap();
    let second = client.search(q).await.unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);

    server.abort();

    let reqs = captured.lock().unwrap();
    assert_eq!(reqs.len(), 1, "second search call should hit cache");
    assert_eq!(reqs[0].path, "/gifs/search");
    assert_eq!(query_value(&reqs[0], "q"), Some("rust lang"));
    assert_eq!(query_value(&reqs[0], "locale"), Some(DEFAULT_GIFS_LOCALE));
    assert_eq!(query_value(&reqs[0], "media_format"), Some("webm"));
    assert_eq!(query_value(&reqs[0], "limit"), Some("5"));
}

#[tokio::test]
async fn search_handles_empty_response() {
    let (base, captured, server) =
        spawn_json_server("200 OK", serde_json::json!([]), 1).await.unwrap();
    let client = GifsClient::with_config(
        reqwest::Client::new(),
        GifsClientConfig {
            base_url: base,
            cache_ttl: chrono::Duration::seconds(60),
            debounce: Duration::from_millis(10),
        },
    );

    let gifs = client.search(GifQuery::search("nothing")).await.unwrap();
    assert!(gifs.is_empty());
    server.abort();

    let reqs = captured.lock().unwrap();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].path, "/gifs/search");
}

#[tokio::test]
async fn debounce_cancels_previous_search() {
    let body = serde_json::json!([sample_gif("d1", "Debounced")]);
    let (base, captured, server) = spawn_json_server("200 OK", body, 2).await.unwrap();
    let client = Arc::new(GifsClient::with_config(
        reqwest::Client::new(),
        GifsClientConfig {
            base_url: base,
            cache_ttl: chrono::Duration::seconds(60),
            debounce: Duration::from_millis(80),
        },
    ));

    let hits = Arc::new(AtomicUsize::new(0));
    let notify = Arc::new(Notify::new());

    let hits1 = Arc::clone(&hits);
    let notify1 = Arc::clone(&notify);
    client
        .search_debounced(GifQuery::search("first"), move |res| async move {
            if res.is_ok() {
                hits1.fetch_add(1, Ordering::SeqCst);
            }
            notify1.notify_one();
        })
        .await
        .unwrap();

    let hits2 = Arc::clone(&hits);
    let notify2 = Arc::clone(&notify);
    client
        .search_debounced(GifQuery::search("second"), move |res| async move {
            assert!(res.is_ok());
            hits2.fetch_add(1, Ordering::SeqCst);
            notify2.notify_one();
        })
        .await
        .unwrap();

    timeout(Duration::from_secs(2), notify.notified())
        .await
        .expect("debounced search should resolve");
    tokio::time::sleep(Duration::from_millis(150)).await;
    server.abort();

    assert_eq!(hits.load(Ordering::SeqCst), 1, "only last debounced call should run");
    let reqs = captured.lock().unwrap();
    assert_eq!(reqs.len(), 1);
    assert_eq!(query_value(&reqs[0], "q"), Some("second"));
    assert_eq!(query_value(&reqs[0], "media_format"), Some(DEFAULT_SEARCH_MEDIA_FORMAT));
}
