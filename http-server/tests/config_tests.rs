//! Tests are grouped by the four areas defined in the project spec:
//!   1. HTTP request parsing (stubs here — full coverage in http_parser_tests)
//!   2. Configuration validation
//!   3. Route matching
//!   4. Status code generation (stubs — full coverage in http tests)

use localhost::config::{parse, validate, ConfigError, HttpMethod};


fn minimal_server(host: &str, port: u16, root: &str) -> String {
    format!(
        r#"
server {{
    host        {host}
    port        {port}
    body_limit  1048576

    location / {{
        methods GET POST DELETE
        root    {root}
        default_file index.html
        directory_listing on
    }}
}}
"#
    )
}


#[test]
fn parse_minimal_server() {
    let input = minimal_server("127.0.0.1", 8080, ".");
    let config = parse(&input).expect("should parse");
    assert_eq!(config.servers.len(), 1);
    let s = &config.servers[0];
    assert_eq!(s.host, "127.0.0.1");
    assert_eq!(s.ports, vec![8080]);
    assert_eq!(s.routes.len(), 1);
}

#[test]
fn parse_multiple_ports() {
    let input = r#"
server {
    host 0.0.0.0
    port 8080 8443

    location / {
        root .
    }
}
"#;
    let config = parse(input).expect("should parse");
    assert_eq!(config.servers[0].ports, vec![8080, 8443]);
}

#[test]
fn parse_multiple_servers() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080
    server_name alpha

    location / { root . }
}

server {
    host 127.0.0.1
    port 9090
    server_name beta

    location / { root . }
}
"#;
    let config = parse(input).expect("should parse");
    assert_eq!(config.servers.len(), 2);
    assert_eq!(config.servers[0].server_name, Some("alpha".to_string()));
    assert_eq!(config.servers[1].server_name, Some("beta".to_string()));
}

#[test]
fn parse_error_pages() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080
    error_page 404 ./www/404.html
    error_page 500 ./www/500.html

    location / { root . }
}
"#;
    let config = parse(input).expect("should parse");
    let s = &config.servers[0];
    assert!(s.error_pages.contains_key(&404));
    assert!(s.error_pages.contains_key(&500));
}

#[test]
fn parse_cgi_directive() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080

    location /cgi-bin {
        methods GET POST
        root    .
        cgi     .py /usr/bin/python3
    }
}
"#;
    let config = parse(input).expect("should parse");
    let route = &config.servers[0].routes[0];
    assert!(route.cgi.is_some());
    let (ext, _interp) = route.cgi.as_ref().unwrap();
    assert_eq!(ext, ".py");
}

#[test]
fn parse_redirect_directive() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080

    location /old {
        redirect /new
    }
}
"#;
    let config = parse(input).expect("should parse");
    let route = &config.servers[0].routes[0];
    assert_eq!(route.redirect, Some("/new".to_string()));
}

#[test]
fn parse_directory_listing_on_off() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080

    location /on  { root . directory_listing on  }
    location /off { root . directory_listing off }
}
"#;
    let config = parse(input).expect("should parse");
    assert!(config.servers[0].routes[0].directory_listing);
    assert!(!config.servers[0].routes[1].directory_listing);
}

#[test]
fn parse_body_limit_zero_means_unlimited() {
    let input = r#"
server {
    host 0.0.0.0
    port 8080
    body_limit 0

    location / { root . }
}
"#;
    let config = parse(input).expect("should parse");
    assert_eq!(config.servers[0].client_max_body_size, 0);
}


#[test]
fn error_missing_host() {
    let input = r#"
server {
    port 8080
    location / { root . }
}
"#;
    let result = parse(input);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ConfigError::MissingField { field: "host", .. }));
}

#[test]
fn error_missing_port() {
    let input = r#"
server {
    host 127.0.0.1
    location / { root . }
}
"#;
    let result = parse(input);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ConfigError::MissingField { field: "port", .. }));
}

#[test]
fn error_invalid_port_string() {
    let input = r#"
server {
    host 127.0.0.1
    port banana
    location / { root . }
}
"#;
    let result = parse(input);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ConfigError::InvalidValue { field: "port", .. }));
}

#[test]
fn error_port_zero_rejected() {
    let input = r#"
server {
    host 127.0.0.1
    port 0
    location / { root . }
}
"#;
    let result = parse(input);
    assert!(result.is_err());
}

#[test]
fn error_port_out_of_range() {
    let input = r#"
server {
    host 127.0.0.1
    port 99999
    location / { root . }
}
"#;
    // 99999 > u16::MAX, should fail
    let result = parse(input);
    assert!(result.is_err());
}

