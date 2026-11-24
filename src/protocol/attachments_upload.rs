//! Upload de anexos no formato oficial multipart/form-data do Discord.
//!
//! O contrato usado por `POST /channels/{channel.id}/messages` combina uma parte
//! `payload_json` com uma parte `files[n]` por arquivo. Os limites de tamanho
//! vêm do tier Nitro real da conta (`NitroTier::upload_limit_mb`).
//!
//! Spoiler de anexo segue a convenção do cliente oficial: o nome do arquivo
//! enviado é prefixado com `SPOILER_`.

use crate::domain::settings::NitroTier;
use crate::protocol::auth::AuthToken;
use anyhow::{Context, Result as AnyResult, bail};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// Quantidade máxima de anexos aceita em uma única mensagem pelo Discord.
pub const MAX_ATTACHMENTS_PER_MESSAGE: usize = 10;

/// Prefixo que transforma um anexo em spoiler no cliente oficial.
pub const SPOILER_PREFIX: &str = "SPOILER_";

/// Tipo usado quando a extensão não permite inferir um MIME conhecido.
pub const DEFAULT_ATTACHMENT_CONTENT_TYPE: &str = "application/octet-stream";

/// Tamanho de um mebibyte em bytes, base dos limites de upload do Discord.
pub const BYTES_PER_MIB: u64 = 1024 * 1024;

/// Limite máximo de bytes por arquivo para o tier informado.
pub fn max_attachment_bytes(tier: NitroTier) -> u64 {
    u64::from(tier.upload_limit_mb()) * BYTES_PER_MIB
}

/// Valida apenas o tamanho de um arquivo contra o limite do tier.
pub fn validate_attachment_size(
    tier: NitroTier,
    size: u64,
) -> Result<(), AttachmentUploadError> {
    let limit = max_attachment_bytes(tier);
    if size > limit {
        return Err(AttachmentUploadError::AttachmentTooLarge { size, limit, tier });
    }
    Ok(())
}

/// Infere o MIME type a partir da extensão do arquivo.
pub fn guess_content_type(filename: &str) -> &'static str {
    let lower = filename.to_ascii_lowercase();
    let extension = lower.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
    match extension {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "zip" => "application/zip",
        _ => DEFAULT_ATTACHMENT_CONTENT_TYPE,
    }
}

/// Retorna `true` quando o nome carrega o prefixo de spoiler oficial.
pub fn is_spoiler_filename(filename: &str) -> bool {
    filename.starts_with(SPOILER_PREFIX)
}

/// Remove um único prefixo `SPOILER_`, preservando o restante do nome.
pub fn strip_spoiler_prefix(filename: &str) -> &str {
    filename
        .strip_prefix(SPOILER_PREFIX)
        .unwrap_or(filename)
}

/// Erros de validação e montagem do upload de anexos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentUploadError {
    EmptyFilename,
    InvalidFilename(String),
    EmptyContentType,
    InvalidContentType,
    EmptyFile { filename: String },
    NoAttachments,
    TooManyAttachments { count: usize, limit: usize },
    AttachmentTooLarge { size: u64, limit: u64, tier: NitroTier },
    InvalidBoundary(String),
    PayloadSerialization(String),
}

impl fmt::Display for AttachmentUploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFilename => write!(f, "o nome do anexo não pode ser vazio"),
            Self::InvalidFilename(name) => {
                write!(f, "o nome do anexo é inválido: {name:?}")
            }
            Self::EmptyContentType => write!(f, "o content type do anexo não pode ser vazio"),
            Self::InvalidContentType => write!(f, "o content type do anexo contém caracteres inválidos"),
            Self::EmptyFile { filename } => {
                write!(f, "o anexo {filename:?} está vazio")
            }
            Self::NoAttachments => write!(f, "nenhum anexo foi informado para o upload"),
            Self::TooManyAttachments { count, limit } => {
                write!(f, "{count} anexos excedem o limite de {limit} por mensagem")
            }
            Self::AttachmentTooLarge { size, limit, tier } => write!(
                f,
                "o anexo tem {size} bytes e excede o limite de {limit} bytes do tier {:?}",
                tier
            ),
            Self::InvalidBoundary(boundary) => {
                write!(f, "boundary multipart inválido: {boundary:?}")
            }
            Self::PayloadSerialization(error) => {
                write!(f, "falha ao serializar payload_json: {error}")
            }
        }
    }
}

