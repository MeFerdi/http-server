use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::config::RouteConfig;
use crate::http::{HttpRequest, HttpResponse, Method, StatusCode};

pub fn static_file_response(request: &HttpRequest, route: &RouteConfig) -> HttpResponse {
    if request.method != Method::Get {
        return HttpResponse::new(StatusCode::MethodNotAllowed)
            .with_body(b"Method Not Allowed".to_vec(), "text/plain; charset=utf-8");
    }

    let relative = request.path.trim_start_matches('/');
    let safe_relative = sanitize_relative_path(relative);
    let candidate = route.root.join(safe_relative);

    if let Some(path) = resolve_target_path(&candidate, route) {
        if path.is_file() {
            match fs::read(&path) {
                Ok(bytes) => {
                    let content_type = guess_content_type(&path);
                    return HttpResponse::new(StatusCode::Ok).with_body(bytes, content_type);
                }
                Err(_) => {
                    return HttpResponse::new(StatusCode::Forbidden)
                        .with_body(b"Forbidden".to_vec(), "text/plain; charset=utf-8");
                }
            }
        }

        if path.is_dir() && route.directory_listing {
            let body = directory_listing_html(&path);
            return HttpResponse::new(StatusCode::Ok)
                .with_body(body.into_bytes(), "text/html; charset=utf-8");
        }
    }

    HttpResponse::new(StatusCode::NotFound).with_body(b"Not Found".to_vec(), "text/plain; charset=utf-8")
}

fn resolve_target_path(candidate: &Path, route: &RouteConfig) -> Option<PathBuf> {
    if candidate.is_file() {
        return Some(candidate.to_path_buf());
    }

    if candidate.is_dir() {
        if let Some(default_file) = &route.default_file {
            let with_default = candidate.join(default_file);
            if with_default.is_file() {
                return Some(with_default);
            }
        }
        return Some(candidate.to_path_buf());
    }

    None
}

fn sanitize_relative_path(input: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for c in Path::new(input).components() {
        match c {
            Component::Normal(segment) => out.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    out
}

fn guess_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn directory_listing_html(path: &Path) -> String {
    let mut html = String::from("<html><body><h1>Index of ");
    html.push_str(&path.display().to_string());
    html.push_str("</h1><ul>");

    if let Ok(entries) = fs::read_dir(path) {
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        for name in names {
            html.push_str("<li>");
            html.push_str(&name);
            html.push_str("</li>");
        }
    }

    html.push_str("</ul></body></html>");
    html
}
