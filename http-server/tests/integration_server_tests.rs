#![cfg(target_os = "linux")]

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct ServerHandle {
    child: Child,
    temp_dir: PathBuf,
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

fn unique_temp_dir() -> PathBuf {
    let mut path = std::env::temp_dir();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time drift")
        .as_nanos();
    path.push(format!("localhost-it-{}-{}", std::process::id(), now));
    path
}

fn reserve_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve port");
    listener
        .local_addr()
        .expect("listener local_addr")
        .port()
}

fn write_config(path: &Path, port: u16, root: &Path) {
    let content = format!(
        "server {{\n    host 127.0.0.1\n    port {}\n    body_limit 1048576\n\n    location / {{\n        methods GET POST DELETE\n        root {}\n        default_file index.html\n        directory_listing off\n    }}\n}}\n",
        port,
        root.display()
    );

    fs::write(path, content).expect("write config");
}

fn wait_until_ready(port: u16) {
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("server not ready on port {}", port);
}

fn start_server() -> (ServerHandle, u16) {
    let temp_dir = unique_temp_dir();
    let www_dir = temp_dir.join("www");
    fs::create_dir_all(&www_dir).expect("create www dir");
    fs::write(www_dir.join("index.html"), "ok").expect("write index file");

    let port = reserve_port();
    let config_path = temp_dir.join("config.conf");
    write_config(&config_path, port, &www_dir);

    let child = Command::new(env!("CARGO_BIN_EXE_localhost"))
        .arg(&config_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn localhost binary");

    wait_until_ready(port);

    (ServerHandle { child, temp_dir }, port)
}

fn start_server_with_config(temp_dir: &Path, config_content: String) -> (ServerHandle, u16) {
    fs::create_dir_all(temp_dir).expect("create temp dir");
    let port = reserve_port();
    let config_path = temp_dir.join("config.conf");
    fs::write(&config_path, config_content.replace("__PORT__", &port.to_string()))
        .expect("write custom config");

    let child = Command::new(env!("CARGO_BIN_EXE_localhost"))
        .arg(&config_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn localhost binary");

    wait_until_ready(port);

    (
        ServerHandle {
            child,
            temp_dir: temp_dir.to_path_buf(),
        },
        port,
    )
}

fn find_python3() -> Option<String> {
    for candidate in ["/usr/bin/python3", "/bin/python3"] {
        if Path::new(candidate).exists() {
            return Some(candidate.to_string());
        }
    }
    None
}

fn send_raw_request(port: u16, request: &[u8]) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to server");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set read timeout");
    stream.write_all(request).expect("write request");
    stream.flush().expect("flush request");

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("read response body");
    response
}

#[test]
fn event_loop_accepts_connection_and_returns_scaffold_response() {
    let (_server, port) = start_server();

    let request = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_raw_request(port, request);

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("Content-Type: text/html; charset=utf-8\r\n"));
    assert!(response.contains("Connection: close\r\n"));
    assert!(response.ends_with("ok"));
}

#[test]
fn event_loop_handles_multiple_requests() {
    let (_server, port) = start_server();

    for _ in 0..3 {
        let request = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let response = send_raw_request(port, request);

        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with("ok"));
    }
}