#[test]
fn error_invalid_body_limit() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080
    body_limit -1
    location / { root . }
}
"#;
    let result = parse(input);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ConfigError::InvalidBodySize { .. }));
}

#[test]
fn error_unknown_server_directive() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080
    foobar something
    location / { root . }
}
"#;
    let result = parse(input);
    assert!(result.is_err());
}

#[test]
fn error_unknown_location_directive() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080

    location / {
        root .
        turbo_mode on
    }
}
"#;
    let result = parse(input);
    assert!(result.is_err());
}

#[test]
fn error_invalid_method() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080

    location / {
        methods PATCH
        root .
    }
}
"#;
    let result = parse(input);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ConfigError::InvalidValue { field: "methods", .. }));
}

#[test]
fn error_unclosed_brace() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080

    location / {
        root .
"#;
    // The parser should return an error (missing closing braces)
    let result = parse(input);
    assert!(result.is_err());
}

#[test]
fn error_top_level_non_server_token() {
    let input = r#"
worker_processes 4;
server {
    host 127.0.0.1
    port 8080
    location / { root . }
}
"#;
    let result = parse(input);
    assert!(result.is_err());
}


#[test]
fn validate_no_routes_error() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080
}
"#;
    let config = parse(input).expect("should parse");
    let errors = validate(&config).unwrap_err();
    assert!(errors.iter().any(|e| matches!(e, ConfigError::NoRoutes { .. })));
}

#[test]
fn validate_conflicting_default_servers() {
    // Two server blocks without server_name on the same host:port conflict.
    let input = r#"
server {
    host 127.0.0.1
    port 8080
    location / { root . }
}

server {
    host 127.0.0.1
    port 8080
    location /api { root . }
}
"#;
    let config = parse(input).expect("should parse");
    let errors = validate(&config).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ConfigError::ConflictingPort { port: 8080, .. })));
}

#[test]
fn validate_named_servers_same_port_ok() {
    // Named servers (server_name set) on the same host:port are fine.
    let input = r#"
server {
    host 127.0.0.1
    port 8080
    server_name alpha.local
    location / { root . }
}

server {
    host 127.0.0.1
    port 8080
    server_name beta.local
    location / { root . }
}
"#;
    let config = parse(input).expect("should parse");
    // Named servers don't conflict — validation should pass (no ConflictingPort)
    let errors_opt = validate(&config);
    // There might be PathNotFound errors (. does exist so actually fine),
    // but no ConflictingPort
    match errors_opt {
        Ok(_) => { /* pass */ }
        Err(errors) => {
            let has_conflict = errors.iter().any(|e| matches!(e, ConfigError::ConflictingPort { .. }));
            assert!(!has_conflict, "named servers should not conflict: {:?}", errors);
        }
    }
}

#[test]
fn validate_missing_error_page_path() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080
    error_page 404 /nonexistent/path/to/404.html

    location / { root . }
}
"#;
    let config = parse(input).expect("should parse");
    let errors = validate(&config).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ConfigError::PathNotFound { field: "error_page", .. })));
}

#[test]
fn validate_missing_root_path() {
    let input = r#"
server {
    host 127.0.0.1
    port 8080

    location / {
        root /this/path/does/not/exist/at/all
    }
}
"#;
    let config = parse(input).expect("should parse");
    let errors = validate(&config).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ConfigError::PathNotFound { field: "root", .. })));
}

#[test]
fn validate_multiple_errors_collected() {
    // Both missing error_page and conflicting port — all errors should be reported.
    let input = r#"
server {
    host 127.0.0.1
    port 8080
    error_page 404 /does/not/exist.html
    location / { root /also/missing }
}

server {
    host 127.0.0.1
    port 8080
    location / { root /also/missing }
}
"#;
    let config = parse(input).expect("should parse");
    let errors = validate(&config).unwrap_err();
    assert!(errors.len() >= 2, "expected multiple errors, got: {:?}", errors);
}


/// Build a ServerConfig with the given route paths (all pointing to ".").
fn server_with_routes(paths: &[&str]) -> localhost::config::ServerConfig {
    use localhost::config::{RouteConfig, ServerConfig};
    use std::collections::HashMap;
    use std::path::PathBuf;

    let routes = paths
        .iter()
        .map(|p| RouteConfig {
            path: p.to_string(),
            methods: vec![],
            redirect: None,
            root: PathBuf::from("."),
            default_file: None,
            cgi: None,
            directory_listing: false,
        })
        .collect();

    ServerConfig {
        host: "127.0.0.1".to_string(),
        ports: vec![8080],
        server_name: None,
        error_pages: HashMap::new(),
        client_max_body_size: 0,
        routes,
    }
}

