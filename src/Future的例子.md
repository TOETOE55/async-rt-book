# Future的例子

刚刚通过接口的形式来介绍`Future`，那么我们怎么实现一个`Future`呢？除了`async {}`这种由编译器生成的`Future`以外，这里稍微给几个简单的例子。



## `yield_now()`

第一次轮询时返回`Pending`，第二次轮询时就绪。用于临时交还控制流用：

```rust
struct YieldNow {
    is_ready: bool
}

impl Future for YieldNow {
    type Output = ();
    
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.is_ready {
            return Poll::Ready(());
        }
        
        self.is_ready = true;
        // 通知调用方，自己有“进展”
        cx.waker().wake_by_ref();
        // 交还控制流
        Poll::Pending
    }
}

pub async fn yield_now() {
    YieldNow { is_ready: false }.await
}
```



## stdin

这个例子通过多线程模拟异步的IO，这里以stdin为例子（Rust标准库里的`Stdin`是同步阻塞的）：

```rust
// 提交到stdin线程处理的任务
struct StdinTask {
    buf: Mutex<Box<[u8]>>,
    waker: AtomicWaker,
    res: Mutex<Option<io::Result<usize>>>,
}

fn task_sender() -> &'static Sender<Arc<StdinTask>> {
    static SENDER: OnceCell<Sender<Arc<StdinTask>>> = OnceCell::new();
    SENDER.get_or_init_bloking(|| {
        let (tx, rx) = mpsc::channel();
        // 单起一个线程来处理stdin的读任务
        thread::spawn(move || {
            for mut task in rx {
                // 同步阻塞地读stdin
                let res = stdin().read(&mut task.buf.lock());
                // 将读的结果塞回去
                *task.res.lock() = Some(res);
                // 通知已完成
           		task.waker.take().map(Waker::wake);
            }
        }).unwrap();
        
        tx
    })
}


impl StdinTask {
    fn poll_read(&self, cx: &mut Contex<'_>) -> Poll<()> {
        // 检查任务是否完成
        if let Some(res) = self.res.lock() {
            return Poll::Ready(())
        }
        
        // 重新注册一遍waker，因为有可能不是同一个运行时poll了。
        self.waker.register(cx.waker().clone());
        Poll::Pending
    }
}

// 对外提供的接口
pub async fn read_stdin(buf: Box<[u8]>) -> io::Result<(usize, Box<u8>)> {
    let task = Arc::new(StdinTask {
        buf: Mutex::new(buf),
        waker: AtomicWaker::new(),
        res: Mutex::new(None),
    });
    
    // 发送到stdin线程处理
    task_sender().send(task.clone()).unwrap();
    
    // 等待task被处理完
    poll_fn(|cx| task.poll_read(cx)).await;
    
    // 返回读的结果
    let task = Arc::try_unwrap(task).unwrap();
    res.map(|i| (i, task.buf.into_inner()))
}

```

但这种用线程来模拟异步io的方式开销会比较大，我们需要通过系统提供的异步io来进行改进。





## `join`组合子

我们有了前面“单功能”的`Future`之后，我们还可以通过写一些组合子去把他们组合起来。比如`join`组合子用于并发执行多个`Future`，直到所有的`Future`都完成。

```rust
#[pin_project]
struct Join<FA, FB> 
where
	FA: Future,
	FB: Future,
{
    #[pin]
    fut_a: Option<FA>,
    a: Option<<FA as Future>::Output>,
    #[pin]
    fut_b: Option<FB>,
    b: Option<<FB as Future>::Output>,
}

impl<FA, FB> Join<FA, FB>
where
	FA: Future,
	FB: Future,
{
    type Output = (FA::Output, FB::Output);
    
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        
        
        loop {
            // 当a fut未完成时poll
            if let Some(fut_a) = this.fut_a.as_pin_mut() {
                if let Poll::Ready(a) = fut_a.poll(cx) {
                    this.a = Some(a);
      				this.fut_a.set(None);
                    continue;
                }
            }
            // 当b fut未完成时poll
            if let Some(fut_b) = this.fut_b.as_pin_mut() {
                if let Poll::Ready(b) = fut_b.poll(cx) {
                    this.b = Some(b);
      				this.fut_b.set(None);
                }
            }
            
            // 当两个future都成功时返回Ready
            // 否则返回Pending
            if let (Some(a), Some(b)) = (this.a, this.b) {
                return Poll::Ready((this.a.take().unwrap(), this.b.take().unwrap()));
            } else {
                return Poll::Pending;
            }
        }
    }
}
```



