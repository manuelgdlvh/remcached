use std::future::Future;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

pub async fn async_measure<F, T>(counter: &AtomicU64, elapsed: &AtomicU64, f: F) -> T
where
    F: Future<Output=T>,
    T: Send,
{
    let start = Instant::now();
    let result = f.await;
    let elapsed_ms = start.elapsed().as_millis() as u64;
    elapsed.fetch_add(elapsed_ms, Relaxed);
    counter.fetch_add(1, Relaxed);
    result
}

pub fn measure<F, T>(counter: &AtomicU64, elapsed: &AtomicU64, f: F) -> T
where
    F: FnOnce() -> T,
    T: Send,
{
    let start = Instant::now();
    let result = f();
    let elapsed_ms = start.elapsed().as_millis() as u64;
    elapsed.fetch_add(elapsed_ms, Relaxed);
    counter.fetch_add(1, Relaxed);
    result
}