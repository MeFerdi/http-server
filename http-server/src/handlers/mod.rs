pub mod cgi;
pub mod error;
pub mod redirect;
pub mod static_file;
pub mod upload;

use crate::config::{Config, HttpMethod, RouteConfig, ServerConfig};
use crate::http::{HttpRequest, HttpResponse, Method, StatusCode};
use crate::router::{match_route, select_server};
use crate::session::{parse_cookie_header, SessionStore};

pub fn handle_request(
	config: &Config,
	local_host: &str,
	local_port: u16,
	sessions: &mut SessionStore,
	request: &HttpRequest,
) -> HttpResponse {
	let cookies = request
		.header("cookie")
		.map(parse_cookie_header)
		.unwrap_or_default();
	let (session_id, is_new_session) = sessions.get_or_create(cookies.get("session_id").map(String::as_str));
	if let Some(session_data) = sessions.get_mut(&session_id) {
		session_data
			.values
			.entry("last_path".to_string())
			.or_insert_with(|| request.path.clone());
	}

	let server = select_server(config, local_host, local_port, request.header("host"));
	let Some(server) = server else {
		let mut response = error::error_response(None, StatusCode::NotFound);
		attach_session_cookie_if_needed(&mut response, &session_id, is_new_session);
		return response;
	};

	let route = match_route(server, &request.path);
	let Some(route) = route else {
		let mut response = error::error_response(Some(server), StatusCode::NotFound);
		attach_session_cookie_if_needed(&mut response, &session_id, is_new_session);
		return response;
	};

	if !route.allows_method(&to_config_method(&request.method)) {
		let mut response = error::error_response(Some(server), StatusCode::MethodNotAllowed);
		attach_session_cookie_if_needed(&mut response, &session_id, is_new_session);
		return response;
	}

	let response = if let Some(location) = &route.redirect {
		redirect::redirect_response(location)
	} else if route.cgi.is_some() {
		cgi::cgi_response(request, route)
	} else if request.method == Method::Post && is_multipart(request) {
		upload::upload_response(request, route)
	} else {
		static_file::static_file_response(request, route)
	};

	let mut response = maybe_override_with_custom_error_page(server, response);
	attach_session_cookie_if_needed(&mut response, &session_id, is_new_session);
	response
}

fn attach_session_cookie_if_needed(response: &mut HttpResponse, session_id: &str, should_set: bool) {
	if should_set {
		response.headers.insert(
			"Set-Cookie".to_string(),
			format!("session_id={}; Path=/; HttpOnly", session_id),
		);
	}
}

fn maybe_override_with_custom_error_page(server: &ServerConfig, response: HttpResponse) -> HttpResponse {
	if response.status.as_u16() >= 400 {
		return error::error_response(Some(server), response.status);
	}
	response
}

fn to_config_method(method: &Method) -> HttpMethod {
	match method {
		Method::Get => HttpMethod::Get,
		Method::Post => HttpMethod::Post,
		Method::Delete => HttpMethod::Delete,
	}
}

fn is_multipart(request: &HttpRequest) -> bool {
	request
		.header("content-type")
		.map(|v| v.to_ascii_lowercase().starts_with("multipart/form-data"))
		.unwrap_or(false)
}

#[allow(dead_code)]
fn _route_identity(route: &RouteConfig) -> String {
	route.path.clone()
}
