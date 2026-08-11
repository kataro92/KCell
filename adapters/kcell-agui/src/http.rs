//! Minimal HTTP/1.1 request reader + SSE / JSON response writers.

use std::io::{Read, Write};
use std::net::TcpStream;

use serde_json::Value;

#[derive(Debug)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
}

pub fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        let n = stream
            .read(&mut tmp)
            .map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(header_end) = find_header_end(&buf) {
            let headers = std::str::from_utf8(&buf[..header_end])
                .map_err(|e| format!("headers utf8: {e}"))?;
            let mut lines = headers.lines();
            let request_line = lines.next().ok_or_else(|| "empty request".to_string())?;
            let mut parts = request_line.split_whitespace();
            let method = parts
                .next()
                .ok_or_else(|| "missing method".to_string())?
                .to_string();
            let path = parts
                .next()
                .ok_or_else(|| "missing path".to_string())?
                .to_string();

            let mut content_length = 0usize;
            for line in lines {
                let lower = line.to_ascii_lowercase();
                if let Some(rest) = lower.strip_prefix("content-length:") {
                    content_length = rest.trim().parse().unwrap_or(0);
                }
            }

            let body_start = header_end + 4; // \r\n\r\n
            while buf.len() < body_start + content_length {
                let n = stream
                    .read(&mut tmp)
                    .map_err(|e| format!("read body: {e}"))?;
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            let body = buf
                .get(body_start..body_start + content_length)
                .unwrap_or(&[])
                .to_vec();
            return Ok(HttpRequest { method, path, body });
        }
        if buf.len() > 1024 * 1024 {
            return Err("request too large".into());
        }
    }
    Err("incomplete request".into())
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

pub fn write_json(stream: &mut TcpStream, status: u16, reason: &str, body: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(body).map_err(|e| e.to_string())?;
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|e| format!("write header: {e}"))?;
    stream
        .write_all(&bytes)
        .map_err(|e| format!("write body: {e}"))?;
    stream.flush().map_err(|e| format!("flush: {e}"))?;
    Ok(())
}

pub fn write_sse_headers(stream: &mut TcpStream) -> Result<(), String> {
    let header = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n";
    stream
        .write_all(header.as_bytes())
        .map_err(|e| format!("write sse headers: {e}"))?;
    stream.flush().map_err(|e| format!("flush: {e}"))?;
    Ok(())
}

pub fn write_sse_event(stream: &mut TcpStream, event: &Value) -> Result<(), String> {
    let line = format_sse_data(event)?;
    stream
        .write_all(line.as_bytes())
        .map_err(|e| format!("write sse: {e}"))?;
    stream.flush().map_err(|e| format!("flush: {e}"))?;
    Ok(())
}

pub fn format_sse_data(event: &Value) -> Result<String, String> {
    let data = serde_json::to_string(event).map_err(|e| e.to_string())?;
    Ok(format!("data: {data}\n\n"))
}

pub fn write_plain(stream: &mut TcpStream, status: u16, reason: &str, body: &str) -> Result<(), String> {
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|e| format!("write: {e}"))?;
    stream.flush().map_err(|e| format!("flush: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sse_line_format() {
        let line = format_sse_data(&json!({"type":"RUN_STARTED","runId":"r1"})).unwrap();
        assert!(line.starts_with("data: {"));
        assert!(line.ends_with("\n\n"));
        assert!(line.contains("RUN_STARTED"));
    }
}
