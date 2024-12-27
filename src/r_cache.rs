use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::{Sender, SendError};

use dashmap::{DashMap, Entry};

use crate::cache_manager::CacheManager;
use crate::cache_task::{CacheTask, Invalidation};
use crate::async_executor::AsyncExecutor;
use crate::metrics::{async_measure, measure};
use crate::r_cache_config::RCacheConfig;
use crate::r_commands::RCommands;
use crate::types::{GenericError, GenericType};

struct CacheTaskProducer<RC>
where
    RC: RCommands + 'static,
{
    config: RCacheConfig,
    statistics: TaskProducerStatistics,
    tx: Sender<CacheTask>,
    _phantom: PhantomData<RC>,
}

impl<RC> CacheTaskProducer<RC>
where
    RC: RCommands + 'static,
{
    pub fn new(config: RCacheConfig, tx: Sender<CacheTask>) -> Self {
        Self { config, statistics: Default::default(), tx, _phantom: Default::default() }
    }

    pub fn expire(&self, key: RC::Key) -> Result<(), SendError<CacheTask>> {
        let task = CacheTask::entry_expiration(self.config.entry_expires_in(), self.config.cache_id(), key);
        self.send(task)
    }

    fn send(&self, task: CacheTask) -> Result<(), SendError<CacheTask>> {
        let result = self.tx.send(task);
        self.statistics.tasks_produced_total.fetch_add(1, Relaxed);
        if result.is_err() {
            self.statistics.tasks_produced_errors_total.fetch_add(1, Relaxed);
        }

        result
    }
}

#[derive(Default, Debug)]
pub struct TaskProducerStatistics {
    tasks_produced_total: AtomicU64,
    tasks_produced_errors_total: AtomicU64,
}
impl TaskProducerStatistics {
    pub fn tasks_produced_total(&self) -> u64 {
        self.tasks_produced_total.load(Relaxed)
    }
    pub fn tasks_produced_errors_total(&self) -> u64 {
        self.tasks_produced_errors_total.load(Relaxed)
    }
}

#[derive(Default, Debug)]
pub struct CacheStatistics {
    hits_total: AtomicU64,
    hits_time_ms: AtomicU64,
    miss_total: AtomicU64,
    miss_time_ms: AtomicU64,
    gets_errors_total: AtomicU64,
    puts_total: AtomicU64,
    puts_errors_total: AtomicU64,
    puts_time_ms: AtomicU64,
    invalidations_processed_total: AtomicU64,
    invalidations_processed_time_ms: AtomicU64,
    expirations_processed_total: AtomicU64,
    expirations_processed_time_ms: AtomicU64,
}

impl CacheStatistics {
    pub fn hits_total(&self) -> u64 {
        self.hits_total.load(Relaxed)
    }
    pub fn hits_time_ms(&self) -> u64 {
        self.hits_time_ms.load(Relaxed)
    }
    pub fn miss_time_ms(&self) -> u64 {
        self.miss_time_ms.load(Relaxed)
    }
    pub fn puts_total(&self) -> u64 {
        self.puts_total.load(Relaxed)
    }

    pub fn puts_errors_total(&self) -> u64 {
        self.puts_errors_total.load(Relaxed)
    }
    pub fn puts_time_ms(&self) -> u64 {
        self.puts_time_ms.load(Relaxed)
    }
    pub fn miss_total(&self) -> u64 {
        self.miss_total.load(Relaxed)
    }
    pub fn gets_errors_total(&self) -> u64 {
        self.gets_errors_total.load(Relaxed)
    }

    pub fn invalidations_processed_total(&self) -> u64 {
        self.invalidations_processed_total.load(Relaxed)
    }
    pub fn invalidations_processed_time_ms(&self) -> u64 {
        self.invalidations_processed_time_ms.load(Relaxed)
    }
    pub fn expirations_processed_total(&self) -> u64 {
        self.expirations_processed_total.load(Relaxed)
    }
    pub fn expirations_processed_time_ms(&self) -> u64 {
        self.expirations_processed_time_ms.load(Relaxed)
    }
}

pub struct RCache<RC>
where
    RC: RCommands + 'static,
{
    task_producer: CacheTaskProducer<RC>,
    storage: DashMap<RC::Key, RC::Value>,
    statistics: CacheStatistics,
    remote_commands: RC,
    config: RCacheConfig,
    initialized: AtomicBool,
}


