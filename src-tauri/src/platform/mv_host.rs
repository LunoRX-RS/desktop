//! Serves the music-video host page on 127.0.0.1 (reached as localhost). The WebView (tauri://localhost) sends no HTTP Referer, and YouTube
//! refuses an embed without one (error 153). A page on this loopback origin sends one. Only `GET /mv/<video id>` is
//! answered; every other request gets 404.
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const MAX_HEAD: usize = 2048;
const TIMEOUT: Duration = Duration::from_secs(5);

/// A YouTube video id: 11 characters from the id alphabet. Anything else never reaches the page.
pub fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The host page for one video, or None when `id` is not a video id. The page runs no script of its own.
pub fn page(id: &str) -> Option<String> {
    is_video_id(id).then(|| {
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"referrer\" content=\"strict-origin-when-cross-origin\"><style>html,body{{margin:0;height:100%;background:#000;overflow:hidden}}iframe{{position:fixed;inset:0;width:100%;height:100%;border:0;pointer-events:none}}</style></head><body><iframe title=\"\" tabindex=\"-1\" src=\"https://www.youtube.com/embed/{id}?autoplay=1&amp;mute=1&amp;loop=1&amp;playlist={id}&amp;controls=0&amp;playsinline=1&amp;modestbranding=1&amp;rel=0\" allow=\"autoplay; encrypted-media\" referrerpolicy=\"strict-origin-when-cross-origin\"></iframe></body></html>"
        )
    })
}

/// The video id of a `GET /mv/<id>` request line (query string ignored), else None.
fn request_id(head: &str) -> Option<&str> {
    let mut parts = head.lines().next()?.split(' ');
    if parts.next()? != "GET" {
        return None;
    }
    parts.next()?.split('?').next()?.strip_prefix("/mv/")
}

/// DNS rebinding guard: the Host header must be this server's own loopback name and port. `localhost` is used
/// because YouTube refuses the embed when the Referer host is an IP literal (127.0.0.1).
fn host_ok(head: &str, port: u16) -> bool {
    let names = [format!("localhost:{port}"), format!("127.0.0.1:{port}")];
    head.lines().skip(1).any(|line| {
        line.split_once(':').is_some_and(|(k, v)| {
            k.trim().eq_ignore_ascii_case("host") && names.contains(&v.trim().to_owned())
        })
    })
}

fn response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: strict-origin-when-cross-origin\r\nContent-Security-Policy: default-src 'none'; frame-src https://www.youtube.com; style-src 'unsafe-inline'\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

async fn read_head(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut buf = Vec::with_capacity(512);
    let mut chunk = [0u8; 512];
    while buf.len() < MAX_HEAD && !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

async fn handle(mut stream: TcpStream, port: u16) {
    let Ok(Ok(head)) = tokio::time::timeout(TIMEOUT, read_head(&mut stream)).await else {
        return;
    };
    let reply = match request_id(&head)
        .filter(|_| host_ok(&head, port))
        .and_then(page)
    {
        Some(html) => response("200 OK", &html),
        None => response("404 Not Found", ""),
    };
    let _ = stream.write_all(reply.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// Binds a free loopback port and serves until the app exits. Returns the port.
pub async fn start() -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    tauri::async_runtime::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    tauri::async_runtime::spawn(handle(stream, port));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    });
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_video_ids_get_a_page() {
        assert!(page("dQw4w9WgXcQ").is_some());
        for bad in [
            "",
            "short",
            "dQw4w9WgXcQX",
            "dQw4w9Wg/cQ",
            "dQw4w9WgXc\"",
            "../../etc/x1",
        ] {
            assert!(page(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn page_frames_youtube_with_a_referrer_and_no_script() {
        let html = page("dQw4w9WgXcQ").unwrap();
        assert!(html.contains("https://www.youtube.com/embed/dQw4w9WgXcQ?"));
        assert!(html.contains("content=\"strict-origin-when-cross-origin\""));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn request_must_be_get_to_mv() {
        assert_eq!(
            request_id("GET /mv/dQw4w9WgXcQ HTTP/1.1\r\n"),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            request_id("GET /mv/dQw4w9WgXcQ?x=1 HTTP/1.1\r\n"),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(request_id("POST /mv/dQw4w9WgXcQ HTTP/1.1\r\n"), None);
        assert_eq!(request_id("GET /other HTTP/1.1\r\n"), None);
    }

    #[test]
    fn host_must_be_our_loopback_address_and_port() {
        assert!(host_ok(
            "GET / HTTP/1.1\r\nHost: localhost:4000\r\n\r\n",
            4000
        ));
        assert!(!host_ok(
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:4001\r\n\r\n",
            4000
        ));
        assert!(!host_ok(
            "GET / HTTP/1.1\r\nHost: evil.test:4000\r\n\r\n",
            4000
        ));
    }

    async fn fetch(raw: String) -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle(stream, port).await;
        });
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client
            .write_all(raw.replace("PORT", &port.to_string()).as_bytes())
            .await
            .unwrap();
        let mut out = String::new();
        client.read_to_string(&mut out).await.unwrap();
        server.await.unwrap();
        out
    }

    #[tokio::test]
    async fn serves_the_page_only_to_the_loopback_host() {
        let ok = fetch("GET /mv/dQw4w9WgXcQ HTTP/1.1\r\nHost: 127.0.0.1:PORT\r\n\r\n".into()).await;
        assert!(ok.starts_with("HTTP/1.1 200 OK"));
        assert!(ok.contains("youtube.com/embed/dQw4w9WgXcQ"));
        let rebound = fetch("GET /mv/dQw4w9WgXcQ HTTP/1.1\r\nHost: evil.test\r\n\r\n".into()).await;
        assert!(rebound.starts_with("HTTP/1.1 404"));
        let bad_id = fetch("GET /mv/..%2Fx HTTP/1.1\r\nHost: 127.0.0.1:PORT\r\n\r\n".into()).await;
        assert!(bad_id.starts_with("HTTP/1.1 404"));
    }
}
