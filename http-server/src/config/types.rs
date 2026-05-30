use std::collections::HashMap;
use std::path::PathBuf;

/// HTTP methods supported by the server.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Delete,
}

impl HttpMethod {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GET"    => Some(HttpMethod::Get),
            "POST"   => Some(HttpMethod::Post),
            "DELETE" => Some(HttpMethod::Delete),
            _        => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get    => "GET",
            HttpMethod::Post   => "POST",
            HttpMethod::Delete => "DELETE",
        }
    }
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Configuration for a single route (location block).
#[derive(Debug, Clone)]
pub struct RouteConfig {
    /// URL prefix this route matches, e.g. "/", "/api", "/uploads"
    pub path: String,
    /// Allowed HTTP methods. Empty means all supported methods are allowed.
    pub methods: Vec<HttpMethod>,
    /// If set, respond with a 301 redirect to this URL.
    pub redirect: Option<String>,
    /// Filesystem root for this route.
    pub root: PathBuf,
    /// Default file to serve when the URL resolves to a directory.
    pub default_file: Option<String>,
    /// CGI: (extension, interpreter_path) e.g. (".py", "/usr/bin/python3")
    pub cgi: Option<(String, PathBuf)>,
    /// Whether to generate an HTML directory listing.
    pub directory_listing: bool,
}

impl RouteConfig {
    pub fn allows_method(&self, method: &HttpMethod) -> bool {
        self.methods.is_empty() || self.methods.contains(method)
    }
}

/// Top-level configuration for one virtual server block.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub ports: Vec<u16>,
    pub server_name: Option<String>,
    pub error_pages: HashMap<u16, PathBuf>,
    /// Maximum request body size in bytes. 0 means unlimited.
    pub client_max_body_size: usize,
    pub routes: Vec<RouteConfig>,
}

impl ServerConfig {
    /// Find the route with the longest matching prefix.
    pub fn match_route(&self, request_path: &str) -> Option<&RouteConfig> {
        self.routes
            .iter()
            .filter(|r| {
                let p = request_path;
                p.starts_with(&r.path)
                    && (r.path.ends_with('/')
                        || p.len() == r.path.len()
                        || p.as_bytes().get(r.path.len()) == Some(&b'/'))
            })
            .max_by_key(|r| r.path.len())
    }

    pub fn error_page(&self, status: u16) -> Option<&PathBuf> {
        self.error_pages.get(&status)
    }

    pub fn identity(&self) -> String {
        let ports: Vec<String> = self.ports.iter().map(|p| p.to_string()).collect();
        format!("{}:{}", self.host, ports.join(","))
    }
}

/// The complete parsed configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub servers: Vec<ServerConfig>,
}

impl Config {
    pub fn new(servers: Vec<ServerConfig>) -> Self {
        Self { servers }
    }
}