impl<RC> RCache<RC>
where
    RC: RCommands + 'static,
{
    pub fn build<E: AsyncExecutor>(cache_manager: &mut CacheManager<E>, config: RCacheConfig, remote_commands: RC) -> Arc<RCache<RC>> {
        let self_ = Arc::new(Self {
            task_producer: CacheTaskProducer::new(config, cache_manager.sender()),
            storage: DashMap::default(),
            statistics: Default::default(),
            initialized: Default::default(),
            config,
            remote_commands,
        });
        cache_manager.register(RCacheInputValidator::<RC>::new(), RCacheTaskProcessor::new(self_.clone()));
        self_
    }
    pub async fn get(&self, key: &RC::Key) -> Result<Option<RC::Value>, GenericError> {
        if !self.is_initialized() {
            return async_measure(&self.statistics.miss_total, &self.statistics.miss_time_ms, async {
                self.get_internal(key).await
            }).await;
        }

        return match self.storage.get(key) {
            None => {
                async_measure(&self.statistics.miss_total, &self.statistics.miss_time_ms, async {
                    log::trace!("cache miss for #{key} entry and #{} cache", self.cache_id());
                    let val = match self.get_internal(key).await? {
                        None => {
                            return Ok(None);
                        }
                        Some(val) => {
                            val
                        }
                    };

                    match self.storage.entry(key.clone()) {
                        Entry::Occupied(_) => {}
                        Entry::Vacant(entry) => {
                            entry.insert(val.clone());
                            if let Err(err) = self.task_producer.expire(key.clone()) {
                                log::error!("add expiration failed for #{key} entry and #{} cache caused by: {err}", self.cache_id());
                                self.storage.remove(key);
                            }
                        }
                    }


                    Ok(Some(val))
                }).await
            }
            Some(val) => {
                measure(&self.statistics.hits_total, &self.statistics.hits_time_ms, || {
                    log::trace!("cache hit for #{key} entry and #{} cache", self.cache_id());
                    Ok(Some(val.value().clone()))
                })
            }
        };
    }


    pub async fn put(&self, key: &RC::Key, value: &RC::Value) -> Result<(), GenericError> {
        if !self.is_initialized() {
            return async_measure(&self.statistics.puts_total, &self.statistics.puts_time_ms, async {
                self.put_internal(key, value).await
            }).await;
        }

        async_measure(&self.statistics.puts_total, &self.statistics.puts_time_ms, async {
            self.put_internal(key, value).await?;

            self.storage.insert(key.clone(), value.clone());
            if let Err(err) = self.task_producer.expire(key.clone()) {
                log::error!("expiration failed for #{key} entry and #{} cache caused by: {err}", self.cache_id());
                self.storage.remove(key);
            }

            Ok(())
        }).await
    }

    async fn put_internal(&self, key: &RC::Key, value: &RC::Value) -> Result<(), GenericError> {
        let result = self.remote_commands.put(key, value).await;
        if result.is_err() {
            self.statistics.puts_errors_total.fetch_add(1, Relaxed);
            return result;
        }

        result
    }

    async fn get_internal(&self, key: &RC::Key) -> Result<Option<RC::Value>, GenericError> {
        let result = self.remote_commands.get(key).await;
        if result.is_err() {
            log::error!("get failed for #{key} entry and #{} cache", self.cache_id());
            self.statistics.gets_errors_total.fetch_add(1, Relaxed);
        }
        result
    }
    fn is_initialized(&self) -> bool {
        if !self.initialized.load(Ordering::Acquire) {
            log::warn!("#{} cache is not initialized!", self.cache_id());
            return false;
        }

        true
    }
    pub fn statistics(&self) -> &CacheStatistics {
        &self.statistics
    }
    pub fn task_producer_statistics(&self) -> &TaskProducerStatistics {
        &self.task_producer.statistics
    }
    pub fn len(&self) -> usize {
        self.storage.len()
    }
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }
    pub fn contains_key(&self, key: &RC::Key) -> bool {
        self.storage.contains_key(key)
    }

    pub fn cache_id(&self) -> &'static str {
        self.config.cache_id()
    }
}


pub trait CacheTaskProcessor: Send + Sync {
    fn init(&self);
    fn stop(&self);
    fn flush_all(&self);
    fn expire(&self, key: GenericType);
    fn invalidate_with_properties(&self, invalidation: GenericType);
    fn invalidate(&self, key: GenericType) -> Pin<Box<dyn Future<Output=Result<(), GenericError>> + Send>>;
    fn cache_id(&self) -> &'static str;
}

struct RCacheTaskProcessor<RC>
where
    RC: RCommands + 'static,
{
    ref_: Arc<RCache<RC>>,
}

