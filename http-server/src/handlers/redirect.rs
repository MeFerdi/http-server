use crate::http::{HttpResponse, StatusCode};

pub fn redirect_response(location: &str) -> HttpResponse {
    let mut response = HttpResponse::new(StatusCode::MovedPermanently).with_body(
        format!("Moved Permanently: {}", location).into_bytes(),
        "text/plain; charset=utf-8",
    );
    response
        .headers
        .insert("Location".to_string(), location.to_string());
    response
}
