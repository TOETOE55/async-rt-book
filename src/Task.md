# Task

我们把“等待Future的waker被调用，然后又丢到队列里”，这个部分单独抽出来，这个步骤称之为一次调度，被调度的称为一个`Task`。

`async_task`已经为我们封装好了这个抽象，它提供的核心接口为：

- `spawn` 创建一个task
- `Runnable::run` poll 一下 task中的future
- `schedule` 当task中的future被唤醒时，把task传到调度器里

```rust
// 接受一个`Future`和`schedule`创建一个`Task`
pub fn spawn<F, S>(future: F, schedule: S) -> (Runnable, Task<F::Output>)
where
    // 因为Future可以丢到任何线程中执行，所以要求Send
    F: Future + Send + 'static,
    F::Output: Send + 'static,
    // 因为schedule可以被任何地方随时调用，所以要求Send + Sync
    S: Fn(Runnable) + Send + Sync + 'static;

impl Runnable {
    // 执行一遍Future::poll()
    // 当wake被调用时，将task传到schedule里
    pub fn run(self) -> bool;
    
    // 直接把task传给schedule
    pub fn schedule(self);
}

// 当Future完成后，才会唤醒一次poll task的运行时
impl<T> Future for Task<T> {
    /*...*/
}
```



比如当`schedule`的作用是将task丢到队列里，就可以写成：

```rust
let (tx, rx) = channel();
// 把task丢到队列里
let schedule = move |runnable| tx.send(runnable).unwrap();

// 创建一个task
let (runnable, task) = async_task::spawn(yield_now(), schedule);

// 执行一遍poll，
// 当waker被调用时，就会把task传给schedule
runnable.run();

// 这里task没法完成，因为传到队列里后没有人执行
```

> 这里插入一些个人的想法：
> 
> rust在客户端最常见的使用方式之一就是作为跨端的业务层使用，被上层业务代码所调用。比如UIKit(iOS), Android等。
> 这些 *宿主* 通常都会自带一个异步的运行时。
> 利用`async_task`我们甚至可以利用“宿主语言”的异步运行时进行调度。
>


至于`async_task`是如何实现的，这里就不做太多展开了，大家可以直接去看源码。这里做点提示，这里`Task`, `Runnable`以及`Waker`背后都指向创建时的Future+schedule，因为职责不一样所以提供的方法不一样：

1. `Task`用于看future是否完成，以及取最终结果
2. `Runnable`用于执行`Future::poll`
3. `Waker`用于把Task传给`schedule`