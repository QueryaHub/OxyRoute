//! `application/x-www-form-urlencoded` and `multipart/form-data` parsing (issue #47).

use std::collections::HashMap;
use std::convert::Infallible;

use bytes::Bytes;
use form_urlencoded::parse as parse_urlencoded;
use futures_util::stream;
use multer::Multipart;

/// Per-part file entry from ``multipart/form-data`` (in-memory, whole body is already bounded).
pub struct ParsedFile {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: String,
    pub data: Vec<u8>,
}

pub struct ParsedFormData {
    pub form: HashMap<String, String>,
    pub files: Vec<ParsedFile>,
}

/// Same semantics as query parsing: last wins for duplicate keys.
pub fn parse_urlencoded_form(body: &[u8]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    if body.is_empty() {
        return m;
    }
    for (k, v) in parse_urlencoded(body) {
        m.insert(k.into_owned(), v.into_owned());
    }
    m
}

/// Parse a full multipart body; ``boundary`` is from ``Content-Type`` (see ``multer::parse_boundary``).
pub async fn parse_multipart(body: Vec<u8>, boundary: &str) -> Result<ParsedFormData, String> {
    let b = body;
    let stream = stream::once(async move { Result::<Bytes, Infallible>::Ok(Bytes::from(b)) });
    let mut multipart = Multipart::new(stream, boundary);
    let mut form: HashMap<String, String> = HashMap::new();
    let mut files: Vec<ParsedFile> = Vec::new();

    while let Some(mut field) = multipart.next_field().await.map_err(|e| e.to_string())? {
        let name = field.name().unwrap_or("field").to_string();
        if field.file_name().is_some() {
            let filename = field.file_name().map(str::to_string);
            let content_type = field
                .content_type()
                .map(|m| m.essence_str().to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let mut data = Vec::<u8>::new();
            while let Some(chunk) = field.chunk().await.map_err(|e| e.to_string())? {
                data.extend_from_slice(&chunk);
            }
            files.push(ParsedFile {
                name,
                filename,
                content_type,
                data,
            });
        } else {
            let text = field.text().await.map_err(|e| e.to_string())?;
            form.insert(name, text);
        }
    }
    Ok(ParsedFormData { form, files })
}

/// Default 8 MiB; ``0`` = no limit (not recommended in production). Override with env ``OXYROUTE_MAX_BODY_BYTES``.
pub fn max_body_bytes() -> u64 {
    const DEFAULT: u64 = 8 * 1024 * 1024;
    match std::env::var("OXYROUTE_MAX_BODY_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        None => DEFAULT,
        Some(0) => u64::MAX,
        Some(n) => n,
    }
}
