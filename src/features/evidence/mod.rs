//! Optional authenticated access to archived evidence for a same-origin Metabase host.

use std::{io::Read, sync::Arc, time::Duration};

use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use diesel::{Connection, OptionalExtension, QueryableByName, RunQueryDsl, sql_query};
use diesel::sql_types::{BigInt, Binary, Integer, Nullable, Text};
use flate2::read::GzDecoder;
use serde::Deserialize;
use tokio::sync::Semaphore;

use crate::shared::{parser::Envelope, persistence::DbPool};

const AUTH_BODY_LIMIT: usize = 64 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
struct EvidenceState {
    pool: DbPool,
    auth: MetabaseAuth,
    compressed_limit: usize,
    uncompressed_limit: usize,
    slots: Arc<Semaphore>,
}

#[derive(Clone)]
struct MetabaseAuth {
    url: reqwest::Url,
    client: reqwest::Client,
}

impl MetabaseAuth {
    fn new(url: &str) -> Result<Self, &'static str> {
        let url = reqwest::Url::parse(url).map_err(|_| "Invalid METABASE_AUTH_URL")?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none()
            || !url.username().is_empty() || url.password().is_some()
            || url.query().is_some() || url.fragment().is_some()
            || url.path() != "/api/user/current"
        {
            return Err("METABASE_AUTH_URL must be a fixed HTTP(S) /api/user/current endpoint");
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(REQUEST_TIMEOUT)
            .build().map_err(|_| "Could not create evidence authentication client")?;
        Ok(Self { url, client })
    }

    async fn authorize(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        let session = session_token(headers)?;
        let mut response = self.client.get(self.url.clone())
            .header("X-Metabase-Session", session)
            .send().await.map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        match response.status().as_u16() {
            200 => {},
            401 => return Err(StatusCode::UNAUTHORIZED),
            403 => return Err(StatusCode::FORBIDDEN),
            _ => return Err(StatusCode::SERVICE_UNAVAILABLE),
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| StatusCode::SERVICE_UNAVAILABLE)? {
            if body.len().saturating_add(chunk.len()) > AUTH_BODY_LIMIT {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
            body.extend_from_slice(&chunk);
        }
        #[derive(Deserialize)]
        struct User { id: i64, is_superuser: bool, is_active: bool }
        let user: User = serde_json::from_slice(&body).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if user.id <= 0 || !user.is_superuser || !user.is_active {
            return Err(StatusCode::FORBIDDEN);
        }
        Ok(())
    }
}

fn session_token(headers: &HeaderMap) -> Result<String, StatusCode> {
    let mut tokens = Vec::new();
    for value in headers.get_all("X-Metabase-Session") {
        tokens.push(value.to_str().map_err(|_| StatusCode::UNAUTHORIZED)?.to_owned());
    }
    for value in headers.get_all(header::COOKIE) {
        for cookie in value.to_str().map_err(|_| StatusCode::UNAUTHORIZED)?.split(';') {
            if let Some((name, value)) = cookie.trim().split_once('=') {
                if name == "metabase.SESSION" {
                    tokens.push(value.to_owned());
                }
            }
        }
    }
    // Reject ambiguity instead of choosing a different identity from the reverse proxy.
    if tokens.is_empty() || tokens.iter().any(|value| value != &tokens[0]) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let token = tokens.remove(0);
    if token.is_empty() || token.len() > 256 || !token.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(token)
}

/// Disabled by default. Enabling this adapter grants evidence access only to Metabase administrators.
pub fn create_router(pool: DbPool, auth_url: Option<&str>, compressed_limit: usize, uncompressed_limit: usize) -> Result<Router, &'static str> {
    let Some(url) = auth_url else { return Ok(Router::new()); };
    let state = EvidenceState {
        pool, auth: MetabaseAuth::new(url)?, compressed_limit, uncompressed_limit,
        slots: Arc::new(Semaphore::new(4)),
    };
    Ok(Router::new().route("/api/evidence/attachments/{id}", get(attachment)).with_state(state))
}

