use std::time::Duration;

#[derive(Copy, Clone)]
pub struct CacheManagerConfig {
    max_pending_ms_await: Duration,
    max_pending_first_poll_ms_await: Duration,
    max_pending_bulk_poll_ms_await: Duration,
    max_task_drain_size: u64,
}

impl CacheManagerConfig {
    pub fn new(max_pending_ms_await: u64, max_pending_first_poll_ms_await: u64, max_pending_bulk_poll_ms_await: u64, max_task_drain_size: u64) -> Self {
        Self {
            max_pending_ms_await: Duration::from_millis(max_pending_ms_await),
            max_pending_first_poll_ms_await: Duration::from_millis(max_pending_first_poll_ms_await),
            max_pending_bulk_poll_ms_await: Duration::from_millis(max_pending_bulk_poll_ms_await),
            max_task_drain_size,
        }
    }

    pub fn max_pending_ms_await(&self) -> Duration {
        self.max_pending_ms_await
    }
    pub fn max_pending_first_poll_ms_await(&self) -> Duration {
        self.max_pending_first_poll_ms_await
    }
    pub fn max_pending_bulk_poll_ms_await(&self) -> Duration {
        self.max_pending_bulk_poll_ms_await
    }
    pub fn max_task_drain_size(&self) -> u64 {
        self.max_task_drain_size
    }
}