#[test]
fn cgi_execution_returns_script_output() {
    let Some(python3) = find_python3() else {
        return;
    };

    let temp_dir = unique_temp_dir();
    let www_dir = temp_dir.join("www");
    let cgi_dir = temp_dir.join("cgi-bin");
    fs::create_dir_all(&www_dir).expect("create www dir");
    fs::create_dir_all(&cgi_dir).expect("create cgi dir");
    fs::write(www_dir.join("index.html"), "ok").expect("write index file");
    fs::write(
        cgi_dir.join("echo.py"),
        r#"import os
print('Content-Type: text/plain')
print()
print(f"method={os.environ.get('REQUEST_METHOD','')}")
print(f"query={os.environ.get('QUERY_STRING','')}")
"#,
    )
    .expect("write cgi script");

    let config = format!(
        "server {{\n    host 127.0.0.1\n    port __PORT__\n    body_limit 1048576\n\n    location / {{\n        methods GET\n        root {}\n        default_file index.html\n        directory_listing off\n    }}\n\n    location /cgi {{\n        methods GET POST\n        root {}\n        cgi .py {}\n    }}\n}}\n",
        www_dir.display(),
        cgi_dir.display(),
        python3
    );

    let (_server, port) = start_server_with_config(&temp_dir, config);
    let response = send_raw_request(
        port,
        b"GET /cgi/echo.py?name=alice HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("method=GET"));
    assert!(response.contains("query=name=alice"));
}

#[test]
fn multipart_upload_saves_file_to_upload_dir() {
    let temp_dir = unique_temp_dir();
    let www_dir = temp_dir.join("www");
    let upload_dir = temp_dir.join("uploads");
    fs::create_dir_all(&www_dir).expect("create www dir");
    fs::create_dir_all(&upload_dir).expect("create upload dir");
    fs::write(www_dir.join("index.html"), "ok").expect("write index file");

    let config = format!(
        "server {{\n    host 127.0.0.1\n    port __PORT__\n    body_limit 1048576\n\n    location / {{\n        methods GET\n        root {}\n        default_file index.html\n        directory_listing off\n    }}\n\n    location /uploads {{\n        methods POST\n        root {}\n    }}\n}}\n",
        www_dir.display(),
        upload_dir.display()
    );

    let (_server, port) = start_server_with_config(&temp_dir, config);

    let boundary = "----localhostBoundary123";
    let multipart_body = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"hello.txt\"\r\nContent-Type: text/plain\r\n\r\nhello upload\r\n--{b}--\r\n",
        b = boundary
    );

    let request = format!(
        "POST /uploads HTTP/1.1\r\nHost: localhost\r\nContent-Type: multipart/form-data; boundary={}\r\nContent-Length: {}\r\n\r\n{}",
        boundary,
        multipart_body.len(),
        multipart_body
    );

    let response = send_raw_request(port, request.as_bytes());
    assert!(response.starts_with("HTTP/1.1 201 Created\r\n"));
    assert!(response.contains("saved_files=1"));

    let entries: Vec<_> = fs::read_dir(&upload_dir)
        .expect("read upload dir")
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1);

    let uploaded = fs::read(entries[0].path()).expect("read uploaded file");
    assert_eq!(uploaded, b"hello upload");
}

#[test]
fn session_cookie_is_issued_and_reused() {
    let (_server, port) = start_server();

    let first = send_raw_request(port, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert!(first.starts_with("HTTP/1.1 200 OK\r\n"));
    let set_cookie_line = first
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("set-cookie:"))
        .expect("set-cookie header on first request")
        .to_string();

    let cookie_pair = set_cookie_line
        .split(':')
        .nth(1)
        .map(str::trim)
        .and_then(|v| v.split(';').next())
        .expect("cookie pair")
        .to_string();

    let second_req = format!(
        "GET / HTTP/1.1\r\nHost: localhost\r\nCookie: {}\r\n\r\n",
        cookie_pair
    );
    let second = send_raw_request(port, second_req.as_bytes());

    assert!(second.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(!second.to_ascii_lowercase().contains("\r\nset-cookie:"));
}

#[test]
fn malformed_request_returns_400() {
    let (_server, port) = start_server();

    let response = send_raw_request(port, b"GET / HTTP/1.1\r\nHost localhost\r\n\r\n");
    assert!(response.starts_with("HTTP/1.1 400 Bad Request\r\n"));
}

#[test]
fn body_limit_violation_returns_413() {
    let temp_dir = unique_temp_dir();
    let www_dir = temp_dir.join("www");
    fs::create_dir_all(&www_dir).expect("create www dir");
    fs::write(www_dir.join("index.html"), "ok").expect("write index file");

    let config = format!(
        "server {{\n    host 127.0.0.1\n    port __PORT__\n    body_limit 4\n\n    location / {{\n        methods GET POST\n        root {}\n        default_file index.html\n        directory_listing off\n    }}\n}}\n",
        www_dir.display()
    );

    let (_server, port) = start_server_with_config(&temp_dir, config);

    let response = send_raw_request(
        port,
        b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nhello",
    );

    assert!(response.starts_with("HTTP/1.1 413 Payload Too Large\r\n"));
}
