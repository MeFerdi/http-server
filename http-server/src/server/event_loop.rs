use std::collections::{HashMap, HashSet};
use std::io;
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::handlers;
use crate::http::{parse_request, HttpParseError};
use crate::session::SessionStore;

use super::connection::{Connection, ConnectionState};
use super::epoll::Epoll;
use super::listener::{build_listeners, Listener};

const MAX_EVENTS: usize = 1024;
const EPOLL_WAIT_TIMEOUT_MS: i32 = 1_000;
pub const TIMEOUT_DURATION: Duration = Duration::from_secs(30);

pub fn run(config: &Config) -> io::Result<()> {
    let listeners = build_listeners(config)?;
    let listener_fds: HashSet<RawFd> = listeners.iter().map(|l| l.fd).collect();
    let listener_by_fd: HashMap<RawFd, &Listener> = listeners.iter().map(|l| (l.fd, l)).collect();

    let epoll = Epoll::new()?;
    for listener in &listeners {
        epoll.add(listener.fd, (libc::EPOLLIN | libc::EPOLLET) as u32)?;
        println!("Listening on {}", listener.identity());
    }

    let mut connections: HashMap<RawFd, Connection> = HashMap::new();
    let mut sessions = SessionStore::new();

    loop {
        let events = epoll.wait(MAX_EVENTS, EPOLL_WAIT_TIMEOUT_MS)?;

        for event in events {
            let fd = event.u64 as RawFd;
            let flags = event.events as i32;

            if listener_fds.contains(&fd) {
                if let Some(listener) = listener_by_fd.get(&fd) {
                    accept_all(listener, &epoll, &mut connections)?;
                }
                continue;
            }

            if flags & (libc::EPOLLHUP | libc::EPOLLERR | libc::EPOLLRDHUP) != 0 {
                close_connection(fd, &epoll, &mut connections);
                continue;
            }

            if flags & libc::EPOLLIN != 0 {
                handle_read(fd, &epoll, config, &mut sessions, &mut connections)?;
            }

            if flags & libc::EPOLLOUT != 0 {
                handle_write(fd, &epoll, &mut connections)?;
            }
        }

        close_timed_out_connections(&epoll, &mut connections);
    }
}

fn accept_all(listener: &Listener, epoll: &Epoll, connections: &mut HashMap<RawFd, Connection>) -> io::Result<()> {
    loop {
        let client_fd = unsafe {
            libc::accept4(
                listener.fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                libc::SOCK_NONBLOCK,
            )
        };

        if client_fd < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock {
                break;
            }
            return Err(err);
        }

        epoll.add(
            client_fd,
            (libc::EPOLLIN | libc::EPOLLRDHUP | libc::EPOLLHUP | libc::EPOLLERR) as u32,
        )?;
        connections.insert(
            client_fd,
            Connection::new(listener.host.clone(), listener.port),
        );
    }

    Ok(())
}

