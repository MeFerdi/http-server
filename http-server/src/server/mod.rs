pub mod connection;
pub mod epoll;
pub mod event_loop;
pub mod listener;

use std::io;

use crate::config::Config;

pub use event_loop::TIMEOUT_DURATION;

pub fn run(config: &Config) -> io::Result<()> {
	event_loop::run(config)
}
