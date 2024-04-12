# block_on

前面花了比较大的笔墨在介绍`Future`上。现在终于可以去聊聊异步运行时的事了。



异步运行时的最低要求是可以求值一个`Future`，这个接口的签名写起来就是：

```rust
/// 阻塞地求值一个`Future`，并把future的结果返回出来
fn block_on<T>(fut: impl Future<Output = T>) -> T;
```

> 因为rust本身的运行时只能直接调用同步的接口，所以提供一个同步的`block_on`接口是必要的，作为“异步转同步”的入口。



按照上面`Future`求值的模板，如果除了求值`Future`之外什么都不干的话，我们就可以在`poll`一个`Future`返回`Pending`时，休眠线程：

```rust
fn block_on<T>(fut: impl Future<Output = T>) -> T {
    // 当前线程的`parker`和`unparker`
    let (parker, unparker) = parking::pair();
    // waker在调用`.wake()`时unpark当前线程
    let waker = waker_fn(move || { unparker.unpark(); });
    let cx = &mut Context::from_waker(&waker);
    
    // 轮询`Future`
    let mut fut = pin!(fut);
    loop {
        if let Poll::Ready(t) = fut.as_mut().poll(cx) {
            return t;
        }
        
        // 返回`Pending`就休眠线程，等待`waker`被调用
        // 注：如果waker已经被调用过了，这里就不会阻塞。
        parker.park()
    }
}
```

这就是`block_on`最基础的实现（当然如果在嵌入式里没有park/unpark，也只有单线程的情况下就另当别论），也是一个异步运行时最简单的形式。之后我们在这个基础上不断添加新的东西来完善它。



有了`block_on`我们就可以写一个简单的异步程序了：

```rust
fn main() {
    // 读一次stdin，并打印
    let fut_a = async {
        let mut buf = Box::<[u8]>::from([0;100]);
        let mut i = 0;
        (i, buf) = read_stdin(buf).await?;
        println!("{:?}", String::from_utf8_lossy(&buf[..i]));
        Ok(())
    };
    
    let fut_b = async {
        yield_now().await;
        println!("yield 1");
        yield_now().await;
        println!("yield 2");
    };
    
    // 并发执行fut_a, fut_b
    let fut = join(fut_a, fut_b);
    
    // 执行fut
    block_on(fut);
}
```



