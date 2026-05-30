use std::io;
use std::mem;
use std::os::fd::RawFd;

pub struct Epoll {
    fd: RawFd,
}

impl Epoll {
    pub fn new() -> io::Result<Self> {
        let fd = unsafe { libc::epoll_create1(0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { fd })
    }

    pub fn add(&self, fd: RawFd, events: u32) -> io::Result<()> {
        self.ctl(libc::EPOLL_CTL_ADD, fd, events)
    }

    pub fn modify(&self, fd: RawFd, events: u32) -> io::Result<()> {
        self.ctl(libc::EPOLL_CTL_MOD, fd, events)
    }

    pub fn delete(&self, fd: RawFd) -> io::Result<()> {
        let rc = unsafe { libc::epoll_ctl(self.fd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut()) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn wait(&self, max_events: usize, timeout_ms: i32) -> io::Result<Vec<libc::epoll_event>> {
        let mut events = vec![unsafe { mem::zeroed::<libc::epoll_event>() }; max_events];
        let ready = unsafe {
            libc::epoll_wait(
                self.fd,
                events.as_mut_ptr(),
                max_events as i32,
                timeout_ms,
            )
        };
        if ready < 0 {
            return Err(io::Error::last_os_error());
        }
        events.truncate(ready as usize);
        Ok(events)
    }

    fn ctl(&self, op: i32, fd: RawFd, events: u32) -> io::Result<()> {
        let mut event = libc::epoll_event {
            events,
            u64: fd as u64,
        };

        let rc = unsafe { libc::epoll_ctl(self.fd, op, fd, &mut event as *mut _) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for Epoll {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}
