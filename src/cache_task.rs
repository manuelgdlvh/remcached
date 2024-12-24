use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::mem;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::GenericType;

pub enum CacheTask {
    Init { exp_time: u128, cache_id: &'static str },
    Stop { exp_time: u128, cache_id: &'static str },
    FlushAll { exp_time: u128, cache_id: &'static str },
    EntryExpiration { exp_time: u128, cache_id: &'static str, id: String, key: GenericType },
    Invalidation { exp_time: u128, cache_id: &'static str, id: String, invalidation: GenericType },
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
    pub fn entry_expiration<K>(expires_in: u64, cache_id: &'static str, key: K) -> CacheTask
    where
        K: Send + ToString + 'static,
    {
        let id = key.to_string();
        CacheTask::EntryExpiration {
            exp_time: Self::calculate_exp_time(expires_in as u128),
            cache_id,
            id,
            key: Box::new(key) as GenericType,
        }
    }

    pub fn invalidation<K, V>(cache_id: &'static str, key: K, value: V) -> CacheTask
    where
        K: Send + ToString + 'static,
        V: Send + 'static,
    {
        let id = key.to_string();
        CacheTask::Invalidation {
            exp_time: Self::calculate_exp_time(0),
            cache_id,
            id,
            invalidation: Box::new(Invalidation::new(key, value)) as GenericType,
        }
    }

    pub fn flush_all(cache_id: &'static str) -> CacheTask {
        CacheTask::FlushAll {
            exp_time: Self::calculate_exp_time(0),
            cache_id,
        }
    }

    pub fn init(cache_id: &'static str) -> CacheTask {
        CacheTask::Init {
            exp_time: Self::calculate_exp_time(0),
            cache_id,
        }
    }

    pub fn stop(cache_id: &'static str) -> CacheTask {
        CacheTask::Stop {
            exp_time: Self::calculate_exp_time(0),
            cache_id,
        }
    }


    pub fn exp_time(&self) -> u128 {
        match self {
            CacheTask::Invalidation { exp_time, .. }
            | CacheTask::Init { exp_time, .. }
            | CacheTask::Stop { exp_time, .. }
            | CacheTask::FlushAll { exp_time, .. }
            | CacheTask::EntryExpiration { exp_time, .. } => *exp_time,
        }
    }

    fn calculate_exp_time(expires_in: u128) -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH)
            .expect("Time went backwards").as_millis() + expires_in
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
            | CacheTask::Init { cache_id, .. }
            | CacheTask::Stop { cache_id, .. }
            | CacheTask::FlushAll { cache_id, .. }
            | CacheTask::EntryExpiration { cache_id, .. } => cache_id,
        }
    }
    pub fn is_expired(&self) -> bool {
        self.exp_time() <= SystemTime::now().duration_since(UNIX_EPOCH)
            .expect("Time went backwards").as_millis()
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
        if self.exp_time() > other.exp_time() {
            return Ordering::Less;
        }

        Ordering::Greater
    }
}





