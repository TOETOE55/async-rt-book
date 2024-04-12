use std::{
    future::{Future, IntoFuture},
    pin::pin,
    task::{Context, Poll},
};
use waker_fn::waker_fn;

pub fn block_on<T>(fut: impl IntoFuture<Output = T>) -> T {
    // 当前线程的`parker`和`unparker`
    let (parker, unparker) = parking::pair();
    // waker在调用`.wake()`时unpark当前线程
    let waker = waker_fn(move || {
        unparker.unpark();
    });
    let cx = &mut Context::from_waker(&waker);

    // 轮询`Future`
    let mut fut = pin!(fut.into_future());
    loop {
        if let Poll::Ready(t) = fut.as_mut().poll(cx) {
            return t;
        }

        // 返回`Pending`就休眠线程，等待`waker`被调用
        // 注：如果waker已经被调用过了，这里就不会阻塞。
        parker.park()
    }
}
