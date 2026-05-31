# localhost

A single-threaded, non-blocking HTTP/1.1 server written in Rust, built from the ground up using `epoll` and raw POSIX syscalls. No async runtimes. No framework, only the protocol and the kernel.

---

## Features

- **Single process, single thread** — multiplexes all clients via `epoll`
- **Non-blocking I/O** — every read and write goes through the event loop
- **HTTP/1.1** — keep-alive, chunked transfer encoding, correct status codes
- **Virtual hosting** — multiple server blocks, port sharing, `Host`-header routing
- **Static file serving** — with `Content-Type` detection and directory listing
- **File uploads** — `multipart/form-data` POST body handling
- **CGI support** — fork/exec scripts by file extension (`.py`, `.php`, ...)
- **Request timeouts** — no connection lives forever
- **Custom error pages** — per-server configuration for 400, 403, 404, 405, 413, 500
- **Cookies & sessions** — `Set-Cookie` / `Cookie` header handling with server-side session store
- **Stress-tested** — targets ≥ 99.5% availability under `siege -b`

---

## Requirements

| Tool       | Version                 |
| ---------- | ----------------------- |
| Rust       | ≥ 1.75 (2021 edition)   |
| Linux      | kernel ≥ 2.6.17 (epoll) |
| libc crate | 0.2                     |

> **No** `tokio`, `async-std`, `nix`, or any other crate that wraps server primitives.

---

## Quick start

```bash
# Build
cargo build --release

# Run with the example config
./target/release/localhost config.conf
```

Then open `http://localhost:8080` in your browser.

---

## Configuration

The config file uses a clean block syntax inspired by NGINX. Comments start with `#`.

```nginx
server {
    host        0.0.0.0
    port        8080
    server_name mysite.local
    body_limit  10485760       # 10 MB; 0 = unlimited

    error_page  404  ./www/errors/404.html
    error_page  500  ./www/errors/500.html

    location / {
        methods           GET POST DELETE
        root              ./www
        default_file      index.html
        directory_listing off
    }

    location /files {
        methods           GET
        root              ./www/files
        directory_listing on
    }

    location /cgi-bin {
        methods  GET POST
        root     ./cgi-bin
        cgi      .py /usr/bin/python3
    }

    location /uploads {
        methods  POST DELETE
        root     ./uploads
    }

    location /old-path {
        redirect  /new-path
    }
}
```

Multiple `server` blocks are allowed. When two blocks share a `host:port`, the request is routed by the `Host` header matched against `server_name`. The first block without a `server_name` is the default for that address.

### Directives

**Server block**

| Directive     | Required | Description                                  |
| ------------- | -------- | -------------------------------------------- |
| `host`        | yes      | IP address to bind, e.g. `0.0.0.0`           |
| `port`        | yes      | One or more ports, space-separated           |
| `server_name` | no       | Virtual host name for `Host` header matching |
| `body_limit`  | no       | Max request body in bytes (`0` = unlimited)  |
| `error_page`  | no       | `<code> <path>` — repeatable                 |

**Location block**

| Directive           | Required | Description                                                          |
| ------------------- | -------- | -------------------------------------------------------------------- |
| `methods`           | no       | Allowed methods: `GET`, `POST`, `DELETE` (empty = all)               |
| `root`              | no\*     | Filesystem directory to serve from                                   |
| `default_file`      | no       | File to serve when URL resolves to a directory                       |
| `directory_listing` | no       | `on` or `off` (default: `off`)                                       |
| `cgi`               | no       | `<.ext> <interpreter>` — runs matching files through the interpreter |
| `redirect`          | no       | Respond with `301` to the given URL (`root` not needed)              |

---

## Project structure

```
localhost/
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Public crate root (for tests)
│   ├── config/
│   │   ├── mod.rs
│   │   ├── types.rs         # ServerConfig, RouteConfig, HttpMethod
│   │   ├── parser.rs        # Lexer + recursive-descent parser + validator
│   │   └── error.rs         # ConfigError enum
│   ├── server/              # [Phase 2]
│   │   ├── mod.rs
│   │   ├── epoll.rs         # unsafe libc epoll wrappers
│   │   ├── listener.rs      # bind/listen sockets
│   │   ├── connection.rs    # ConnectionState machine
│   │   └── event_loop.rs    # main epoll_wait loop
│   ├── http/                # [Phase 3]
│   │   ├── mod.rs
│   │   ├── request.rs       # HttpRequest + incremental parser
│   │   ├── response.rs      # HttpResponse + builder
│   │   ├── method.rs
│   │   └── status.rs        # StatusCode enum + reason phrases
│   ├── handlers/            # [Phase 4]
│   │   ├── mod.rs
│   │   ├── static_file.rs
│   │   ├── directory.rs
│   │   ├── cgi.rs
│   │   ├── upload.rs
│   │   └── error.rs
│   ├── router/              # [Phase 4]
│   │   └── matcher.rs
│   └── session/             # [Phase 5]
│       └── store.rs
├── tests/
│   ├── config_tests.rs      # [Phase 1] — 40+ unit tests
│   ├── http_parser_tests.rs # [Phase 3]
│   ├── router_tests.rs      # [Phase 4]
│   └── integration/
│       └── server_tests.rs  # [Phase 6]
├── www/
│   ├── index.html
│   ├── errors/              # Default error pages (400 403 404 405 413 500)
│   └── files/
├── cgi-bin/
├── uploads/
├── config.conf              # Example configuration
└── Cargo.toml
```

---

## Implementation phases

| Phase | Status   | Description                                        |
| ----- | -------- | -------------------------------------------------- |
| 1     | Complete | Config parser, validator, types, unit tests        |
| 2     | Complete | Non-blocking TCP sockets + `epoll` event loop      |
| 3     | Complete | HTTP/1.1 request parser (headers, body, chunked)   |
| 4     | Complete | Router + static file / CGI / upload handlers       |
| 5     | Complete | Error pages, cookies, sessions                     |
| 6     | Complete | Hardening, integration tests, `siege` stress tests |

---

## Running tests

```bash
# All unit tests
cargo test

# A specific test module
cargo test config_tests

# With output (useful for debugging)
cargo test -- --nocapture
```

---

## Stress testing

```bash
# Install siege (Ubuntu/Debian)
sudo apt install siege

# Run a stress test — target ≥ 99.5% availability
siege -b http://127.0.0.1:8080

# Concurrent users, time-limited
siege -c 100 -t 30S http://127.0.0.1:8080
```

> **Warning:** Only run `siege` against servers you own. Using it against third-party servers without permission is illegal.

---

## Architecture notes

### Why epoll + one thread?

`epoll` lets a single thread manage thousands of concurrent connections without blocking on any one of them. The kernel maintains a set of "interesting" file descriptors and notifies your process only when I/O is actually possible — no wasted CPU spinning, no thread-per-connection overhead.

### Connection lifecycle

```
accept() → register fd with epoll
    → EPOLLIN fires  → read bytes into buffer → parse HTTP request
    → dispatch to handler → build response
    → EPOLLOUT fires → write bytes from response buffer → close or keep-alive
```

Every state transition is driven by the event loop. If a connection is silent for `TIMEOUT_DURATION` seconds, the timeout scanner closes it.

### epoll is called once per client

The spec requires `epoll_ctl` / `epoll_wait` to be called once per client/server communication cycle, not once globally. Each new connection registers itself; each completed write deregisters. The main `epoll_wait` call in the event loop dispatches to whichever fds are ready.

---

## License

MIT
