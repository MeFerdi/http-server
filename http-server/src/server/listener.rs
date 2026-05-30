use std::collections::HashSet;
use std::io;
use std::mem;
use std::net::Ipv4Addr;
use std::os::fd::RawFd;

use crate::config::Config;

pub struct Listener {
    pub fd: RawFd,
    pub host: String,
    pub port: u16,
}

impl Listener {
    pub fn identity(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

pub fn build_listeners(config: &Config) -> io::Result<Vec<Listener>> {
    let mut pairs = HashSet::new();
    for server in &config.servers {
        for &port in &server.ports {
            pairs.insert((server.host.clone(), port));
        }
    }

    let mut listeners = Vec::new();
    for (host, port) in pairs {
        let fd = create_listener(&host, port)?;
        listeners.push(Listener { fd, host, port });
    }

    listeners.sort_by(|a, b| a.identity().cmp(&b.identity()));
    Ok(listeners)
}

fn create_listener(host: &str, port: u16) -> io::Result<RawFd> {
    let ip: Ipv4Addr = host
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, format!("invalid IPv4 host: {}", host)))?;

    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM | libc::SOCK_NONBLOCK, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let yes: libc::c_int = 1;
    let set_reuse = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &yes as *const _ as *const libc::c_void,
            mem::size_of_val(&yes) as libc::socklen_t,
        )
    };
    if set_reuse < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(err);
    }

    let addr = libc::sockaddr_in {
        sin_family: libc::AF_INET as u16,
        sin_port: port.to_be(),
        sin_addr: libc::in_addr {
            s_addr: u32::from(ip).to_be(),
        },
        sin_zero: [0; 8],
    };

    let bind_rc = unsafe {
        libc::bind(
            fd,
            &addr as *const _ as *const libc::sockaddr,
            mem::size_of_val(&addr) as libc::socklen_t,
        )
    };
    if bind_rc < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(err);
    }

    let listen_rc = unsafe { libc::listen(fd, 1024) };
    if listen_rc < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(err);
    }

    Ok(fd)
}
