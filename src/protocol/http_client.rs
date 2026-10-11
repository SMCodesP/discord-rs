use futures::future::BoxFuture;
use futures::io::AsyncReadExt;
use gpui::http_client::{http, AsyncBody, HttpClient, Url};

/// An HTTP client implementation for GPUI that uses `reqwest`.
/// Enables fetching remote image assets (such as Discord server icons, user avatars, etc.).
/// Carries a `tokio::runtime::Handle` to spawn network tasks on the Tokio runtime thread pool.
#[derive(Clone)]
pub struct ReqwestHttpClient {
    client: reqwest::Client,
    handle: tokio::runtime::Handle,
}

impl ReqwestHttpClient {
    pub fn new() -> Self {
        let handle = tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build fallback Tokio runtime for ReqwestHttpClient");
            let handle = rt.handle().clone();
            std::mem::forget(rt);
            handle
        });

        Self {
            client: reqwest::Client::builder()
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            handle,
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        let handle = tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build fallback Tokio runtime for ReqwestHttpClient");
            let handle = rt.handle().clone();
            std::mem::forget(rt);
            handle
        });

        Self { client, handle }
    }

    pub fn with_client_and_handle(client: reqwest::Client, handle: tokio::runtime::Handle) -> Self {
        Self { client, handle }
    }
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for ReqwestHttpClient {
    fn user_agent(&self) -> Option<&http::HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(
        &self,
        req: http::Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<http::Response<AsyncBody>>> {
        let client = self.client.clone();
        let handle = self.handle.clone();
        let (parts, mut body) = req.into_parts();
        let method = parts.method;
        let uri = parts.uri.to_string();
        let headers = parts.headers;

        Box::pin(async move {
            let resp = handle
                .spawn(async move {
                    let mut req_builder = client.request(method, &uri);
                    for (name, value) in headers.iter() {
                        req_builder = req_builder.header(name, value);
                    }

                    let mut body_bytes = Vec::new();
                    let _ = body.read_to_end(&mut body_bytes).await;
                    if !body_bytes.is_empty() {
                        req_builder = req_builder.body(body_bytes);
                    }

                    let response = req_builder.send().await?;
                    let status = response.status();
                    let version = response.version();
                    let resp_headers = response.headers().clone();
                    let bytes = response.bytes().await?;

                    let mut builder = http::Response::builder()
                        .status(status)
                        .version(version);

                    for (name, value) in resp_headers.iter() {
                        builder = builder.header(name, value);
                    }

                    let resp = builder.body(AsyncBody::from_bytes(bytes))?;
                    Ok::<_, anyhow::Error>(resp)
                })
                .await??;

            Ok(resp)
        })
    }
}
