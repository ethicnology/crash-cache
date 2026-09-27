use super::*;
use crate::shared::compression::GzipCompressor;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn cookie(value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, HeaderValue::from_str(value).unwrap());
    headers
}

#[test]
fn extracts_only_unambiguous_bounded_sessions() {
    assert_eq!(session_token(&cookie("unrelated=secret; metabase.SESSION=abc-123")).unwrap(), "abc-123");
    assert!(session_token(&cookie("unrelated=secret")).is_err());
    assert!(session_token(&cookie("metabase.SESSION=abc; metabase.SESSION=def")).is_err());
    assert!(session_token(&cookie("metabase.SESSION=has space")).is_err());
    assert!(session_token(&cookie(&format!("metabase.SESSION={}", "x".repeat(257)))).is_err());
    let mut headers = cookie("metabase.SESSION=abc");
    headers.insert("X-Metabase-Session", HeaderValue::from_static("def"));
    assert!(session_token(&headers).is_err());
}

#[test]
fn rejects_untrusted_auth_endpoint_forms() {
    for url in ["file:///api/user/current", "http://user:secret@localhost/api/user/current",
        "http://localhost/api/user/current?next=elsewhere", "http://localhost/api/user/current#fragment",
        "http://localhost/api/other"] {
        assert!(MetabaseAuth::new(url).is_err());
    }
    assert!(MetabaseAuth::new("http://metabase:3000/api/user/current").is_ok());
}

async fn auth_server(status: u16, body: String, extra_headers: &str) -> (MetabaseAuth, tokio::task::JoinHandle<String>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/api/user/current", listener.local_addr().unwrap());
    let response = format!("HTTP/1.1 {status} Test\r\nContent-Length: {}\r\n{extra_headers}Connection: close\r\n\r\n{body}", body.len());
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = vec![0; 4096];
        let count = stream.read(&mut request).await.unwrap();
        stream.write_all(response.as_bytes()).await.unwrap();
        String::from_utf8(request[..count].to_vec()).unwrap()
    });
    (MetabaseAuth::new(&url).unwrap(), handle)
}

#[tokio::test]
async fn authorizes_only_active_administrators_and_forwards_only_session() {
    for (body, expected) in [
        (r#"{"id":4,"is_superuser":true,"is_active":true}"#, Ok(())),
        (r#"{"id":4,"is_superuser":false,"is_active":true}"#, Err(StatusCode::FORBIDDEN)),
        (r#"{"id":4,"is_superuser":true,"is_active":false}"#, Err(StatusCode::FORBIDDEN)),
        (r#"{"id":4,"is_superuser":true}"#, Err(StatusCode::SERVICE_UNAVAILABLE)),
    ] {
        let (auth, server) = auth_server(200, body.to_owned(), "").await;
        assert_eq!(auth.authorize(&cookie("metabase.SESSION=fixture-session; unrelated=private")).await, expected);
        let request = server.await.unwrap().to_lowercase();
        assert!(request.contains("x-metabase-session: fixture-session"));
        assert!(!request.contains("private"));
        assert!(!request.contains("cookie:"));
    }
}

#[tokio::test]
async fn rejects_auth_redirect_errors_and_oversized_responses() {
    for (status, body, expected) in [
        (302, "".to_owned(), StatusCode::SERVICE_UNAVAILABLE),
        (401, "".to_owned(), StatusCode::UNAUTHORIZED),
        (403, "".to_owned(), StatusCode::FORBIDDEN),
        (500, "".to_owned(), StatusCode::SERVICE_UNAVAILABLE),
        (200, "x".repeat(AUTH_BODY_LIMIT + 1), StatusCode::SERVICE_UNAVAILABLE),
    ] {
        let (auth, server) = auth_server(status, body, "Location: http://127.0.0.1:1/\r\n").await;
        assert_eq!(auth.authorize(&cookie("metabase.SESSION=fixture")).await, Err(expected));
        server.await.unwrap();
    }
}

#[tokio::test]
async fn auth_timeout_is_bounded() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let auth = MetabaseAuth::new(&format!("http://{}/api/user/current", listener.local_addr().unwrap())).unwrap();
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        std::future::pending::<()>().await;
    });
    let start = std::time::Instant::now();
    assert_eq!(auth.authorize(&cookie("metabase.SESSION=fixture")).await, Err(StatusCode::SERVICE_UNAVAILABLE));
    assert!(start.elapsed() < Duration::from_secs(5));
    server.abort();
}

fn fixture(payload: &[u8]) -> ArchivedAttachment {
    let mut envelope = format!("{{}}\n{{\"type\":\"attachment\",\"content_type\":\"text/plain\",\"length\":{}}}\n", payload.len()).into_bytes();
    envelope.extend_from_slice(payload);
    ArchivedAttachment {
        item_index: 0, filename: Some("log.txt".to_owned()), size_bytes: payload.len() as i64,
        compressed_payload: GzipCompressor::new().compress(&envelope).unwrap(),
    }
}

#[test]
fn extraction_checks_size_index_kind_and_compression_limit() {
    assert_eq!(extract(fixture(b"log entry"), 1024).unwrap().headers()[header::CONTENT_TYPE], "text/plain; charset=utf-8");
    assert_eq!(extract(fixture(b"log entry"), 4).unwrap_err(), StatusCode::PAYLOAD_TOO_LARGE);
    let mut bad = fixture(b"log entry"); bad.item_index = 1;
    assert_eq!(extract(bad, 1024).unwrap_err(), StatusCode::UNPROCESSABLE_ENTITY);
    let mut bad = fixture(b"log entry"); bad.size_bytes = 999;
    assert_eq!(extract(bad, 1024).unwrap_err(), StatusCode::UNPROCESSABLE_ENTITY);
    let bad = ArchivedAttachment { item_index: 0, filename: None, size_bytes: 2,
        compressed_payload: GzipCompressor::new().compress(b"{}\n{\"type\":\"event\",\"length\":2}\n{}").unwrap() };
    assert_eq!(extract(bad, 1024).unwrap_err(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[test]
fn active_content_is_never_inline_html_or_svg() {
    assert_eq!(content_type(b"<script>alert(1)</script>", Some("text/html")), ("application/octet-stream", "attachment"));
    assert_eq!(content_type(b"<svg onload='alert(1)'>", Some("image/svg+xml")), ("application/octet-stream", "attachment"));
    assert_eq!(content_type(b"<script>alert(1)</script>", Some("image/png")), ("application/octet-stream", "attachment"));
    assert_eq!(content_type(b"\xff", Some("text/plain")), ("application/octet-stream", "attachment"));
    assert_eq!(content_type(b"\x89PNG\r\n\x1a\nfixture", Some("text/html")), ("image/png", "inline"));
    assert_eq!(content_type(b"\xff\xd8\xfffixture\xff\xd9", None), ("image/jpeg", "inline"));
    assert_eq!(safe_filename("../bad\"\r\nname.svg"), ".._bad___name.svg");
}
