use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::mem;

use crate::types::GenericType;
use crate::utils::{calculate_time_until_next_ms, is_now_after};

pub enum CacheTask {
    Init { cache_id: &'static str },
    Stop { cache_id: &'static str },
    FlushAll { cache_id: &'static str },
    EntryExpiration { exec_time: u128, cache_id: &'static str, id: String, key: GenericType },
    Invalidation { exp_time: u128, cache_id: &'static str, id: String, key: GenericType },
    InvalidationWithProperties { cache_id: &'static str, id: String, invalidation: GenericType },
}

pub struct Invalidation<K, V> {
    key: K,
    value: V,
}

impl<K, V> Invalidation<K, V> {
    pub fn new(key: K, value: V) -> Self {
        Self { key, value }
    }
    pub fn key(&self) -> &K {
        &self.key
    }
    pub fn value(&self) -> &V {
        &self.value
    }
}


impl CacheTask {
    pub fn entry_expiration<K>(exec_in: u64, cache_id: &'static str, key: K) -> CacheTask
    where
        K: Send + ToString + 'static,
    {
        let id = key.to_string();
        CacheTask::EntryExpiration {
            exec_time: calculate_time_until_next_ms(exec_in),
            cache_id,
            id,
            key: Box::new(key) as GenericType,
        }
    }

    pub fn invalidation_with_properties<K, V>(cache_id: &'static str, key: K, value: V) -> CacheTask
    where
        K: Send + ToString + 'static,
        V: Send + 'static,
    {
        let id = key.to_string();
        CacheTask::InvalidationWithProperties {
            cache_id,
            id,
            invalidation: Box::new(Invalidation::new(key, value)) as GenericType,
        }
    }

    pub fn invalidation<K>(expires_in: u64, cache_id: &'static str, key: K) -> CacheTask
    where
        K: Send + ToString + 'static,
    {
        let id = key.to_string();
        CacheTask::Invalidation {
            exp_time: calculate_time_until_next_ms(expires_in),
            cache_id,
            id,
            key: Box::new(key) as GenericType,
        }
    }

    pub fn flush_all(cache_id: &'static str) -> CacheTask {
        CacheTask::FlushAll {
            cache_id,
        }
    }

    pub fn init(cache_id: &'static str) -> CacheTask {
        CacheTask::Init {
            cache_id,
        }
    }

    pub fn stop(cache_id: &'static str) -> CacheTask {
        CacheTask::Stop {
            cache_id,
        }
    }

    pub fn exec_time(&self) -> u128 {
        match self {
            CacheTask::EntryExpiration { exec_time, .. } => *exec_time,
            _ => 0,
        }
    }

    pub fn exp_time(&self) -> Option<u128> {
        match self {
            CacheTask::Invalidation { exp_time, .. } => Some(*exp_time),
            _ => None,
        }
    }
    pub fn is_async(&self) -> bool {
        match self {
            CacheTask::Invalidation { .. } => true,
            _ => { false }
        }
    }


    pub fn id(&self) -> Option<&str> {
        match self {
            CacheTask::Invalidation { id, .. } => Some(id),
            CacheTask::EntryExpiration { id, .. } => Some(id),
            _ => { None }
        }
    }

    pub fn cache_id(&self) -> &'static str {
        match self {
            CacheTask::Invalidation { cache_id, .. }
            | CacheTask::InvalidationWithProperties { cache_id, .. }
            | CacheTask::Init { cache_id, .. }
            | CacheTask::Stop { cache_id, .. }
            | CacheTask::FlushAll { cache_id, .. }
            | CacheTask::EntryExpiration { cache_id, .. } => cache_id,
        }
    }
    pub fn is_executable(&self) -> bool {
        is_now_after(self.exec_time())
    }

    pub fn is_expired(&self) -> bool {
        match self.exp_time() {
            None => {
                false
            }
            Some(exp_time) => {
                is_now_after(exp_time)
            }
        }
    }
}


impl Hash for CacheTask {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.cache_id().hash(state);
        self.id().hash(state);
    }
}
impl Eq for CacheTask {}
impl PartialEq<Self> for CacheTask {
    fn eq(&self, other: &Self) -> bool {
        let cache_matches = self.cache_id().eq(other.cache_id());
        let op_matches = mem::discriminant(self) == mem::discriminant(other);
        let input_matches = self.id().eq(&other.id());
        cache_matches && op_matches && input_matches
    }
}
impl PartialOrd<Self> for CacheTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for CacheTask {
    // Ordering as priority
    fn cmp(&self, other: &Self) -> Ordering {
        if self.exec_time() > other.exec_time() {
            return Ordering::Less;
        }

        Ordering::Greater
    }
}





