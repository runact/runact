use std::collections::HashMap;
use std::io;
use std::os::unix::io::RawFd;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Interest(u32);

impl Interest {
    pub const READABLE: Interest = Interest(0b001);
    pub const WRITABLE: Interest = Interest(0b010);

    pub fn contains(self, other: Interest) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for Interest {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Interest(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for Interest {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Interest(self.0 & rhs.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Readiness(u32);

impl Readiness {
    pub fn is_readable(self) -> bool {
        self.0 & Interest::READABLE.0 != 0
    }

    pub fn is_writable(self) -> bool {
        self.0 & Interest::WRITABLE.0 != 0
    }
}

#[derive(Debug)]
pub struct Event {
    pub token: u64,
    pub readiness: Readiness,
}

pub struct Reactor {
    epoll_fd: RawFd,
    registrations: HashMap<RawFd, u64>,
}

impl Reactor {
    pub fn new() -> io::Result<Self> {
        let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if epoll_fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            epoll_fd,
            registrations: HashMap::new(),
        })
    }

    pub fn len(&self) -> usize {
        self.registrations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.registrations.is_empty()
    }

    pub fn register(&mut self, fd: RawFd, interest: Interest) -> io::Result<()> {
        self.register_with_token(fd, interest, fd as u64)
    }

    pub fn register_with_token(
        &mut self,
        fd: RawFd,
        interest: Interest,
        token: u64,
    ) -> io::Result<()> {
        let mut epoll_event = libc::epoll_event {
            events: interest_to_epoll(interest),
            u64: token,
        };
        let ret =
            unsafe { libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut epoll_event) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        self.registrations.insert(fd, token);
        Ok(())
    }

    pub fn unregister(&mut self, fd: RawFd) -> io::Result<()> {
        let ret = unsafe {
            libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut())
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        self.registrations.remove(&fd);
        Ok(())
    }

    pub fn modify(&self, fd: RawFd, interest: Interest) -> io::Result<()> {
        let token = self.registrations.get(&fd).copied().unwrap_or(fd as u64);
        let mut epoll_event = libc::epoll_event {
            events: interest_to_epoll(interest),
            u64: token,
        };
        let ret =
            unsafe { libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_MOD, fd, &mut epoll_event) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn wait(&self, timeout: Duration) -> io::Result<Vec<Event>> {
        let mut events = vec![libc::epoll_event { events: 0, u64: 0 }; 64];
        let timeout_ms = timeout.as_millis() as i32;
        let n = unsafe {
            libc::epoll_wait(
                self.epoll_fd,
                events.as_mut_ptr(),
                events.len() as i32,
                timeout_ms,
            )
        };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                return Ok(Vec::new());
            }
            return Err(err);
        }
        Ok(events[..n as usize]
            .iter()
            .map(|e| Event {
                token: e.u64,
                readiness: epoll_to_readiness(e.events),
            })
            .collect())
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.epoll_fd);
        }
    }
}

fn interest_to_epoll(interest: Interest) -> u32 {
    let mut events: u32 = 0;
    if interest.contains(Interest::READABLE) {
        events |= libc::EPOLLIN as u32;
    }
    if interest.contains(Interest::WRITABLE) {
        events |= libc::EPOLLOUT as u32;
    }
    events
}

fn epoll_to_readiness(events: u32) -> Readiness {
    let mut ready = 0u32;
    if events & libc::EPOLLIN as u32 != 0 {
        ready |= Interest::READABLE.0;
    }
    if events & libc::EPOLLOUT as u32 != 0 {
        ready |= Interest::WRITABLE.0;
    }
    Readiness(ready)
}
