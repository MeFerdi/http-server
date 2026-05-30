use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::RouteConfig;
use crate::http::{HttpRequest, HttpResponse, StatusCode};

pub fn cgi_response(request: &HttpRequest, route: &RouteConfig) -> HttpResponse {
    let Some((ext, interpreter)) = &route.cgi else {
        return HttpResponse::new(StatusCode::InternalServerError)
            .with_body(b"CGI not configured".to_vec(), "text/plain; charset=utf-8");
    };

    let (path_only, query) = split_path_and_query(&request.path);
    let Some(script_path) = cgi_script_path(path_only, route) else {
        return HttpResponse::new(StatusCode::NotFound)
            .with_body(b"Not Found".to_vec(), "text/plain; charset=utf-8");
    };

    if !script_path.exists() || !script_path.is_file() {
        return HttpResponse::new(StatusCode::NotFound)
            .with_body(b"Not Found".to_vec(), "text/plain; charset=utf-8");
    }

    if script_path.extension().and_then(|e| e.to_str()) != Some(ext.trim_start_matches('.')) {
        return HttpResponse::new(StatusCode::Forbidden)
            .with_body(b"Forbidden".to_vec(), "text/plain; charset=utf-8");
    }

    let mut cmd = Command::new(interpreter);
    cmd.arg(&script_path);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    cmd.env("PATH_INFO", path_only);
    cmd.env("REQUEST_METHOD", request.method.as_str());
    cmd.env("QUERY_STRING", query);
    cmd.env(
        "CONTENT_TYPE",
        request.header("content-type").unwrap_or(""),
    );
    cmd.env("CONTENT_LENGTH", request.body.len().to_string());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            return HttpResponse::new(StatusCode::InternalServerError)
                .with_body(b"CGI spawn failed".to_vec(), "text/plain; charset=utf-8")
        }
    };

    if let Some(stdin) = child.stdin.as_mut() {
        if stdin.write_all(&request.body).is_err() {
            return HttpResponse::new(StatusCode::InternalServerError)
                .with_body(b"CGI stdin write failed".to_vec(), "text/plain; charset=utf-8");
        }
    }

    let output = match child.wait_with_output() {
        Ok(out) => out,
        Err(_) => {
            return HttpResponse::new(StatusCode::InternalServerError)
                .with_body(b"CGI execution failed".to_vec(), "text/plain; charset=utf-8");
        }
    };

    if !output.status.success() {
        return HttpResponse::new(StatusCode::InternalServerError)
            .with_body(output.stderr, "text/plain; charset=utf-8");
    }

    parse_cgi_output(&output.stdout)
}

fn cgi_script_path(path_only: &str, route: &RouteConfig) -> Option<PathBuf> {
    let rel = strip_route_prefix(path_only, &route.path)?;
    let rel = rel.trim_start_matches('/');
    Some(route.root.join(rel))
}

fn strip_route_prefix<'a>(path: &'a str, route_path: &str) -> Option<&'a str> {
    if route_path == "/" {
        return Some(path);
    }
    path.strip_prefix(route_path)
}

fn split_path_and_query(path: &str) -> (&str, &str) {
    if let Some((p, q)) = path.split_once('?') {
        (p, q)
    } else {
        (path, "")
    }
}

fn parse_cgi_output(raw: &[u8]) -> HttpResponse {
    let split = find_header_end(raw);
    let (head, body) = if let Some(i) = split {
        (&raw[..i], &raw[(i + header_terminator_len(raw, i))..])
    } else {
        (&[][..], raw)
    };

    let mut headers = HashMap::new();
    let mut status = StatusCode::Ok;

    if !head.is_empty() {
        let head_str = String::from_utf8_lossy(head);
        for line in head_str.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if k.eq_ignore_ascii_case("Status") {
                    let code = v
                        .split_whitespace()
                        .next()
                        .and_then(|c| c.parse::<u16>().ok())
                        .and_then(StatusCode::from_u16);
                    if let Some(code) = code {
                        status = code;
                    }
                } else {
                    headers.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }

    let mut response = HttpResponse::new(status).with_body(
        body.to_vec(),
        headers
            .get("Content-Type")
            .map(String::as_str)
            .unwrap_or("text/plain; charset=utf-8"),
    );

    for (k, v) in headers {
        response.headers.insert(k, v);
    }

    response
}

fn find_header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .or_else(|| raw.windows(2).position(|w| w == b"\n\n"))
}

fn header_terminator_len(raw: &[u8], idx: usize) -> usize {
    if raw.get(idx..(idx + 4)) == Some(b"\r\n\r\n") {
        4
    } else {
        2
    }
}

#[allow(dead_code)]
fn _normalize_path(path: &Path) -> String {
    path.display().to_string()
}
