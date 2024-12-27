#[derive(Copy, Clone)]
pub struct RCacheConfig {
    cache_id: &'static str,
    entry_expires_in: u64,
}

impl RCacheConfig {
    pub fn new(cache_id: &'static str, entry_expires_in: u64) -> Self {
        Self { cache_id, entry_expires_in }
    }
    pub fn entry_expires_in(&self) -> u64 {
        self.entry_expires_in
    }
    pub fn cache_id(&self) -> &'static str {
        self.cache_id
    }
}