impl std::error::Error for AttachmentUploadError {}

/// Arquivo local pronto para ser enviado como uma parte multipart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadAttachment {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
    pub description: Option<String>,
}

impl UploadAttachment {
    /// Cria o anexo validando nome, content type e conteúdo.
    pub fn new(
        filename: impl Into<String>,
        content_type: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, AttachmentUploadError> {
        let filename = filename.into();
        let content_type = content_type.into();
        validate_filename(&filename)?;
        validate_content_type(&content_type)?;
        Ok(Self {
            filename,
            content_type,
            bytes: bytes.into(),
            description: None,
        })
    }

    /// Cria o anexo inferindo o content type pela extensão do nome.
    pub fn from_bytes(
        filename: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, AttachmentUploadError> {
        let filename = filename.into();
        let content_type = guess_content_type(&filename).to_string();
        Self::new(filename, content_type, bytes)
    }

    /// Associa uma descrição alternativa (ALT text) ao anexo.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Tamanho real do arquivo em bytes.
    pub fn size(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// `true` quando o nome já está prefixado com `SPOILER_`.
    pub fn is_spoiler(&self) -> bool {
        is_spoiler_filename(&self.filename)
    }

    /// Nome exibido sem o prefixo de spoiler.
    pub fn display_filename(&self) -> &str {
        strip_spoiler_prefix(&self.filename)
    }

    /// Liga ou desliga o spoiler sem duplicar o prefixo existente.
    pub fn with_spoiler(mut self, spoiler: bool) -> Result<Self, AttachmentUploadError> {
        let base = self.display_filename().to_string();
        if base.is_empty() {
            return Err(AttachmentUploadError::EmptyFilename);
        }
        self.filename = if spoiler {
            format!("{SPOILER_PREFIX}{base}")
        } else {
            base
        };
        validate_filename(&self.filename)?;
        Ok(self)
    }
}

/// Corpo multipart pronto para ser enviado em uma requisição HTTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartMessage {
    pub boundary: String,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl MultipartMessage {
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn into_body(self) -> Vec<u8> {
        self.body
    }
}

/// Constrói o multipart oficial (`payload_json` + `files[n]`).
#[derive(Debug, Clone, Default)]
pub struct MultipartMessageBuilder {
    content: String,
    attachments: Vec<UploadAttachment>,
    boundary: Option<String>,
}

impl MultipartMessageBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn attachment(mut self, attachment: UploadAttachment) -> Self {
        self.attachments.push(attachment);
        self
    }

    pub fn attachments(
        mut self,
        attachments: impl IntoIterator<Item = UploadAttachment>,
    ) -> Self {
        self.attachments.extend(attachments);
        self
    }

    /// Define um boundary explícito. Precisa ser um token RFC 2046 válido.
    pub fn with_boundary(mut self, boundary: impl Into<String>) -> Self {
        self.boundary = Some(boundary.into());
        self
    }

    /// Valida o lote para o tier e produz o corpo multipart.
    pub fn build(
        self,
        tier: NitroTier,
    ) -> Result<MultipartMessage, AttachmentUploadError> {
        validate_attachments(&self.attachments, tier)?;

        let metadata = self
            .attachments
            .iter()
            .enumerate()
            .map(|(index, attachment)| AttachmentMetadata {
                id: index,
                filename: attachment.filename.as_str(),
                description: attachment.description.as_deref(),
            })
            .collect::<Vec<_>>();
        let payload = PayloadJson {
            content: self.content.as_str(),
            attachments: metadata,
        };
        let payload_bytes = serde_json::to_vec(&payload)
            .map_err(|error| AttachmentUploadError::PayloadSerialization(error.to_string()))?;

        let boundary = match self.boundary {
            Some(boundary) => {
                validate_boundary(&boundary)?;
                if boundary_collides(&boundary, &payload_bytes, &self.attachments) {
                    return Err(AttachmentUploadError::InvalidBoundary(boundary));
                }
                boundary
            }
            None => generate_boundary(&payload_bytes, &self.attachments),
        };

        let mut body = Vec::new();
        push_boundary(&mut body, &boundary);
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"payload_json\"\r\n");
        body.extend_from_slice(b"Content-Type: application/json\r\n\r\n");
        body.extend_from_slice(&payload_bytes);
        body.extend_from_slice(b"\r\n");

        for (index, attachment) in self.attachments.iter().enumerate() {
            push_boundary(&mut body, &boundary);
            let disposition = format!(
                "Content-Disposition: form-data; name=\"files[{index}]\"; filename=\"{}\"\r\n",
                escape_disposition_value(&attachment.filename)
            );
            body.extend_from_slice(disposition.as_bytes());
            let content_type = format!("Content-Type: {}\r\n\r\n", attachment.content_type);
            body.extend_from_slice(content_type.as_bytes());
            body.extend_from_slice(&attachment.bytes);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(b"--");
        body.extend_from_slice(boundary.as_bytes());
        body.extend_from_slice(b"--\r\n");

        Ok(MultipartMessage {
            content_type: format!("multipart/form-data; boundary={boundary}"),
            boundary,
            body,
        })
    }
}

/// Valida um único anexo para o tier informado.
pub fn validate_attachment(
    attachment: &UploadAttachment,
    tier: NitroTier,
) -> Result<(), AttachmentUploadError> {
    validate_filename(&attachment.filename)?;
    validate_content_type(&attachment.content_type)?;
    if attachment.bytes.is_empty() {
        return Err(AttachmentUploadError::EmptyFile {
            filename: attachment.filename.clone(),
        });
    }
    validate_attachment_size(tier, attachment.size())
}

/// Valida o lote completo: quantidade, nome, conteúdo, tipo e tamanho.
pub fn validate_attachments(
    attachments: &[UploadAttachment],
    tier: NitroTier,
) -> Result<(), AttachmentUploadError> {
    if attachments.is_empty() {
        return Err(AttachmentUploadError::NoAttachments);
    }
    if attachments.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(AttachmentUploadError::TooManyAttachments {
            count: attachments.len(),
            limit: MAX_ATTACHMENTS_PER_MESSAGE,
        });
    }
    for attachment in attachments {
        validate_attachment(attachment, tier)?;
    }
    Ok(())
}

/// Resposta relevante de uma mensagem enviada com anexos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadedMessage {
    pub id: String,
    #[serde(default)]
    pub channel_id: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<UploadedAttachment>,
}

