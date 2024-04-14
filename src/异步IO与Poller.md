# 异步IO与Poller

既然标准库没有异步的IO接口，我们把目标转向OS自身提供的异步IO接口，比如linux下的`epoll`。`epoll`主要提供了三个接口：

```c
/// 创建新的epoll实例
int epoll_create(int size);

/// 向 epoll 实例中添加、修改或删除文件描述符
int epoll_ctl(int epfd, int op, int fd, struct epoll_event * event);

/// 等待 epoll 实例中的文件描述符准备就绪。
/// 这个函数会阻塞调用线程，到存在文件描述符就绪。
int epoll_wait(int epfd, struct epoll_event * events, int maxevents, int timeout);
```

核心思路是：

1. 把“关心”的IO的fd添加到`epoll`实例中（比如关心fd是否能写）
2. 然后调用`epoll_wait`，阻塞调用线程到存在关心的fd已就绪时，线程继续运行。
3. 这时候就能直接给已就绪的fd进行IO操作。

相对于标准库同步阻塞的IO操作来说，`epoll`把 *等待fd就绪* 这一步单独抽离了出来（也就是上面的第二步），允许同时监听多个fd，且允许超时。**这就允许用额外一个线程就可以处理多个IO事件，或者是通过时间片的方式来处理IO事件。**

不过要注意的是，**`epoll`并不支持普通的文件**，如果把文件fd添加到`epoll`里会返回`EPERM`错误。普通的文件读写需要用到aio或者使用一个线程池把同步转成异步。
`epoll`目前支持的fd是：

* 网络socket，比如说TCP, UDP等
* `timerfd`
* `signalfd`
* `inotify`
* pipe
* 子进程
* 终端相关，比如`stdin`, `stdout`, `stderr`
* `epoll`本身（可以加到其它`epoll`实例中）
* 等等


不同的操作系统都有类似的接口，rust里已经有crate进行统一封装，比如说[`mio`](https://crates.io/crates/mio), [`polling`](https://crates.io/crates/polling)。比如说`polling`提供提了以下接口，大体结构与`epoll`一致：

```rust
/// Poller实例，
/// * linux下是epoll实例
/// * mac, iOS下是kqueue实例
/// * windows下是iocp实例
struct Poller { /*...*/ } 

impl Poller {
    /// 创建一个poller实例
    pub fn new() -> Result<Poller>;
    
    /// 往poller实例中添加fd，及关心的事件
    pub unsafe fn add(
        &self,
        /// unix下是fd, windows下是socket
        source: impl AsRawSource,
        interest: Event
    ) -> Result<()>;
    
    /// 更新fd关心的事件，
    /// 比如关心fd是否可读，改成是否可写
    pub fn modify(&self, source: impl AsSource, interest: Event) -> Result<()>;
    
    /// 删除关心的fd
    pub fn delete(&self, source: impl AsSource) -> Result<()>;
    
    /// 阻塞调用线程直到
    /// * 存在关心的fd就绪，或
    /// * 超时
    /// * 唤醒poller实例
    pub fn wait(
        &self,
        events: &mut Events,
        /// * `None`为不设置超时
        /// * `Some(0)`为不阻塞
        timeout: Option<Duration>
    ) -> Result<usize>;
    
    /// 唤醒poller实例
    pub fn notify(&self) -> Result<()>;
}
```



这是`polling`库的Example（截自README）

> ```rust
> use polling::{Event, Poller};
> use std::net::TcpListener;
> 
> // Create a TCP listener.
> let socket = TcpListener::bind("127.0.0.1:8000")?;
> socket.set_nonblocking(true)?;
> let key = 7; // Arbitrary key identifying the socket.
> 
> // Create a poller and register interest in readability on the socket.
> let poller = Poller::new()?;
> poller.add(&socket, Event::readable(key))?;
> 
> // The event loop.
> let mut events = Vec::new();
> loop {
> // Wait for at least one I/O event.
> events.clear();
>  poller.wait(&mut events, None)?;
> 
>  for ev in &events {
>   if ev.key == key {
>          // Perform a non-blocking accept operation.
>          socket.accept()?;
>          // Set interest in the next readability event.
>          poller.modify(&socket, Event::readable(key))?;
>      }
>  }
> }
> ```