#[test]
fn route_exact_match() {
    let s = server_with_routes(&["/", "/about", "/api"]);
    let r = s.match_route("/about").expect("should match");
    assert_eq!(r.path, "/about");
}

#[test]
fn route_longest_prefix_wins() {
    let s = server_with_routes(&["/", "/api", "/api/v2"]);
    let r = s.match_route("/api/v2/users").expect("should match");
    assert_eq!(r.path, "/api/v2");
}

#[test]
fn route_fallback_to_root() {
    let s = server_with_routes(&["/", "/api"]);
    let r = s.match_route("/unknown/path").expect("should match root");
    assert_eq!(r.path, "/");
}

#[test]
fn route_no_false_prefix_match() {
    // "/api2" should NOT match route "/api" — only if next char is '/' or end
    let s = server_with_routes(&["/", "/api"]);
    let r = s.match_route("/api2/something").expect("should match root");
    assert_eq!(r.path, "/", "'/api' should not match '/api2/...'");
}

#[test]
fn route_exact_path_match() {
    let s = server_with_routes(&["/", "/api"]);
    let r = s.match_route("/api").expect("should match /api exactly");
    assert_eq!(r.path, "/api");
}

#[test]
fn route_empty_routes_returns_none() {
    use localhost::config::ServerConfig;
    use std::collections::HashMap;
    let s = ServerConfig {
        host: "127.0.0.1".to_string(),
        ports: vec![8080],
        server_name: None,
        error_pages: HashMap::new(),
        client_max_body_size: 0,
        routes: vec![],
    };
    assert!(s.match_route("/anything").is_none());
}

// HttpMethod helpers

#[test]
fn method_from_str_case_insensitive() {
    assert_eq!(HttpMethod::from_str("get"), Some(HttpMethod::Get));
    assert_eq!(HttpMethod::from_str("GET"), Some(HttpMethod::Get));
    assert_eq!(HttpMethod::from_str("Post"), Some(HttpMethod::Post));
    assert_eq!(HttpMethod::from_str("DELETE"), Some(HttpMethod::Delete));
    assert_eq!(HttpMethod::from_str("PATCH"), None);
    assert_eq!(HttpMethod::from_str(""), None);
}

#[test]
fn route_allows_method() {
    use localhost::config::RouteConfig;
    use std::path::PathBuf;

    let route_all = RouteConfig {
        path: "/".to_string(),
        methods: vec![],
        redirect: None,
        root: PathBuf::from("."),
        default_file: None,
        cgi: None,
        directory_listing: false,
    };
    // Empty methods = all allowed
    assert!(route_all.allows_method(&HttpMethod::Get));
    assert!(route_all.allows_method(&HttpMethod::Post));
    assert!(route_all.allows_method(&HttpMethod::Delete));

    let route_get_only = RouteConfig {
        methods: vec![HttpMethod::Get],
        ..route_all.clone()
    };
    assert!(route_get_only.allows_method(&HttpMethod::Get));
    assert!(!route_get_only.allows_method(&HttpMethod::Post));
    assert!(!route_get_only.allows_method(&HttpMethod::Delete));
}

// Config identity and display

#[test]
fn server_identity_single_port() {
    let input = minimal_server("127.0.0.1", 8080, ".");
    let config = parse(&input).expect("should parse");
    assert_eq!(config.servers[0].identity(), "127.0.0.1:8080");
}

#[test]
fn server_identity_multi_port() {
    let input = r#"
server {
    host 0.0.0.0
    port 80 443

    location / { root . }
}
"#;
    let config = parse(&input).expect("should parse");
    assert_eq!(config.servers[0].identity(), "0.0.0.0:80,443");
}

#[test]
fn config_error_display_is_non_empty() {
    let errors = vec![
        ConfigError::MissingField { block: "server", field: "host" },
        ConfigError::ConflictingPort { host: "127.0.0.1".to_string(), port: 8080 },
        ConfigError::InvalidBodySize { value: "-1".to_string() },
        ConfigError::NoRoutes { server: "127.0.0.1:8080".to_string() },
    ];
    for e in &errors {
        let s = e.to_string();
        assert!(!s.is_empty(), "Display for {:?} returned empty string", e);
    }
}