/// Anexo confirmado pela resposta REST do Discord.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadedAttachment {
    pub id: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub content_type: Option<String>,
}

/// Cliente HTTP que envia a mensagem multipart ao endpoint oficial.
#[derive(Clone)]
pub struct AttachmentUploader {
    http: reqwest::Client,
    base_url: String,
    authorization: String,
}

impl AttachmentUploader {
    pub fn new(
        http: reqwest::Client,
        base_url: impl Into<String>,
        authorization: impl Into<String>,
    ) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            authorization: authorization.into(),
        }
    }

    pub fn with_token(
        http: reqwest::Client,
        base_url: impl Into<String>,
        token: &AuthToken,
    ) -> Self {
        Self::new(http, base_url, token.authorization_header())
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn build_message(
        &self,
        content: &str,
        tier: NitroTier,
        attachments: Vec<UploadAttachment>,
    ) -> Result<MultipartMessage, AttachmentUploadError> {
        MultipartMessageBuilder::new()
            .content(content)
            .attachments(attachments)
            .build(tier)
    }

    /// Envia a mensagem com anexos; só devolve sucesso após a resposta da API.
    pub async fn send_message(
        &self,
        channel_id: &str,
        content: &str,
        tier: NitroTier,
        attachments: Vec<UploadAttachment>,
    ) -> AnyResult<UploadedMessage> {
        let channel_id = channel_id.trim();
        if channel_id.is_empty() {
            bail!("ID do canal vazio ao enviar anexos");
        }
        let message = self.build_message(content, tier, attachments)?;
        let url = format!("{}/channels/{}/messages", self.base_url, channel_id);

        let response = self
            .http
            .post(url)
            .header(reqwest::header::AUTHORIZATION, &self.authorization)
            .header(reqwest::header::CONTENT_TYPE, &message.content_type)
            .body(message.body)
            .send()
            .await
            .context("falha ao enviar anexos para o Discord")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("falha ao ler a resposta do upload de anexos")?;
        if !status.is_success() {
            bail!(
                "upload de anexos falhou com HTTP {}: {}",
                status.as_u16(),
                body
            );
        }

        serde_json::from_str(&body).context("resposta do upload de anexos inválida")
    }
}

