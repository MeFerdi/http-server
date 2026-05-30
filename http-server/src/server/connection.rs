use std::time::{Duration, Instant};

use crate::http::HttpRequest;

#[derive(Debug)]
pub enum ConnectionState {
    Reading { buffer: Vec<u8>, timeout: Instant },
    Processing { request: HttpRequest },
    Writing { response: Vec<u8>, offset: usize, timeout: Instant },
    Closing,
}

#[derive(Debug)]
pub struct Connection {
    pub local_host: String,
    pub local_port: u16,
    pub state: ConnectionState,
}

impl Connection {
    pub fn new(local_host: String, local_port: u16) -> Self {
        Self {
            local_host,
            local_port,
            state: ConnectionState::Reading {
                buffer: Vec::with_capacity(4096),
                timeout: Instant::now(),
            },
        }
    }

    pub fn is_timed_out(&self, timeout_duration: Duration) -> bool {
        match &self.state {
            ConnectionState::Reading { timeout, .. } => timeout.elapsed() > timeout_duration,
            ConnectionState::Writing { timeout, .. } => timeout.elapsed() > timeout_duration,
            ConnectionState::Processing { .. } | ConnectionState::Closing => false,
        }
    }
}
