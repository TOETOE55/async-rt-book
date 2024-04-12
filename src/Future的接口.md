# Future的接口

`Future`的接口采用的是基于轮询的形式，而非更加常见的CPS形式：

> 为了方便叙述，这里先去掉一些噪音，化简了一下现有接口

```rust
/// 异步计算的抽象
trait Future {
    type Output;
    
    /// 提供一个轮询的接口
    fn poll(&mut self, waker: Waker) -> Poll<Self::Output>;
}

/// 轮询的结果
enum Poll<T> {
    Pending,
    Ready(T)
}

#[derive(Clone)]
struct Waker { /*...*/ }
impl Waker {
    /// 当异步计算有进展时调用，以通知轮询方进行下一轮的轮询
    fn wake(self);
}
```

我们需要拿到一个`Future`的值，我们需要不断地调用`poll`轮询它：

* 当`poll`返回`Pending`的时候，表示`Future`还没完成，且暂时不需要占用当前控制流。从`Future`的角度来说，则是让出了当前的控制流，让我们可以做一些其它的事情。

  > 相比于同步阻塞的IO，异步IO当资源未就绪时返回Pending，可以避免陷入内核态，同时能减少上下文切换的开销

* 当`poll`返回`Ready`的时候，则表示`Future`的计算已完成。

当然，除了`poll`以外，还可以取消一个`Future`，只需要不再轮询它，这时可以选择析构掉`Future`，释放掉里面的资源（这时对于`Future`来说，相当于在`.await`处panic了）。

其中`poll`还有一个参数`Waker`，当`Future`有进展时，就可以调用`.wake()`，来通知轮询方继续轮询`Future`。其中`Waker`，满足`Send`和 `Sync`，意味着`.wake()`方法可以在任何地方调用，比如说把`Waker`注册给OS，由OS来调用`.wake()`。

> 注意：这个Waker参数并不一定是自上而下传递下来的，也有可能是poll中间构造的，甚至可以来自于别的运行时的。



于是对一个`Future`求值最基础的程序就长这样：

```rust
// 轮询future
loop {
    match fut.poll(waker) {
        Pending => {
            // 当异步计算不需要占用当前线程控制流的时候，会让出控制流，于是可以做一些其它事情
        }
        Ready(r) => {
            // 计算完成
            break r
        }
    }
    
    // 当`fut`有进一步进展时，可以进一步轮询。
    if todo!("fut 有进展") {
        continue;
    }
}

```



不过这里补充一点，`poll`一个`Future`的策略完全由轮询方来决定，不同的业务场景可以以不同的方式去轮询。`Waker`不调用的时候也轮询方也可以去`poll`一个`Future`；反过来`Waker`被调用了，也可以不立刻去`poll`。比如我们可以“马不停蹄”地轮询`Future`

```rust
loop {
    // 返回`Pending`时，立刻继续`poll`，直到返回`Ready`，
    // 对于不希望线程休眠的程序的运行时，就可以这么设计
    if let Ready(r) = fut.poll(waker) {
        return r;
    }
}
```



> 作为对比，这里简单地把基于CPS变换的异步计算的抽象列在这里：
>
> ```rust
> trait Future {
>  type Output;
> 
>  /// 1.`schedule`和`callback`不应该阻塞，`callback`可能会被注册到一些地方
>  /// 2. 当异步计算**完成**时，`callback`就会被调用。
>  fn schedule<Callback>(self, callback: Callback)
>  where
>  	Callback: FnOnce(Self::Output) + Send + Sync
> }
> ```
>
> 这时候对`Future`的求值就和我们在其他语言（比如js）中见到的类似了：
>
> ```rust
> fut.schedule(|output| {
>  // 异步计算完成
>  do_something(output);
> });
> ```



其它更复杂的异步接口，在rust里也都可以，（也倾向于）设计成`poll_xxx`的形式（注：后续所有的waker参数都替换成现在的`Context`）：

* 异步流：`fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>`
* 异步读：`fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize, Error>>`
* 异步写：`fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>>`

等等。



`Future`的接口其实是脱胎于`Iterator`的接口，都是通过**“外部”**轮询的方式来获取下一步结果，**同样也可以造出很多不同功能的组合子**（也可以叫Adapter）。相对于传统回调的方式，这种方式更符合Rust的哲学——*零开销抽象*，同时在borrow checker下这样的接口也更易使用一些。

这里更深入的讨论就不展开了，大家有兴趣可以看一下这些资料：

* [Why async Rust?](https://without.boats/blog/why-async-rust/)

* [Designing futures for Rust](https://aturon.github.io/blog/2016/09/07/futures-design/)