#[derive(Debug, Serialize)]
struct PayloadJson<'a> {
    #[serde(skip_serializing_if = "str::is_empty")]
    content: &'a str,
    attachments: Vec<AttachmentMetadata<'a>>,
}

#[derive(Debug, Serialize)]
struct AttachmentMetadata<'a> {
    id: usize,
    filename: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
}

fn validate_filename(filename: &str) -> Result<(), AttachmentUploadError> {
    if filename.trim().is_empty() {
        return Err(AttachmentUploadError::EmptyFilename);
    }
    if filename == SPOILER_PREFIX {
        return Err(AttachmentUploadError::InvalidFilename(filename.to_string()));
    }
    if filename.chars().any(char::is_control)
        || filename.contains('/')
        || filename.contains('\\')
    {
        return Err(AttachmentUploadError::InvalidFilename(filename.to_string()));
    }
    Ok(())
}

fn validate_content_type(content_type: &str) -> Result<(), AttachmentUploadError> {
    if content_type.trim().is_empty() {
        return Err(AttachmentUploadError::EmptyContentType);
    }
    if content_type.chars().any(char::is_control) {
        return Err(AttachmentUploadError::InvalidContentType);
    }
    Ok(())
}

fn validate_boundary(boundary: &str) -> Result<(), AttachmentUploadError> {
    let valid = !boundary.is_empty()
        && boundary.len() <= 70
        && boundary.bytes().all(|byte| {
            matches!(
                byte,
                b'0'..=b'9'
                    | b'a'..=b'z'
                    | b'A'..=b'Z'
                    | b'\''
                    | b'('
                    | b')'
                    | b'+'
                    | b'_'
                    | b','
                    | b'-'
                    | b'.'
                    | b'/'
                    | b':'
                    | b'='
                    | b'?'
            )
        });
    if valid {
        Ok(())
    } else {
        Err(AttachmentUploadError::InvalidBoundary(boundary.to_string()))
    }
}

fn generate_boundary(payload: &[u8], attachments: &[UploadAttachment]) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    static PROCESS_SALT: OnceLock<u64> = OnceLock::new();
    let salt = *PROCESS_SALT.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0);
        nanos ^ (u64::from(std::process::id()) << 32)
    });

    loop {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let boundary = format!("----DiscordRsUpload{salt:016x}{sequence:016x}");
        if !boundary_collides(&boundary, payload, attachments) {
            return boundary;
        }
    }
}

fn boundary_collides(
    boundary: &str,
    payload: &[u8],
    attachments: &[UploadAttachment],
) -> bool {
    let needle = boundary.as_bytes();
    contains_bytes(payload, needle)
        || attachments
            .iter()
            .any(|attachment| contains_bytes(&attachment.bytes, needle))
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn push_boundary(body: &mut Vec<u8>, boundary: &str) {
    body.extend_from_slice(b"--");
    body.extend_from_slice(boundary.as_bytes());
    body.extend_from_slice(b"\r\n");
}

fn escape_disposition_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            _ => escaped.push(character),
        }
    }
    escaped
}