impl<RC> RCacheTaskProcessor<RC>
where
    RC: RCommands + 'static,
{
    pub fn new(ref_: Arc<RCache<RC>>) -> Self {
        Self { ref_ }
    }
}

impl<RC> CacheTaskProcessor for RCacheTaskProcessor<RC>
where
    RC: RCommands,
{
    fn init(&self) {
        self.ref_.initialized.store(true, Ordering::Release);
        log::info!("initialized #{} cache", self.ref_.cache_id());
    }

    fn stop(&self) {
        self.ref_.initialized.store(false, Ordering::Release);
        log::info!("stopped #{} cache", self.ref_.cache_id());
    }

    fn flush_all(&self) {
        self.stop();
        self.ref_.storage.clear();
        log::info!("flushed all entries of #{} cache", self.ref_.cache_id());
        self.init();
    }

    fn expire(&self, key: GenericType) {
        measure(&self.ref_.statistics.expirations_processed_total, &self.ref_.statistics.expirations_processed_time_ms, || {
            let key = key.downcast::<RC::Key>().expect("Key conversion successfully");
            log::trace!("#{key} entry expired successfully for #{} cache", self.ref_.cache_id());
            self.ref_.storage.remove(&key);
        })
    }

    fn invalidate_with_properties(&self, invalidation: GenericType) {
        measure(&self.ref_.statistics.invalidations_processed_total, &self.ref_.statistics.invalidations_processed_time_ms, || {
            let invalidation = invalidation.downcast::<Invalidation<RC::Key, RC::Value>>().expect("Invalidation conversion successfully");

            match self.ref_.storage.entry(invalidation.key().clone()) {
                Entry::Occupied(entry) => {
                    entry.replace_entry(invalidation.value().clone());
                }
                Entry::Vacant(entry) => {
                    log::trace!("#{} entry invalidation skipped due to not found in #{} cache", entry.key(), self.ref_.cache_id());
                    return;
                }
            }

            if self.ref_.task_producer.expire(invalidation.key().clone()).is_err() {
                self.ref_.storage.remove(invalidation.key());
                return;
            }

            log::trace!("#{} entry invalidated with properties successfully for #{} cache", invalidation.key(), self.ref_.cache_id());
        })
    }


    fn invalidate(&self, key: GenericType) -> Pin<Box<dyn Future<Output=Result<(), GenericError>> + Send>> {
        let ref_ = Arc::clone(&self.ref_);
        Box::pin(
            async move {
                let key = key.downcast::<RC::Key>().expect("Key conversion successfully");

                if !ref_.storage.contains_key(&key) {
                    log::trace!("#{} entry invalidation skipped due to not found in #{} cache", key, ref_.cache_id());
                    return Ok(());
                }

                let result = match ref_.get_internal(&key).await? {
                    None => {
                        log::trace!("#{} entry invalidation skipped due to not found in remote repository for #{} cache", key, ref_.cache_id());
                        return Ok(());
                    }
                    Some(result) => {
                        result
                    }
                };

                match ref_.storage.entry(*key.clone()) {
                    Entry::Occupied(entry) => {
                        entry.replace_entry(result);
                    }
                    Entry::Vacant(entry) => {
                        log::trace!("#{} entry invalidation skipped due to not found in #{} cache", entry.key(), ref_.cache_id());
                        return Ok(());
                    }
                }

                if ref_.task_producer.expire(*key.clone()).is_err() {
                    ref_.storage.remove(&key);
                }

                log::trace!("#{} entry invalidated successfully for #{} cache", key, ref_.cache_id());
                Ok(())
            }
        )
    }


    fn cache_id(&self) -> &'static str {
        self.ref_.cache_id()
    }
}


pub trait InputValidator: Send {
    fn validate(&self, input: &GenericType, is_invalidation_with_props: bool) -> bool;
}
struct RCacheInputValidator<RC>
where
    RC: RCommands + 'static,
{
    phantom_: PhantomData<RC>,
}
impl<RC> RCacheInputValidator<RC>
where
    RC: RCommands + 'static,
{
    pub fn new() -> Self {
        Self { phantom_: Default::default() }
    }
}

impl<RC> InputValidator for RCacheInputValidator<RC>
where
    RC: RCommands + 'static,
{
    fn validate(&self, input: &GenericType, is_invalidation: bool) -> bool {
        if !is_invalidation {
            return input.downcast_ref::<RC::Key>().is_some();
        }
        input.downcast_ref::<Invalidation<RC::Key, RC::Value>>().is_some()
    }
}


