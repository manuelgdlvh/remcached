use thiserror::Error;

#[derive(Error, Debug)]
pub enum ManualOperationError {
    #[error("Invalid cache. Cache: [{cache_id}]")]
    CacheNotFound {
        cache_id: &'static str,
    },
    #[error("Invalid input.")]
    InvalidInput,
    #[error("Send task error.")]
    SendError,
}