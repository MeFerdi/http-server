//! Config file parser.
//!
//! Syntax:
//!
//! ```text
//! server {
//!     host        0.0.0.0
//!     port        8080 8081
//!     server_name example.com
//!     error_page  404 ./www/errors/404.html
//!     body_limit  10485760   # bytes; 0 = unlimited
//!
//!     location / {
//!         methods         GET POST DELETE
//!         root            ./www
//!         default_file    index.html
//!         directory_listing  on
//!     }
//!
//!     location /cgi-bin {
//!         methods  GET POST
//!         root     ./cgi-bin
//!         cgi      .py /usr/bin/python3
//!     }
//!
//!     location /uploads {
//!         methods  POST DELETE
//!         root     ./uploads
//!     }
//!
//!     location /old {
//!         redirect  /new
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

use super::error::ConfigError;
use super::types::{Config, HttpMethod, RouteConfig, ServerConfig};


/// A flat list of tokens with their source line numbers.
#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    OpenBrace,
    CloseBrace,
    Newline,
}

struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Lexer { input, pos: 0, line: 1 }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
        }
        Some(ch)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip horizontal whitespace. Newlines are significant for directive boundaries.
            while matches!(self.peek(), Some(' ') | Some('\t') | Some('\r')) {
                self.advance();
            }
            // Skip comments (# to end of line)
            if self.peek() == Some('#') {
                while !matches!(self.peek(), None | Some('\n')) {
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    /// Tokenize the entire input.  Returns (Token, line_number) pairs.
    fn tokenize(&mut self) -> Result<Vec<(Token, usize)>, ConfigError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            match self.peek() {
                None => break,
                Some('\n') => {
                    self.advance();
                    tokens.push((Token::Newline, self.line.saturating_sub(1)));
                }
                Some('{') => {
                    self.advance();
                    tokens.push((Token::OpenBrace, self.line));
                }
                Some('}') => {
                    self.advance();
                    tokens.push((Token::CloseBrace, self.line));
                }
                Some(_) => {
                    let start = self.pos;
                    let start_line = self.line;
                    while !matches!(
                        self.peek(),
                        None | Some(' ') | Some('\t') | Some('\n') | Some('\r')
                            | Some('{') | Some('}') | Some('#')
                    ) {
                        self.advance();
                    }
                    let word = self.input[start..self.pos].to_string();
                    tokens.push((Token::Word(word), start_line));
                }
            }
        }
        Ok(tokens)
    }
}

