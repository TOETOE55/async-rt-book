use std::marker::PhantomData;

use async_task::{Runnable, Task};
use futures::StreamExt;
use std::future::Future;

pub struct Executor<'a> {
    tx: flume::Sender<Runnable>,
    rx: flume::Receiver<Runnable>,

    /// Makes the `'a` lifetime invariant.
    _marker: PhantomData<std::cell::UnsafeCell<&'a ()>>,
}

impl<'a> Executor<'a> {
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();

        Self {
            tx,
            rx,
            _marker: PhantomData,
        }
    }

    // 创建一个Task，并丢到Executor中
    pub fn spawn<T: Send + 'a>(&self, future: impl Future<Output = T> + Send + 'a) -> Task<T> {
        let tx = self.tx.clone();
        let schedule = move |runnable| tx.send(runnable).unwrap();

        // 创建一个task，并直接丢到队列里
        let (runnable, task) = unsafe { async_task::spawn_unchecked(future, schedule) };
        runnable.schedule();

        task
    }

    // 当executor里有新的task时，就会拿出来执行
    pub async fn execute(&self) {
        let mut rx = self.rx.stream();
        while let Some(runnable) = rx.next().await {
            runnable.run();
        }
    }
}

impl Default for Executor<'_> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for Executor<'_> {}
unsafe impl Sync for Executor<'_> {}
