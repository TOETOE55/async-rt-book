use std::{io, task::Poll, thread};

use futures::future::poll_fn;

use simple_rt::{block_on, Executor, Reactor, Stdin};

async fn yield_now() {
    let mut x = false;
    poll_fn(|cx| {
        if x {
            Poll::Ready(())
        } else {
            x = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await;
}

fn main() -> io::Result<()> {
    let reactor = Reactor::new()?;
    let ex = Executor::new();

    thread::scope(|s| {
        // reactor io线程，用于处理IO事件
        s.spawn(|| reactor.event_loop().unwrap());

        // 起8个线程作为线程池，来并发执行task
        for _ in 0..8 {
            s.spawn(|| {
                block_on(ex.execute());
            });
        }

        // 创建异步任务，丢到Executor中执行
        ex.spawn(async {
            let mut buf = [0; 1000];
            let mut buf = &mut buf[..];
            let stdin = Stdin::new(&reactor).unwrap();

            while buf.len() > 0 {
                let x = stdin.read(buf).await.unwrap();
                println!("from stdin: {:?}", String::from_utf8_lossy(&buf[..x]));

                buf = &mut buf[x..];
            }
        })
        .detach();

        // 这个也会丢到Executor，然后被并发执行
        ex.spawn(async {
            yield_now().await;
            println!("yield 1");
            yield_now().await;
            println!("yield 2");
        })
        .detach();
    });

    Ok(())
}
