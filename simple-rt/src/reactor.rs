use futures::task::AtomicWaker;
use futures::Future;
use parking_lot::Mutex;
use polling::{Event, Events, Poller};
use slab::Slab;
use std::future::poll_fn;
use std::io::{self, ErrorKind};
use std::os::fd::RawFd;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use waker_fn::waker_fn;

pub struct Reactor {
    poller: Arc<Poller>,
    repo: Mutex<Slab<Arc<IOEvent>>>,
}

struct IOEvent {
    fd: RawFd,
    key: usize,
    is_ready: AtomicBool,
    waker: AtomicWaker,
}

impl Reactor {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            poller: Arc::new(Poller::new()?),
            repo: Mutex::new(Slab::new()),
        })
    }

    // 阻塞至关心的fd就绪
    pub fn block_until(&self) -> io::Result<()> {
        let mut events = Events::new();

        match self.poller.wait(&mut events, None) {
            Ok(0) => Ok(()),
            Ok(_) => {
                let repo = self.repo.lock();
                for ev in events.iter() {
                    if let Some(s) = repo.get(ev.key) {
                        let _ = s.waker.take().map(Waker::wake);
                        s.is_ready.swap(true, Ordering::Release);
                    }
                }
                Ok(())
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => Ok(()),
            Err(err) => Err(err),
        }
    }

    // 唤醒bloking_until
    pub fn waker(&self) -> Waker {
        let poller = self.poller.clone();
        waker_fn(move || {
            let _ = poller.notify();
        })
    }

    pub fn event_loop(&self) -> io::Result<()> {
        loop {
            self.block_until()?;
        }
    }

    // 注册可读fd，直到fd就绪
    pub async fn register_readable(&self, fd: BorrowedFd<'_>) -> io::Result<()> {
        struct IoGuard<'r> {
            reactor: &'r Reactor,
            event: Arc<IOEvent>,
        }

        impl<'r> IoGuard<'r> {
            fn new(reactor: &'r Reactor, fd: BorrowedFd<'_>) -> io::Result<Self> {
                let event = {
                    let mut events = reactor.repo.lock();
                    let entry = events.vacant_entry();
                    let event = Arc::new(IOEvent {
                        fd: fd.as_raw_fd(),
                        key: entry.key(),
                        is_ready: AtomicBool::new(false),
                        waker: AtomicWaker::new(),
                    });

                    entry.insert(event.clone());
                    event
                };

                if let Err(err) =
                    unsafe { reactor.poller.add(event.fd, Event::readable(event.key)) }
                {
                    let mut repo = reactor.repo.lock();
                    repo.remove(event.key);
                    return Err(err);
                }

                Ok(Self { reactor, event })
            }
        }

        impl Drop for IoGuard<'_> {
            fn drop(&mut self) {
                let mut repo = self.reactor.repo.lock();
                repo.remove(self.event.key);
                self.reactor
                    .poller
                    .delete(unsafe { BorrowedFd::borrow_raw(self.event.fd) })
                    .ok();
            }
        }

        let guard = IoGuard::new(self, fd)?;

        poll_fn(|cx| {
            let event = &*guard.event;
            if event.is_ready.load(Ordering::Acquire) {
                return Poll::Ready(Ok(()));
            }

            event.waker.register(cx.waker());

            Poll::Pending
        })
        .await
    }

    pub fn block_on<T, F>(fut: F) -> io::Result<T>
    where
        for<'r> F: FnOnce(&'r Self) -> BoxFuture<'r, T>,
    {
        let reactor = Reactor::new()?;
        let mut fut = fut(&reactor);

        let waker = reactor.waker();
        let cx = &mut Context::from_waker(&waker);

        loop {
            if let Poll::Ready(t) = fut.as_mut().poll(cx) {
                return Ok(t);
            }

            reactor.block_until()?;
        }
    }
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;
