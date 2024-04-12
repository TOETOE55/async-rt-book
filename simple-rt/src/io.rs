use crate::reactor::Reactor;
use std::{
    io::{self, Read},
    os::fd::AsFd,
};

pub struct Stdin<'r> {
    reactor: &'r Reactor,
    stdin: io::Stdin,
}

impl<'r> Stdin<'r> {
    pub fn new(reactor: &'r Reactor) -> io::Result<Self> {
        let this = Self {
            reactor,
            stdin: io::stdin(),
        };

        rustix::io::ioctl_fionbio(&this.stdin, true)?;

        Ok(this)
    }

    pub async fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.stdin.lock().read(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return res,
            }
            self.reactor.register_readable(self.stdin.as_fd()).await?;
        }
    }
}