#[derive(QueryableByName)]
struct ArchivedAttachment {
    #[diesel(sql_type = Integer)]
    item_index: i32,
    #[diesel(sql_type = Nullable<Text>)]
    filename: Option<String>,
    #[diesel(sql_type = BigInt)]
    size_bytes: i64,
    #[diesel(sql_type = Binary)]
    compressed_payload: Vec<u8>,
}

async fn attachment(State(state): State<EvidenceState>, Path(id): Path<i64>, headers: HeaderMap) -> Response {
    let result = read_attachment(state, id, headers).await;
    let mut response = match result { Ok(value) => value, Err(status) => status.into_response() };
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    headers.insert("X-Content-Type-Options", HeaderValue::from_static("nosniff"));
    headers.insert("Content-Security-Policy", HeaderValue::from_static("default-src 'none'; sandbox"));
    headers.insert("Referrer-Policy", HeaderValue::from_static("no-referrer"));
    response
}

async fn read_attachment(state: EvidenceState, id: i64, headers: HeaderMap) -> Result<Response, StatusCode> {
    let permit = state.slots.clone().try_acquire_owned().map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    state.auth.authorize(&headers).await?;
    if id <= 0 { return Err(StatusCode::NOT_FOUND); }
    let evidence = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut conn = state.pool.get_timeout(REQUEST_TIMEOUT).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let record = conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query("SET LOCAL statement_timeout = '2s'").execute(conn)?;
            sql_query("SELECT m.item_index,m.filename,m.size_bytes,a.compressed_payload FROM attachment_metadata m JOIN archive a ON a.hash=m.archive_hash AND a.project_id=m.project_id JOIN project p ON p.id=m.project_id WHERE m.id=$1 AND octet_length(a.compressed_payload)<=$2")
                .bind::<BigInt,_>(id).bind::<BigInt,_>(i64::try_from(state.compressed_limit).unwrap_or(i64::MAX))
                .get_result::<ArchivedAttachment>(conn).optional()
        }).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?.ok_or(StatusCode::NOT_FOUND)?;
        extract(record, state.uncompressed_limit)
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;
    Ok(evidence)
}

fn extract(record: ArchivedAttachment, limit: usize) -> Result<Response, StatusCode> {
    let mut decoded = Vec::new();
    GzDecoder::new(record.compressed_payload.as_slice()).take((limit as u64).saturating_add(1))
        .read_to_end(&mut decoded).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
    if decoded.len() > limit { return Err(StatusCode::PAYLOAD_TOO_LARGE); }
    let envelope = Envelope::parse(&decoded).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
    let index = usize::try_from(record.item_index).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
    let item = envelope.items.into_iter().nth(index).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
    if item.header.item_type != "attachment" || i64::try_from(item.payload.len()).ok() != Some(record.size_bytes) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let (mime, disposition) = content_type(&item.payload, item.header.content_type.as_deref());
    let filename = safe_filename(record.filename.as_deref().unwrap_or("attachment"));
    let mut response = item.payload.into_response();
    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    response.headers_mut().insert(header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("{disposition}; filename=\"{filename}\""))
            .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?);
    Ok(response)
}

fn content_type(bytes: &[u8], declared: Option<&str>) -> (&'static str, &'static str) {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        ("image/png", "inline")
    } else if bytes.starts_with(b"\xff\xd8\xff") && bytes.ends_with(b"\xff\xd9") {
        ("image/jpeg", "inline")
    } else if declared.is_some_and(|value| value.split(';').next().unwrap_or("").trim() == "text/plain")
        && std::str::from_utf8(bytes).is_ok() {
        ("text/plain; charset=utf-8", "inline")
    } else {
        ("application/octet-stream", "attachment")
    }
}

fn safe_filename(value: &str) -> String {
    let value: String = value.chars().take(120).map(|c| {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' }
    }).collect();
    if value.is_empty() || value.chars().all(|c| c == '.') { "attachment".to_owned() } else { value }
}

#[cfg(test)]
mod tests;
