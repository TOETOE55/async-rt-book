# 一个完整的Executor的例子

这里的`Executor`提供两个核心的接口：

```rust
pub struct Executor<'a> {
    /// 这里用的是异步mpmc channel
    tx: Sender<Runnable>,
    rx: Receiver<Runnerble>,
    
    /// Makes the `'a` lifetime invariant.
    _marker: PhantomData<std::cell::UnsafeCell<&'a ()>>,
}

impl<'a> Executor<'a> {
    // 创建一个Task，并丢到Executor中
    pub fn spawn<T: Send + 'a>(&self, future: impl Future<Output = T> + Send + 'a) -> Task<T>;
    
    // 当executor里有新的task时，就会拿出来执行
    pub async fn execute(&self);
}
```



有了`async_task`我们就很容易实现这两个接口：

```rust
 pub fn spawn<T: Send + 'a>(&self, future: impl Future<Output = T> + Send + 'a) -> Task<T> {
     let tx = self.tx.clone();
     let schedule = move |runnable| tx.send(runnable).unwrap();
     
     // 创建一个task，并直接丢到队列里
     let (runnable, task) = unsafe { async_task::spawn_unchecked(future, schedule) };
     runnable.schedule();
     
     task
}

pub async fn execute(&self) {
    // 不断从队列里取task，并轮询
        let mut rx = self.rx.stream();
        while let Some(runnable) = rx.next().await {
            runnable.run();
        }
}
```



> 注：同`Reactor::event_loop`，`Executor::execute`也可以集成到`block_on`中。



有了`Executor`我们就可以利用多个线程来并发执行异步代码了：

```rust
let reactor = Reactor::new();
let executor = Executor::new();

thread::scope(|s| {
    // reactor io线程，用于处理IO事件
    s.spawn(|| reactor.event_loop().unwrap());
    
    // 起8个线程作为线程池，来并发执行task
    for _ in 0..8 {
        s.spawn(|| {
            block_on(executor.execute());
        });
    }
    
    // 创建异步任务，丢到Executor中执行
    executor.spawn(async {
        let mut buf = [0; 1000];
        let mut buf = &mut buf[..];
        let stdin = Stdin::new(reactor).unwrap();

        while buf.len() > 0 {
            let x = stdin.read(buf).await.unwrap();
            println!("from stdin: {:?}", String::from_utf8_lossy(&buf[..x]));

            buf = &mut buf[x..];
        }
    }).detach();
    
    // 这个也会丢到Executor，然后被并发执行
    executor.spawn(async {
        yield_now().await;
        println!("yield 1");
        yield_now().await;
        println!("yield 2");
    }).detach();
    
});
```

