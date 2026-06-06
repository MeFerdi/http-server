use std::collections::HashMap;
use std::path::PathBuf;

use localhost::config::{Config, RouteConfig, ServerConfig};
use localhost::router::{match_route, select_server};

fn route(path: &str) -> RouteConfig {
    RouteConfig {
        path: path.to_string(),
        methods: vec![],
        redirect: None,
        root: PathBuf::from("."),
        default_file: None,
        cgi: None,
        directory_listing: false,
    }
}

fn server(host: &str, port: u16, server_name: Option<&str>, routes: Vec<RouteConfig>) -> ServerConfig {
    ServerConfig {
        host: host.to_string(),
        ports: vec![port],
        server_name: server_name.map(str::to_string),
        error_pages: HashMap::new(),
        client_max_body_size: 0,
        routes,
    }
}

#[test]
fn select_server_prefers_matching_server_name() {
    let config = Config::new(vec![
        server("127.0.0.1", 8080, None, vec![route("/")]),
        server("127.0.0.1", 8080, Some("api.local"), vec![route("/api")]),
    ]);

    let selected = select_server(&config, "127.0.0.1", 8080, Some("api.local"))
        .expect("server selected");
    assert_eq!(selected.server_name.as_deref(), Some("api.local"));
}

#[test]
fn select_server_falls_back_to_default_unnamed_server() {
    let config = Config::new(vec![
        server("127.0.0.1", 8080, None, vec![route("/")]),
        server("127.0.0.1", 8080, Some("api.local"), vec![route("/api")]),
    ]);

    let selected = select_server(&config, "127.0.0.1", 8080, Some("unknown.local"))
        .expect("server selected");
    assert_eq!(selected.server_name, None);
}

#[test]
fn route_match_uses_longest_prefix_without_query_string() {
    let s = server(
        "127.0.0.1",
        8080,
        None,
        vec![route("/"), route("/api"), route("/api/v2")],
    );

    let matched = match_route(&s, "/api/v2/users?id=1").expect("route match");
    assert_eq!(matched.path, "/api/v2");
}
