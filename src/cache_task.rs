use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::GenericType;

pub enum CacheTask {
    Init { exp_time: u128, cache_id: &'static str },
    Stop { exp_time: u128, cache_id: &'static str },
    FlushAll { exp_time: u128, cache_id: &'static str },
    EntryExpiration { exp_time: u128, cache_id: &'static str, key: GenericType },
    Invalidation { exp_time: u128, cache_id: &'static str, invalidation: GenericType },
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
        K: Send + 'static,
    {
        let key = Box::new(key) as GenericType;
        let exp_time = SystemTime::now().duration_since(UNIX_EPOCH)
            .expect("Time went backwards").as_millis() + expires_in as u128;

        CacheTask::EntryExpiration {
            exp_time,
            cache_id,
            key,
        }
    }

    pub fn invalidation<K, V>(cache_id: &'static str, key: K, value: V) -> CacheTask
    where
        K: Send + 'static,
        V: Send + 'static,
    {
        let invalidation = Box::new(Invalidation::new(key, value)) as GenericType;
        let exp_time = SystemTime::now().duration_since(UNIX_EPOCH)
            .expect("Time went backwards").as_millis();

        CacheTask::Invalidation {
            exp_time,
            cache_id,
            invalidation,
        }
    }

    pub fn flush_all(cache_id: &'static str) -> CacheTask {
        let exp_time = SystemTime::now().duration_since(UNIX_EPOCH)
            .expect("Time went backwards").as_millis();

        CacheTask::FlushAll {
            exp_time,
            cache_id,
        }
    }

    pub fn init(cache_id: &'static str) -> CacheTask {
        let exp_time = SystemTime::now().duration_since(UNIX_EPOCH)
            .expect("Time went backwards").as_millis();

        CacheTask::Init {
            exp_time,
            cache_id,
        }
    }

    pub fn stop(cache_id: &'static str) -> CacheTask {
        let exp_time = SystemTime::now().duration_since(UNIX_EPOCH)
            .expect("Time went backwards").as_millis();

        CacheTask::Stop {
            exp_time,
            cache_id,
        }
    }

    pub fn exp_time(&self) -> u128 {
        match self {
            CacheTask::Invalidation { exp_time, .. } => {
                *exp_time
            }
            CacheTask::Init { exp_time, .. } => {
                *exp_time
            }
            CacheTask::Stop { exp_time, .. } => {
                *exp_time
            }
            CacheTask::FlushAll { exp_time, .. } => {
                *exp_time
            }
            CacheTask::EntryExpiration { exp_time, .. } => {
                *exp_time
            }
        }
    }
    pub fn is_expired(&self) -> bool {
        self.exp_time() <= SystemTime::now().duration_since(UNIX_EPOCH)
            .expect("Time went backwards").as_millis()
    }
}

impl Eq for CacheTask {}

impl PartialEq<Self> for CacheTask {
    fn eq(&self, other: &Self) -> bool {
        self.exp_time() == other.exp_time()
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





