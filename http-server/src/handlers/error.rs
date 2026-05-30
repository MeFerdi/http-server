use std::fs;

use crate::config::ServerConfig;
use crate::http::{HttpResponse, StatusCode};

pub fn error_response(server: Option<&ServerConfig>, status: StatusCode) -> HttpResponse {
    if let Some(server) = server {
        if let Some(path) = server.error_page(status.as_u16()) {
            if let Ok(content) = fs::read(path) {
                return HttpResponse::new(status).with_body(content, "text/html; charset=utf-8");
            }
        }
    }

    let body = format!(
        "<html><body><h1>{} {}</h1></body></html>",
        status.as_u16(),
        status.reason_phrase()
    );
    HttpResponse::new(status).with_body(body.into_bytes(), "text/html; charset=utf-8")
}
