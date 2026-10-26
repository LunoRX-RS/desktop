//! The MCP Streamable HTTP transport, reduced to what a local tool server needs: one `POST /mcp` endpoint that
//! answers with JSON (no server-sent events, no sessions). Listens on 127.0.0.1 only. Every request must carry the
//! bearer token, and the Host and Origin headers must name this loopback address, so a web page in the user's
//! browser cannot reach the player through DNS rebinding.
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub const PATH: &str = "/mcp";
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 256 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// HTTP status for a request that is refused before it reaches the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    TooLarge,
    UnsupportedMediaType,
}

impl Refusal {
    fn status(self) -> &'static str {
        match self {
            Self::BadRequest => "400 Bad Request",
            Self::Unauthorized => "401 Unauthorized",
            Self::Forbidden => "403 Forbidden",
            Self::NotFound => "404 Not Found",
            Self::MethodNotAllowed => "405 Method Not Allowed",
            Self::TooLarge => "413 Content Too Large",
            Self::UnsupportedMediaType => "415 Unsupported Media Type",
        }
    }
}

/// Parses the head and body out of `raw`. `Ok(None)` while more bytes are needed.
pub fn parse(raw: &[u8]) -> Result<Option<Request>, Refusal> {
    let Some(head_end) = raw.windows(4).position(|w| w == b"\r\n\r\n") else {
        return if raw.len() > MAX_HEADER_BYTES {
            Err(Refusal::TooLarge)
        } else {
            Ok(None)
        };
    };
    if head_end > MAX_HEADER_BYTES {
        return Err(Refusal::TooLarge);
    }
    let head = std::str::from_utf8(&raw[..head_end]).map_err(|_| Refusal::BadRequest)?;
    let mut lines = head.split("\r\n");
    let mut start = lines.next().unwrap_or("").split(' ');
    let (Some(method), Some(target), Some(version)) = (start.next(), start.next(), start.next())
    else {
        return Err(Refusal::BadRequest);
    };
    if !version.starts_with("HTTP/1.") {
        return Err(Refusal::BadRequest);
    }
    let mut headers = Vec::new();
    for line in lines {
        let (k, v) = line.split_once(':').ok_or(Refusal::BadRequest)?;
        headers.push((k.trim().to_owned(), v.trim().to_owned()));
    }
    let mut req = Request {
        method: method.to_owned(),
        path: target.split('?').next().unwrap_or("").to_owned(),
        headers,
        body: Vec::new(),
    };
    if req.header("transfer-encoding").is_some() {
        // Chunked bodies are not needed by MCP clients that send one JSON message per POST.
        return Err(Refusal::BadRequest);
    }
    let len = match req.header("content-length") {
        None => 0,
        Some(v) => v.parse::<usize>().map_err(|_| Refusal::BadRequest)?,
    };
    if len > MAX_BODY_BYTES {
        return Err(Refusal::TooLarge);
    }
    let body_start = head_end + 4;
    if raw.len() < body_start + len {
        return Ok(None);
    }
    req.body = raw[body_start..body_start + len].to_vec();
    Ok(Some(req))
}

fn loopback_authority(value: &str, port: u16) -> bool {
    value == format!("127.0.0.1:{port}") || value == format!("localhost:{port}")
}

/// Equal-time comparison so a wrong token takes as long to refuse as a nearly right one.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Everything a request must satisfy before its body is read as JSON-RPC.
pub fn admit(req: &Request, token: &str, port: u16) -> Result<(), Refusal> {
    if req.path != PATH {
        return Err(Refusal::NotFound);
    }
    let host_ok = req
        .header("host")
        .is_some_and(|h| loopback_authority(h, port));
    let origin_ok = req.header("origin").is_none_or(|o| {
        o.strip_prefix("http://")
            .is_some_and(|rest| loopback_authority(rest, port))
    });
    if !host_ok || !origin_ok {
        return Err(Refusal::Forbidden);
    }
    let presented = req
        .header("authorization")
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if token.is_empty() || !same(presented.as_bytes(), token.as_bytes()) {
        return Err(Refusal::Unauthorized);
    }
    if req.method != "POST" {
        // GET would open a server-sent event stream; this server has nothing to push.
        return Err(Refusal::MethodNotAllowed);
    }
    let json = req.header("content-type").is_some_and(|c| {
        c.split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("application/json")
    });
    if !json {
        return Err(Refusal::UnsupportedMediaType);
    }
    Ok(())
}

/// Reads one request from the socket within the time and size limits.
pub async fn read(stream: &mut TcpStream) -> Result<Request, Refusal> {
    let mut raw = Vec::with_capacity(4096);
    let mut buf = [0u8; 8192];
    let read_all = async {
        loop {
            if let Some(req) = parse(&raw)? {
                return Ok(req);
            }
            let n = stream
                .read(&mut buf)
                .await
                .map_err(|_| Refusal::BadRequest)?;
            if n == 0 {
                return Err(Refusal::BadRequest);
            }
            raw.extend_from_slice(&buf[..n]);
            if raw.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES + 4 {
                return Err(Refusal::TooLarge);
            }
        }
    };
    tokio::time::timeout(READ_TIMEOUT, read_all)
        .await
        .unwrap_or(Err(Refusal::BadRequest))
}

