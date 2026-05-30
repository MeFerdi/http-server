use std::collections::HashMap;

use super::method::Method;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: Method,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub keep_alive: bool,
}

impl HttpRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpParseError {
    Incomplete,
    EmptyMethod,
    InvalidRequestLine,
    UnsupportedMethod(String),
    InvalidVersion(String),
    MalformedHeader(String),
    InvalidContentLength(String),
    InvalidChunkSize(String),
    BodyTooLarge { limit: usize, actual: usize },
}

pub fn parse_request(raw: &[u8], max_body_size: usize) -> Result<HttpRequest, HttpParseError> {
    let header_end = find_subsequence(raw, b"\r\n\r\n").ok_or(HttpParseError::Incomplete)?;
    let head = &raw[..header_end];
    let mut lines = head.split(|b| *b == b'\n').map(trim_cr);

    let request_line = lines.next().ok_or(HttpParseError::InvalidRequestLine)?;
    let request_line = std::str::from_utf8(request_line).map_err(|_| HttpParseError::InvalidRequestLine)?;
    if request_line.starts_with(char::is_whitespace) {
        return Err(HttpParseError::EmptyMethod);
    }
    let mut parts = request_line.split_whitespace();

    let method_raw = parts.next().ok_or(HttpParseError::EmptyMethod)?;
    if method_raw.is_empty() {
        return Err(HttpParseError::EmptyMethod);
    }

    let path = parts.next().ok_or(HttpParseError::InvalidRequestLine)?;
    let version = parts.next().ok_or(HttpParseError::InvalidRequestLine)?;
    if parts.next().is_some() {
        return Err(HttpParseError::InvalidRequestLine);
    }

    let method = Method::from_str(method_raw)
        .ok_or_else(|| HttpParseError::UnsupportedMethod(method_raw.to_string()))?;

    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(HttpParseError::InvalidVersion(version.to_string()));
    }

    let mut headers: HashMap<String, String> = HashMap::new();
    for raw_line in lines {
        if raw_line.is_empty() {
            continue;
        }

        let line = std::str::from_utf8(raw_line)
            .map_err(|_| HttpParseError::MalformedHeader("non-utf8 header".to_string()))?;

        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim();
            if name.is_empty() {
                return Err(HttpParseError::MalformedHeader(line.to_string()));
            }

            headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
        } else {
            return Err(HttpParseError::MalformedHeader(line.to_string()));
        }
    }

    let body_bytes = &raw[(header_end + 4)..];
    let body = if is_chunked(&headers) {
        parse_chunked_body(body_bytes)?
    } else {
        parse_content_length_body(&headers, body_bytes)?
    };

    if max_body_size > 0 && body.len() > max_body_size {
        return Err(HttpParseError::BodyTooLarge {
            limit: max_body_size,
            actual: body.len(),
        });
    }

    let keep_alive = compute_keep_alive(version, headers.get("connection"));

    Ok(HttpRequest {
        method,
        path: path.to_string(),
        version: version.to_string(),
        headers,
        body,
        keep_alive,
    })
}

fn trim_cr(line: &[u8]) -> &[u8] {
    if let Some(stripped) = line.strip_suffix(b"\r") {
        stripped
    } else {
        line
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn is_chunked(headers: &HashMap<String, String>) -> bool {
    headers
        .get("transfer-encoding")
        .map(|v| {
            v.split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("chunked"))
        })
        .unwrap_or(false)
}

fn parse_content_length_body(
    headers: &HashMap<String, String>,
    body_bytes: &[u8],
) -> Result<Vec<u8>, HttpParseError> {
    match headers.get("content-length") {
        None => Ok(Vec::new()),
        Some(raw_len) => {
            let length = raw_len
                .trim()
                .parse::<usize>()
                .map_err(|_| HttpParseError::InvalidContentLength(raw_len.clone()))?;

            if body_bytes.len() < length {
                return Err(HttpParseError::Incomplete);
            }

            Ok(body_bytes[..length].to_vec())
        }
    }
}

fn parse_chunked_body(body_bytes: &[u8]) -> Result<Vec<u8>, HttpParseError> {
    let mut out = Vec::new();
    let mut i = 0usize;

    loop {
        let Some(line_end_rel) = find_subsequence(&body_bytes[i..], b"\r\n") else {
            return Err(HttpParseError::Incomplete);
        };

        let line_end = i + line_end_rel;
        let size_line = std::str::from_utf8(&body_bytes[i..line_end])
            .map_err(|_| HttpParseError::InvalidChunkSize("non-utf8 chunk size".to_string()))?;

        let size_token = size_line.split(';').next().unwrap_or("").trim();
        let chunk_size = usize::from_str_radix(size_token, 16)
            .map_err(|_| HttpParseError::InvalidChunkSize(size_line.to_string()))?;

        i = line_end + 2;

        if chunk_size == 0 {
            if body_bytes.get(i..(i + 2)) != Some(b"\r\n") {
                return Err(HttpParseError::Incomplete);
            }
            break;
        }

        if body_bytes.len() < i + chunk_size + 2 {
            return Err(HttpParseError::Incomplete);
        }

        out.extend_from_slice(&body_bytes[i..(i + chunk_size)]);
        i += chunk_size;

        if body_bytes.get(i..(i + 2)) != Some(b"\r\n") {
            return Err(HttpParseError::Incomplete);
        }
        i += 2;
    }

    Ok(out)
}

fn compute_keep_alive(version: &str, connection_header: Option<&String>) -> bool {
    match version {
        "HTTP/1.1" => !connection_header
            .map(|v| v.eq_ignore_ascii_case("close"))
            .unwrap_or(false),
        "HTTP/1.0" => connection_header
            .map(|v| v.eq_ignore_ascii_case("keep-alive"))
            .unwrap_or(false),
        _ => false,
    }
}
