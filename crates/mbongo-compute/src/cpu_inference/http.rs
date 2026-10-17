//! A minimal, bounded HTTP/1.1 client for the local llama-server
//! (MVP-B §62). Loopback only, one request per connection, a total
//! deadline, and a hard cap on the bytes read. Nothing here logs a body.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// A response: status and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// HTTP status code.
    pub status: u16,
    /// Body bytes (chunked encoding already decoded).
    pub body: Vec<u8>,
}

/// Why a request failed. Never carries body content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HttpError {
    /// Nothing is listening, or the connection was refused.
    #[error("cannot connect: {0}")]
    Connect(String),
    /// The total deadline passed.
    #[error("timed out")]
    Timeout,
    /// The response exceeded the byte cap.
    #[error("response exceeds {0} bytes")]
    TooLarge(usize),
    /// The bytes are not an HTTP/1.x response this client understands.
    #[error("malformed response: {0}")]
    Malformed(&'static str),
    /// Any other I/O failure (the server died mid-response, …).
    #[error("i/o: {0}")]
    Io(String),
}

/// Header allowance on top of the body cap.
const HEADER_ALLOWANCE: usize = 16 * 1024;

/// Sends one request to `127.0.0.1:port` and reads the whole response.
pub fn request(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    timeout: Duration,
    max_body: usize,
) -> Result<Response, HttpError> {
    let deadline = Instant::now() + timeout;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let connect_budget = timeout.min(Duration::from_secs(5));
    let mut stream = TcpStream::connect_timeout(&addr, connect_budget)
        .map_err(|e| HttpError::Connect(e.kind().to_string()))?;
    let _ = stream.set_nodelay(true);
    let payload = body.unwrap_or_default();
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        payload.len()
    );
    stream
        .set_write_timeout(Some(remaining(deadline)?))
        .map_err(|e| HttpError::Io(e.kind().to_string()))?;
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(payload))
        .map_err(|e| io_error(&e))?;

    let cap = max_body + HEADER_ALLOWANCE;
    let mut raw = Vec::with_capacity(8192);
    let mut buf = [0u8; 16 * 1024];
    loop {
        stream
            .set_read_timeout(Some(remaining(deadline)?))
            .map_err(|e| HttpError::Io(e.kind().to_string()))?;
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&buf[..n]);
                if raw.len() > cap {
                    return Err(HttpError::TooLarge(max_body));
                }
                if let Some(done) = complete_length(&raw) {
                    if raw.len() >= done {
                        raw.truncate(done);
                        break;
                    }
                }
            }
            Err(e) => return Err(io_error(&e)),
        }
    }
    parse(&raw, max_body)
}

fn remaining(deadline: Instant) -> Result<Duration, HttpError> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        Err(HttpError::Timeout)
    } else {
        Ok(left)
    }
}

fn io_error(e: &std::io::Error) -> HttpError {
    match e.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => HttpError::Timeout,
        k => HttpError::Io(k.to_string()),
    }
}

fn header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

/// When headers carry a `Content-Length`, the total response length; lets
/// the reader stop without waiting for the server to close.
fn complete_length(raw: &[u8]) -> Option<usize> {
    let end = header_end(raw)?;
    let head = std::str::from_utf8(&raw[..end]).ok()?;
    head.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case("content-length")
            .then(|| v.trim().parse::<usize>().ok().map(|n| end + n))?
    })
}

fn parse(raw: &[u8], max_body: usize) -> Result<Response, HttpError> {
    let end = header_end(raw).ok_or(HttpError::Malformed("no header terminator"))?;
    let head =
        std::str::from_utf8(&raw[..end]).map_err(|_| HttpError::Malformed("non-UTF-8 headers"))?;
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or(HttpError::Malformed("no status line"))?;
    let mut parts = status_line.splitn(3, ' ');
    let version = parts.next().unwrap_or_default();
    if !version.starts_with("HTTP/1.") {
        return Err(HttpError::Malformed("not HTTP/1.x"));
    }
    let status = parts
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or(HttpError::Malformed("bad status code"))?;
    let mut chunked = false;
    let mut length: Option<usize> = None;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            let k = k.trim();
            if k.eq_ignore_ascii_case("transfer-encoding")
                && v.trim().eq_ignore_ascii_case("chunked")
            {
                chunked = true;
            } else if k.eq_ignore_ascii_case("content-length") {
                length =
                    Some(v.trim().parse().map_err(|_| HttpError::Malformed("bad content-length"))?);
            }
        }
    }
    let rest = &raw[end..];
    let body = if chunked {
        dechunk(rest, max_body)?
    } else if let Some(n) = length {
        if rest.len() < n {
            return Err(HttpError::Malformed("truncated body"));
        }
        rest[..n].to_vec()
    } else {
        rest.to_vec()
    };
    if body.len() > max_body {
        return Err(HttpError::TooLarge(max_body));
    }
    Ok(Response { status, body })
}

fn dechunk(mut rest: &[u8], max_body: usize) -> Result<Vec<u8>, HttpError> {
    let mut out = Vec::new();
    loop {
        let line_end = rest
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or(HttpError::Malformed("truncated chunk size"))?;
        let size_text = std::str::from_utf8(&rest[..line_end])
            .map_err(|_| HttpError::Malformed("bad chunk size"))?;
        let size_text = size_text.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| HttpError::Malformed("bad chunk size"))?;
        rest = &rest[line_end + 2..];
        if size == 0 {
            return Ok(out);
        }
        if rest.len() < size + 2 {
            return Err(HttpError::Malformed("truncated chunk"));
        }
        out.extend_from_slice(&rest[..size]);
        if out.len() > max_body {
            return Err(HttpError::TooLarge(max_body));
        }
        rest = &rest[size + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_length_and_chunked() {
        let r = parse(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok", 10).unwrap();
        assert_eq!((r.status, r.body.as_slice()), (200, b"ok".as_slice()));
        let r = parse(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nok\r\n1\r\n!\r\n0\r\n\r\n",
            10,
        )
        .unwrap();
        assert_eq!(r.body, b"ok!");
        assert_eq!(
            parse(b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\nshort", 100),
            Err(HttpError::Malformed("truncated body"))
        );
        assert_eq!(
            parse(b"garbage", 10),
            Err(HttpError::Malformed("no header terminator"))
        );
        assert_eq!(
            parse(b"HTTP/1.1 200 OK\r\n\r\n0123456789AB", 10),
            Err(HttpError::TooLarge(10))
        );
    }
}
