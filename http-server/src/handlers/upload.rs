use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::RouteConfig;
use crate::http::{HttpRequest, HttpResponse, StatusCode};

pub fn upload_response(request: &HttpRequest, route: &RouteConfig) -> HttpResponse {
    let content_type = request.header("content-type").unwrap_or("");
    let Some(boundary) = multipart_boundary(content_type) else {
        return HttpResponse::new(StatusCode::BadRequest)
            .with_body(b"Missing multipart boundary".to_vec(), "text/plain; charset=utf-8");
    };

    if fs::create_dir_all(&route.root).is_err() {
        return HttpResponse::new(StatusCode::InternalServerError)
            .with_body(b"Failed to prepare upload directory".to_vec(), "text/plain; charset=utf-8");
    }

    let files = parse_multipart_files(&request.body, &boundary);
    if files.is_empty() {
        return HttpResponse::new(StatusCode::BadRequest)
            .with_body(b"No files in multipart payload".to_vec(), "text/plain; charset=utf-8");
    }

    let mut saved = 0usize;
    for file in files {
        let safe_name = sanitize_filename(file.filename.as_deref().unwrap_or("upload.bin"));
        let mut target = route.root.join(&safe_name);
        if target.exists() {
            target = route.root.join(unique_prefixed_name(&safe_name));
        }

        if fs::write(&target, &file.content).is_ok() {
            saved += 1;
        }
    }

    if saved == 0 {
        return HttpResponse::new(StatusCode::InternalServerError)
            .with_body(b"Failed to save uploaded files".to_vec(), "text/plain; charset=utf-8");
    }

    HttpResponse::new(StatusCode::Created).with_body(
        format!("saved_files={}\n", saved).into_bytes(),
        "text/plain; charset=utf-8",
    )
}

struct MultipartFile {
    filename: Option<String>,
    content: Vec<u8>,
}

fn multipart_boundary(content_type: &str) -> Option<String> {
    for part in content_type.split(';') {
        let trimmed = part.trim();
        if let Some(value) = trimmed.strip_prefix("boundary=") {
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

fn parse_multipart_files(body: &[u8], boundary: &str) -> Vec<MultipartFile> {
    let marker = format!("--{}", boundary).into_bytes();
    let mut out = Vec::new();

    let mut start = 0usize;
    while let Some(idx) = find_subsequence(&body[start..], &marker) {
        let part_start = start + idx + marker.len();

        if body.get(part_start..(part_start + 2)) == Some(b"--") {
            break;
        }

        let mut p = part_start;
        if body.get(p..(p + 2)) == Some(b"\r\n") {
            p += 2;
        }

        let Some(next_rel) = find_subsequence(&body[p..], &marker) else {
            break;
        };
        let part_end = p + next_rel;
        let part = &body[p..part_end];

        if let Some(file) = parse_single_part(part) {
            out.push(file);
        }

        start = part_end;
    }

    out
}

fn parse_single_part(part: &[u8]) -> Option<MultipartFile> {
    let header_end = find_subsequence(part, b"\r\n\r\n")?;
    let headers = String::from_utf8_lossy(&part[..header_end]);
    let mut filename: Option<String> = None;

    for line in headers.lines() {
        if line.to_ascii_lowercase().starts_with("content-disposition:") {
            if let Some(name) = extract_filename(line) {
                filename = Some(name);
            }
        }
    }

    let mut content = part[(header_end + 4)..].to_vec();
    if content.ends_with(b"\r\n") {
        content.truncate(content.len().saturating_sub(2));
    }

    Some(MultipartFile { filename, content })
}

fn extract_filename(content_disposition: &str) -> Option<String> {
    for token in content_disposition.split(';') {
        let token = token.trim();
        if let Some(v) = token.strip_prefix("filename=") {
            return Some(v.trim_matches('"').to_string());
        }
    }
    None
}

fn sanitize_filename(raw: &str) -> String {
    let candidate = Path::new(raw)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("upload.bin");

    let clean: String = candidate
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if clean.is_empty() {
        "upload.bin".to_string()
    } else {
        clean
    }
}

fn unique_prefixed_name(base: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{}_{}", now, base)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[allow(dead_code)]
fn _path_to_string(path: &PathBuf) -> String {
    path.display().to_string()
}