struct Parser {
    tokens: Vec<(Token, usize)>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<(Token, usize)>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn current_line(&self) -> usize {
        self.tokens.get(self.pos).map(|t| t.1).unwrap_or(0)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|t| &t.0)
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Some(Token::Newline)) {
            self.pos += 1;
        }
    }

    fn expect_word(&mut self) -> Result<String, ConfigError> {
        self.skip_newlines();
        match self.tokens.get(self.pos) {
            Some((Token::Word(w), _)) => {
                let word = w.clone();
                self.pos += 1;
                Ok(word)
            }
            _ => Err(ConfigError::ParseError {
                line: self.current_line(),
                message: "expected a word token".to_string(),
            }),
        }
    }

    fn expect_open_brace(&mut self) -> Result<(), ConfigError> {
        self.skip_newlines();
        match self.tokens.get(self.pos) {
            Some((Token::OpenBrace, _)) => {
                self.pos += 1;
                Ok(())
            }
            _ => Err(ConfigError::ParseError {
                line: self.current_line(),
                message: "expected '{'".to_string(),
            }),
        }
    }

    fn expect_close_brace(&mut self) -> Result<(), ConfigError> {
        self.skip_newlines();
        match self.tokens.get(self.pos) {
            Some((Token::CloseBrace, _)) => {
                self.pos += 1;
                Ok(())
            }
            _ => Err(ConfigError::ParseError {
                line: self.current_line(),
                message: "expected '}'".to_string(),
            }),
        }
    }

    /// Consume tokens until the next keyword or close-brace, returning them as
    /// a Vec of strings.  This collects multi-value directives like `port 80 443`.
    fn collect_values(&mut self) -> Vec<String> {
        let mut values = Vec::new();
        while let Some(Token::Word(w)) = self.peek() {
            values.push(w.clone());
            self.pos += 1;
        }
        values
    }

    fn expect_value(&mut self, field: &'static str, line: usize) -> Result<String, ConfigError> {
        match self.tokens.get(self.pos) {
            Some((Token::Word(w), _)) => {
                self.pos += 1;
                Ok(w.clone())
            }
            _ => Err(ConfigError::ParseError {
                line,
                message: format!("'{}' requires a value", field),
            }),
        }
    }


    fn parse_config(&mut self) -> Result<Config, ConfigError> {
        let mut servers = Vec::new();
        while self.pos < self.tokens.len() {
            self.skip_newlines();
            if self.pos >= self.tokens.len() {
                break;
            }
            let keyword = self.expect_word()?;
            if keyword != "server" {
                return Err(ConfigError::ParseError {
                    line: self.current_line(),
                    message: format!("unexpected token '{}', expected 'server'", keyword),
                });
            }
            servers.push(self.parse_server()?);
        }
        Ok(Config::new(servers))
    }


    fn parse_server(&mut self) -> Result<ServerConfig, ConfigError> {
        self.expect_open_brace()?;

        let mut host: Option<String> = None;
        let mut ports: Vec<u16> = Vec::new();
        let mut server_name: Option<String> = None;
        let mut error_pages: HashMap<u16, PathBuf> = HashMap::new();
        let mut body_limit: usize = 0;
        let mut routes: Vec<RouteConfig> = Vec::new();

        loop {
            match self.peek() {
                Some(Token::Newline) => {
                    self.pos += 1;
                }
                Some(Token::CloseBrace) => {
                    self.expect_close_brace()?;
                    break;
                }
                Some(Token::Word(_)) => {
                    let line = self.current_line();
                    let key = self.expect_word()?;
                    match key.as_str() {
                        "host" => {
                            host = Some(self.expect_value("host", line)?);
                        }
                        "port" => {
                            let values = self.collect_values();
                            if values.is_empty() {
                                return Err(ConfigError::ParseError {
                                    line,
                                    message: "port directive requires at least one value".to_string(),
                                });
                            }
                            for v in &values {
                                let p = v.parse::<u16>().map_err(|_| {
                                    ConfigError::InvalidValue {
                                        field: "port",
                                        value: v.clone(),
                                        reason: "must be an integer between 1 and 65535",
                                    }
                                })?;
                                if p == 0 {
                                    return Err(ConfigError::InvalidValue {
                                        field: "port",
                                        value: v.clone(),
                                        reason: "port 0 is not allowed",
                                    });
                                }
                                ports.push(p);
                            }
                        }
                        "server_name" => {
                            server_name = Some(self.expect_value("server_name", line)?);
                        }
                        "error_page" => {
                            let code_raw = self.expect_value("error_page", line)?;
                            let path_raw = self.expect_value("error_page", line)?;
                            let code = code_raw.parse::<u16>().map_err(|_| {
                                ConfigError::InvalidValue {
                                    field: "error_page",
                                    value: code_raw.clone(),
                                    reason: "must be a valid HTTP status code",
                                }
                            })?;
                            error_pages.insert(code, PathBuf::from(path_raw));
                        }
                        "body_limit" => {
                            let raw = self.expect_value("body_limit", line)?;
                            body_limit = raw.parse::<usize>().map_err(|_| {
                                ConfigError::InvalidBodySize { value: raw.clone() }
                            })?;
                        }
                        "location" => {
                            let path = self.expect_word().map_err(|_| ConfigError::ParseError {
                                line,
                                message: "location directive requires a path argument".to_string(),
                            })?;
                            routes.push(self.parse_location(path, line)?);
                        }
                        other => {
                            return Err(ConfigError::ParseError {
                                line,
                                message: format!("unknown server directive '{}'", other),
                            });
                        }
                    }
                }
                _ => {
                    return Err(ConfigError::ParseError {
                        line: self.current_line(),
                        message: "unexpected token in server block".to_string(),
                    });
                }
            }
        }

        let host = host.ok_or(ConfigError::MissingField { block: "server", field: "host" })?;
        if ports.is_empty() {
            return Err(ConfigError::MissingField { block: "server", field: "port" });
        }

        Ok(ServerConfig {
            host,
            ports,
            server_name,
            error_pages,
            client_max_body_size: body_limit,
            routes,
        })
    }


    fn parse_location(&mut self, path: String, _outer_line: usize) -> Result<RouteConfig, ConfigError> {
        self.expect_open_brace()?;

        let mut methods: Vec<HttpMethod> = Vec::new();
        let mut redirect: Option<String> = None;
        let mut root: PathBuf = PathBuf::from("./www");
        let mut default_file: Option<String> = None;
        let mut cgi: Option<(String, PathBuf)> = None;
        let mut directory_listing = false;

        loop {
            match self.peek() {
                Some(Token::Newline) => {
                    self.pos += 1;
                }
                Some(Token::CloseBrace) => {
                    self.expect_close_brace()?;
                    break;
                }
                Some(Token::Word(_)) => {
                    let line = self.current_line();
                    let key = self.expect_word()?;
                    match key.as_str() {
                        "methods" => {
                            let values = self.collect_values();
                            if values.is_empty() {
                                return Err(ConfigError::ParseError {
                                    line,
                                    message: "methods directive requires at least one method".to_string(),
                                });
                            }
                            for v in &values {
                                let m = HttpMethod::from_str(v).ok_or_else(|| {
                                    ConfigError::InvalidValue {
                                        field: "methods",
                                        value: v.clone(),
                                        reason: "must be GET, POST, or DELETE",
                                    }
                                })?;
                                methods.push(m);
                            }
                        }
                        "root" => {
                            let r = self.expect_value("root", line)?;
                            root = PathBuf::from(r);
                        }
                        "default_file" => {
                            default_file = Some(self.expect_value("default_file", line)?);
                        }
                        "redirect" => {
                            redirect = Some(self.expect_value("redirect", line)?);
                        }
                        "cgi" => {
                            let ext = self.expect_value("cgi", line)?;
                            let interp = PathBuf::from(self.expect_value("cgi", line)?);
                            cgi = Some((ext, interp));
                        }
                        "directory_listing" => {
                            let v = self.expect_value("directory_listing", line)?;
                            directory_listing = match v.as_str() {
                                "on" | "true" | "yes" | "1" => true,
                                "off" | "false" | "no" | "0" => false,
                                _ => {
                                    return Err(ConfigError::InvalidValue {
                                        field: "directory_listing",
                                        value: v,
                                        reason: "must be on or off",
                                    })
                                }
                            };
                        }
                        other => {
                            return Err(ConfigError::ParseError {
                                line,
                                message: format!("unknown location directive '{}'", other),
                            });
                        }
                    }
                }
                _ => {
                    return Err(ConfigError::ParseError {
                        line: self.current_line(),
                        message: "unexpected token in location block".to_string(),
                    });
                }
            }
        }

        Ok(RouteConfig {
            path,
            methods,
            redirect,
            root,
            default_file,
            cgi,
            directory_listing,
        })
    }
}

