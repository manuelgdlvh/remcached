use std::future::Future;

use crate::types::GenericError;

pub trait AsyncExecutor: Send + 'static {
    type Task: AsyncTask;
    fn execute<F>(&self, exp_time: u128, future: F) -> Self::Task
    where
        F: Future<Output=Result<(), GenericError>> + Send + 'static;
}


pub trait AsyncTask: Send {
    fn is_finished(&self) -> bool;
    fn is_expired(&self) -> bool;
    fn abort(&self);
}


#[cfg(feature = "tokio")]
pub mod tokio {
    use std::future::Future;
    use tokio::runtime::Handle;
    use tokio::task::JoinHandle;
    use crate::async_executor::{AsyncExecutor, AsyncTask};
    use crate::types::GenericError;
    use crate::utils::is_now_after;

    pub struct TokioAsyncExecutor {
        handle: Handle,
    }

    impl TokioAsyncExecutor {
        pub fn new(handle: Handle) -> Self {
            Self { handle }
        }
    }

    impl AsyncExecutor for TokioAsyncExecutor {
        type Task = TokioAsyncTask;
        fn execute<F>(&self, exp_time: u128, future: F) -> Self::Task
        where
            F: Future<Output=Result<(), GenericError>> + Send + 'static,
        {
            let task = self.handle.spawn(future);
            TokioAsyncTask::new(task, exp_time)
        }
    }

    pub struct TokioAsyncTask {
        delegate: JoinHandle<Result<(), GenericError>>,
        exp_time: u128,
    }

    impl TokioAsyncTask {
        pub fn new(delegate: JoinHandle<Result<(), GenericError>>, exp_time: u128) -> Self {
            Self { delegate, exp_time }
        }
    }

    impl AsyncTask for TokioAsyncTask {
        fn is_finished(&self) -> bool {
            self.delegate.is_finished()
        }

        fn is_expired(&self) -> bool {
            is_now_after(self.exp_time)
        }

        fn abort(&self) {
            self.delegate.abort();
        }
    }
}