pub async fn respond_json(stream: &mut TcpStream, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(body).await;
    let _ = stream.shutdown().await;
}

/// 202 Accepted: a notification was received; MCP sends no body back.
pub async fn respond_accepted(stream: &mut TcpStream) {
    let _ = stream
        .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .await;
    let _ = stream.shutdown().await;
}

pub async fn refuse(stream: &mut TcpStream, why: Refusal) {
    let mut head = format!(
        "HTTP/1.1 {}\r\nContent-Length: 0\r\nConnection: close\r\n",
        why.status()
    );
    if why == Refusal::Unauthorized {
        head.push_str("WWW-Authenticate: Bearer\r\n");
    }
    if why == Refusal::MethodNotAllowed {
        head.push_str("Allow: POST\r\n");
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";
    const PORT: u16 = 47615;

    fn request(extra: &str, body: &str) -> Vec<u8> {
        format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{PORT}\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn admitted(raw: &[u8]) -> Result<(), Refusal> {
        admit(&parse(raw).unwrap().unwrap(), TOKEN, PORT)
    }

    #[test]
    fn a_valid_request_is_admitted() {
        let raw = request("", r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#);
        let req = parse(&raw).unwrap().unwrap();
        assert_eq!(req.body, br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#);
        assert_eq!(admit(&req, TOKEN, PORT), Ok(()));
        let local_origin = request(&format!("Origin: http://localhost:{PORT}\r\n"), "{}");
        assert_eq!(admitted(&local_origin), Ok(()));
    }

    #[test]
    fn waits_for_the_whole_body() {
        let raw = request("", r#"{"jsonrpc":"2.0"}"#);
        assert_eq!(parse(&raw[..raw.len() - 3]), Ok(None));
        assert_eq!(parse(b"POST /mcp HTTP/1.1\r\nHost: x"), Ok(None));
    }

    #[test]
    fn wrong_or_missing_token_is_unauthorized() {
        let raw = String::from_utf8(request("", "{}")).unwrap();
        let wrong = raw.replace(TOKEN, &"f".repeat(TOKEN.len()));
        assert_eq!(admitted(wrong.as_bytes()), Err(Refusal::Unauthorized));
        let missing = raw.replace(&format!("Authorization: Bearer {TOKEN}\r\n"), "");
        assert_eq!(admitted(missing.as_bytes()), Err(Refusal::Unauthorized));
        let req = parse(raw.as_bytes()).unwrap().unwrap();
        assert_eq!(admit(&req, "", PORT), Err(Refusal::Unauthorized));
    }

    #[test]
    fn foreign_host_or_origin_is_forbidden() {
        let raw = String::from_utf8(request("", "{}")).unwrap();
        let rebound = raw.replace(
            &format!("127.0.0.1:{PORT}"),
            &format!("evil.example:{PORT}"),
        );
        assert_eq!(admitted(rebound.as_bytes()), Err(Refusal::Forbidden));
        let other_port = raw.replace(&format!("127.0.0.1:{PORT}"), "127.0.0.1:80");
        assert_eq!(admitted(other_port.as_bytes()), Err(Refusal::Forbidden));
        let web_page = request("Origin: https://evil.example\r\n", "{}");
        assert_eq!(admitted(&web_page), Err(Refusal::Forbidden));
    }

    #[test]
    fn method_path_and_type() {
        let raw = String::from_utf8(request("", "{}")).unwrap();
        assert_eq!(
            admitted(raw.replacen("POST", "GET", 1).as_bytes()),
            Err(Refusal::MethodNotAllowed)
        );
        assert_eq!(
            admitted(raw.replacen("/mcp", "/admin", 1).as_bytes()),
            Err(Refusal::NotFound)
        );
        assert_eq!(
            admitted(raw.replace("application/json", "text/plain").as_bytes()),
            Err(Refusal::UnsupportedMediaType)
        );
        assert_eq!(
            admitted(
                raw.replace("application/json", "application/json; charset=utf-8")
                    .as_bytes()
            ),
            Ok(())
        );
    }

    #[test]
    fn size_limits() {
        let huge = format!(
            "POST /mcp HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        assert_eq!(parse(huge.as_bytes()), Err(Refusal::TooLarge));
        let endless_head = vec![b'a'; MAX_HEADER_BYTES + 1];
        assert_eq!(parse(&endless_head), Err(Refusal::TooLarge));
        let chunked = "POST /mcp HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n";
        assert_eq!(parse(chunked.as_bytes()), Err(Refusal::BadRequest));
    }

    #[test]
    fn token_comparison() {
        assert!(same(b"abc", b"abc"));
        assert!(!same(b"abc", b"abd"));
        assert!(!same(b"abc", b"abcd"));
    }
}