/// Validate a fully parsed Config, checking for semantic errors.
pub fn validate(config: &Config) -> Result<(), Vec<ConfigError>> {
    let mut errors: Vec<ConfigError> = Vec::new();

    // Track (host, port, server_name=None) combinations to detect conflicts.
    // Two unnamed servers on the same host:port conflict because there can only
    // be one default.
    let mut default_servers: HashMap<(String, u16), usize> = HashMap::new();

    for (idx, server) in config.servers.iter().enumerate() {
        // 1. No routes
        if server.routes.is_empty() {
            errors.push(ConfigError::NoRoutes {
                server: server.identity(),
            });
        }

        // 2. Conflicting default server on same host:port
        for &port in &server.ports {
            if server.server_name.is_none() {
                let key = (server.host.clone(), port);
                if let Some(prev_idx) = default_servers.get(&key) {
                    if *prev_idx != idx {
                        errors.push(ConfigError::ConflictingPort {
                            host: server.host.clone(),
                            port,
                        });
                    }
                } else {
                    default_servers.insert(key, idx);
                }
            }
        }

        // 3. Validate error page paths exist
        for (code, path) in &server.error_pages {
            if !path.exists() {
                errors.push(ConfigError::PathNotFound {
                    field: "error_page",
                    path: format!("[{}] {}", code, path.display()),
                });
            }
        }

        // 4. Validate route roots
        for route in &server.routes {
            // Skip root validation for pure-redirect routes
            if route.redirect.is_some() {
                continue;
            }
            if !route.root.exists() {
                errors.push(ConfigError::PathNotFound {
                    field: "root",
                    path: route.root.display().to_string(),
                });
            } else if !route.root.is_dir() {
                errors.push(ConfigError::RootNotDirectory {
                    path: route.root.display().to_string(),
                });
            }

            // 5. Validate CGI interpreter path
            if let Some((_, interp)) = &route.cgi {
                if !interp.exists() {
                    errors.push(ConfigError::PathNotFound {
                        field: "cgi interpreter",
                        path: interp.display().to_string(),
                    });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}


/// Parse a config file from a string.  Returns the Config without filesystem
pub fn parse(input: &str) -> Result<Config, ConfigError> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse_config()
}

/// Parse a config file from the filesystem and run full validation.
pub fn load(path: &str) -> Result<Config, Vec<ConfigError>> {
    let input = std::fs::read_to_string(path).map_err(|e| {
        vec![ConfigError::IoError(format!("{}: {}", path, e))]
    })?;

    let config = parse(&input).map_err(|e| vec![e])?;
    validate(&config)?;
    Ok(config)
}
