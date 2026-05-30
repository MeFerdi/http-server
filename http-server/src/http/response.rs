use std::collections::HashMap;

use super::status::StatusCode;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: StatusCode,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status: StatusCode) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }

    pub fn with_body(mut self, body: Vec<u8>, content_type: &str) -> Self {
        self.headers
            .insert("Content-Type".to_string(), content_type.to_string());
        self.body = body;
        self
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(
            format!(
                "HTTP/1.1 {} {}\r\n",
                self.status.as_u16(),
                self.status.reason_phrase()
            )
            .as_bytes(),
        );

        let mut headers = self.headers.clone();
        headers
            .entry("Content-Length".to_string())
            .or_insert_with(|| self.body.len().to_string());
        headers
            .entry("Connection".to_string())
            .or_insert_with(|| "close".to_string());

        for (k, v) in headers {
            out.extend_from_slice(format!("{}: {}\r\n", k, v).as_bytes());
        }

        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(&self.body);
        out
    }
}