fn handle_read(
    fd: RawFd,
    epoll: &Epoll,
    config: &Config,
    sessions: &mut SessionStore,
    connections: &mut HashMap<RawFd, Connection>,
) -> io::Result<()> {
    let mut should_close = false;
    let mut switch_to_write = false;
    let mut parse_error: Option<HttpParseError> = None;

    if let Some(conn) = connections.get_mut(&fd) {
        loop {
            let mut buf = [0_u8; 4096];
            let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };

            if n == 0 {
                should_close = true;
                break;
            }

            if n < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::WouldBlock {
                    break;
                }
                should_close = true;
                break;
            }

            let n = n as usize;
            let received = &buf[..n];

            let (local_host, local_port) = (conn.local_host.clone(), conn.local_port);
            if let ConnectionState::Reading { buffer, timeout } = &mut conn.state {
                buffer.extend_from_slice(received);
                *timeout = Instant::now();

                match parse_request(buffer, 0) {
                    Ok(request) => {
                        let effective_limit = config
                            .servers
                            .iter()
                            .filter(|s| s.host == local_host && s.ports.contains(&local_port))
                            .filter(|s| s.client_max_body_size > 0)
                            .map(|s| s.client_max_body_size)
                            .min();

                        if let Some(limit) = effective_limit.filter(|limit| request.body.len() > *limit) {
                            parse_error = Some(HttpParseError::BodyTooLarge {
                                limit,
                                actual: request.body.len(),
                            });
                        } else {
                            conn.state = ConnectionState::Processing { request };
                        }
                        switch_to_write = true;
                        break;
                    }
                    Err(HttpParseError::Incomplete) => {
                        // keep reading until a full request is available
                    }
                    Err(err) => {
                        parse_error = Some(err);
                        switch_to_write = true;
                        break;
                    }
                }
            }
        }
    }

    if should_close {
        close_connection(fd, epoll, connections);
        return Ok(());
    }

    if switch_to_write {
        if let Some(conn) = connections.get_mut(&fd) {
            let response = if let Some(err) = &parse_error {
                parse_error_response(err)
            } else if let ConnectionState::Processing { request } = &conn.state {
                handlers::handle_request(config, &conn.local_host, conn.local_port, sessions, request)
                    .to_bytes()
            } else {
                parse_error_response(&HttpParseError::InvalidRequestLine)
            };

            conn.state = ConnectionState::Writing {
                response,
                offset: 0,
                timeout: Instant::now(),
            };
            epoll.modify(
                fd,
                (libc::EPOLLOUT | libc::EPOLLRDHUP | libc::EPOLLHUP | libc::EPOLLERR) as u32,
            )?;
        }
    }

    Ok(())
}

fn handle_write(fd: RawFd, epoll: &Epoll, connections: &mut HashMap<RawFd, Connection>) -> io::Result<()> {
    let mut should_close = false;

    if let Some(conn) = connections.get_mut(&fd) {
        if let ConnectionState::Writing {
            response,
            offset,
            timeout,
        } = &mut conn.state
        {
            while *offset < response.len() {
                let remaining = &response[*offset..];
                let n = unsafe {
                    libc::write(
                        fd,
                        remaining.as_ptr() as *const libc::c_void,
                        remaining.len(),
                    )
                };

                if n < 0 {
                    let err = io::Error::last_os_error();
                    if err.kind() == io::ErrorKind::WouldBlock {
                        break;
                    }
                    should_close = true;
                    break;
                }

                *offset += n as usize;
                *timeout = Instant::now();
            }

            if *offset >= response.len() {
                should_close = true;
            }
        }
    }

    if should_close {
        close_connection(fd, epoll, connections);
    }

    Ok(())
}

fn close_timed_out_connections(epoll: &Epoll, connections: &mut HashMap<RawFd, Connection>) {
    let timed_out: Vec<RawFd> = connections
        .iter()
        .filter_map(|(fd, conn)| conn.is_timed_out(TIMEOUT_DURATION).then_some(*fd))
        .collect();

    for fd in timed_out {
        close_connection(fd, epoll, connections);
    }
}

fn close_connection(fd: RawFd, epoll: &Epoll, connections: &mut HashMap<RawFd, Connection>) {
    if let Some(conn) = connections.get_mut(&fd) {
        conn.state = ConnectionState::Closing;
    }

    let _ = epoll.delete(fd);
    connections.remove(&fd);
    unsafe {
        libc::close(fd);
    }
}

fn scaffold_bad_request_response(err: &HttpParseError) -> Vec<u8> {
    let body = format!("bad request: {:?}\n", err);
    format!(
        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes()
}

fn parse_error_response(err: &HttpParseError) -> Vec<u8> {
    match err {
        HttpParseError::BodyTooLarge { .. } => {
            let body = b"Payload Too Large\n";
            format!(
                "HTTP/1.1 413 Payload Too Large\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes()
            .into_iter()
            .chain(body.iter().copied())
            .collect()
        }
        _ => scaffold_bad_request_response(err),
    }
}
