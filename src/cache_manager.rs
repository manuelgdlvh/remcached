use std::collections::{BinaryHeap, HashMap, HashSet};
use std::collections::hash_set::Iter;
use std::sync::{Arc, mpsc};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;
use std::thread;

use crate::cache_manager_config::CacheManagerConfig;
use crate::cache_task::CacheTask;
use crate::errors::ManualOperationError;
use crate::metrics::measure;
use crate::r_cache::{CacheTaskProcessor, InputValidator};

#[derive(Default, Debug)]
pub struct CacheManagerStatistics {
    tasks_total: AtomicU64,
    merged_tasks_total: AtomicU64,
    pending_tasks_total: AtomicU64,
    cycles_total: AtomicU64,
    cycles_time_ms: AtomicU64,
}

impl CacheManagerStatistics {
    fn init() -> Self {
        Self {
            tasks_total: Default::default(),
            merged_tasks_total: Default::default(),
            pending_tasks_total: Default::default(),
            cycles_total: Default::default(),
            cycles_time_ms: Default::default(),
        }
    }

    pub fn share_init() -> Arc<Self> {
        Arc::new(Self::init())
    }
    pub fn tasks_total(&self) -> u64 {
        self.tasks_total.load(Relaxed)
    }
    pub fn merged_tasks_total(&self) -> u64 {
        self.merged_tasks_total.load(Relaxed)
    }
    pub fn pending_tasks_total(&self) -> u64 {
        self.pending_tasks_total.load(Relaxed)
    }
    pub fn cycles_total(&self) -> u64 {
        self.cycles_total.load(Relaxed)
    }
    pub fn cycles_time_ms(&self) -> u64 {
        self.cycles_time_ms.load(Relaxed)
    }
}


pub struct CacheManagerInner {
    tasks: BinaryHeap<CacheTask>,
    task_processors: HashMap<&'static str, Box<dyn CacheTaskProcessor>>,
    config: CacheManagerConfig,
    rx: mpsc::Receiver<CacheTask>,
}

impl CacheManagerInner {
    pub fn new(config: CacheManagerConfig, rx: mpsc::Receiver<CacheTask>) -> Self {
        Self { tasks: Default::default(), task_processors: Default::default(), config, rx }
    }
    pub fn register<T>(&mut self, task_processor: T)
    where
        T: CacheTaskProcessor + 'static,
    {
        if self.task_processors.contains_key(task_processor.cache_id()) {
            panic!("#{} cache currently registered!", task_processor.cache_id());
        }

        self.task_processors.insert(task_processor.cache_id(), Box::new(task_processor));
    }
    fn push_tasks(&mut self) -> (u64, u64) {
        let mut pending_tasks = HashSet::new();
        let mut merged_tasks = 0;
        if let Ok(task) = self.rx.recv_timeout(self.config.max_pending_ms_await()) {
            if pending_tasks.replace(task).is_some() {
                merged_tasks += 1;
            }
            while pending_tasks.len() < self.config.max_task_drain_size() as usize {
                match self.rx.recv_timeout(self.config.max_pending_bulk_ms_await()) {
                    Ok(task) => {
                        if pending_tasks.replace(task).is_some() {
                            merged_tasks += 1;
                        }
                    }
                    Err(_) => break,
                }
            }
        }

        let tasks_len = self.tasks.len();
        self.tasks.retain(|task| !pending_tasks.contains(task));
        let merged_tasks = merged_tasks + (tasks_len - self.tasks.len());

        let tasks_pushed = pending_tasks.len();
        self.tasks.extend(pending_tasks);
        (merged_tasks as u64, tasks_pushed as u64)
    }

    fn process_tasks(&mut self) -> u64 {
        let mut processed_tasks = 0;

        loop {
            match self.tasks.peek() {
                Some(val) if !val.is_expired() => break,
                Some(_) => {
                    let task = self.tasks.pop().expect("Task Found");
                    self.execute(task);
                    processed_tasks += 1;
                }
                None => break,
            }
        }

        processed_tasks
    }

    fn execute(&self, task: CacheTask) {
        let task_processor = self.task_processors.get(task.cache_id()).expect("Task processor found");
        match task {
            CacheTask::Invalidation { invalidation, .. } => task_processor.invalidate(invalidation),
            CacheTask::EntryExpiration { key, .. } => task_processor.expire(key),
            CacheTask::Init { .. } => task_processor.init(),
            CacheTask::Stop { .. } => task_processor.stop(),
            CacheTask::FlushAll { .. } => task_processor.flush_all(),
        }
    }
}

pub struct CacheManager {
    inner: Option<CacheManagerInner>,
    cache_ids: HashSet<&'static str>,
    input_validators: HashMap<&'static str, Box<dyn InputValidator>>,
    statistics: Arc<CacheManagerStatistics>,
    tx: mpsc::Sender<CacheTask>,
}


impl CacheManager {
    pub fn new(config: CacheManagerConfig) -> Self {
        let (tx, rx) = mpsc::channel::<CacheTask>();
        let inner = Some(CacheManagerInner::new(config, rx));
        Self { inner, cache_ids: Default::default(), input_validators: Default::default(), statistics: CacheManagerStatistics::share_init(), tx }
    }
    pub fn register<I, T>(&mut self, input_validator: I, task_processor: T)
    where
        I: InputValidator + 'static,
        T: CacheTaskProcessor + 'static,
    {
        self.cache_ids.insert(task_processor.cache_id());
        self.input_validators.insert(task_processor.cache_id(), Box::new(input_validator));
        self.inner.as_mut()
            .expect("Cache manager inner found").register(task_processor);
    }

    pub fn sender(&self) -> mpsc::Sender<CacheTask> {
        self.tx.clone()
    }

    pub fn start(&mut self) {
        let mut inner = self.inner.take().expect("Cache manager inner found");

        inner.task_processors
            .values()
            .for_each(|val| val.init());

        thread::spawn({
            let statistics = Arc::clone(&self.statistics);
            move || {
                loop {
                    measure(&statistics.cycles_total, &statistics.cycles_time_ms, || {
                        let (merged, pushed) = inner.push_tasks();
                        log::debug!("#{pushed} pushed cache tasks");
                        log::debug!("#{merged} merged cache tasks");
                        statistics.tasks_total.fetch_add(pushed, Relaxed);
                        statistics.merged_tasks_total.fetch_add(merged, Relaxed);
                        let processed_tasks = inner.process_tasks();
                        log::debug!("#{processed_tasks} processed cache tasks");
                        statistics.pending_tasks_total.store(inner.tasks.len() as u64, Relaxed);
                        log::debug!("#{} remaining cache tasks to be processed", inner.tasks.len());
                    });
                }
            }
        });
    }


    pub fn init(&self, cache_id: &'static str) -> anyhow::Result<(), ManualOperationError> {
        let task = CacheTask::init(cache_id);
        self.send(task)?;
        Ok(())
    }

    pub fn stop(&self, cache_id: &'static str) -> anyhow::Result<(), ManualOperationError> {
        let task = CacheTask::stop(cache_id);
        self.send(task)?;
        Ok(())
    }
    pub fn flush_all(&self, cache_id: &'static str) -> anyhow::Result<(), ManualOperationError> {
        let task = CacheTask::flush_all(cache_id);
        self.send(task)?;
        Ok(())
    }

    pub fn force_expiration<K>(&self, cache_id: &'static str, key: K) -> anyhow::Result<(), ManualOperationError>
    where
        K: Send + ToString + 'static,
    {
        let task = CacheTask::entry_expiration(0, cache_id, key);
        self.send(task)?;
        Ok(())
    }
    pub fn invalidate<K, V>(&self, cache_id: &'static str, key: K, value: V) -> anyhow::Result<(), ManualOperationError>
    where
        K: Send + ToString + 'static,
        V: Send + 'static,
    {
        let task = CacheTask::invalidation(cache_id, key, value);
        self.send(task)?;
        Ok(())
    }
    fn send(&self, task: CacheTask) -> anyhow::Result<(), ManualOperationError> {
        self.validate_cache_id(task.cache_id())?;
        self.validate_task(&task)?;
        let result = self.tx.send(task);
        if result.is_err() {
            return Err(ManualOperationError::SendError);
        }

        Ok(())
    }

    fn validate_task(&self, task: &CacheTask) -> anyhow::Result<(), ManualOperationError> {
        let result = match task {
            CacheTask::EntryExpiration { cache_id, key, .. } => {
                self.input_validators.get(cache_id)
                    .expect("Input validator found").validate(key, false)
            }
            CacheTask::Invalidation { cache_id, invalidation, .. } => {
                self.input_validators.get(cache_id)
                    .expect("Input validator found").validate(invalidation, true)
            }
            _ => { true }
        };

        if !result {
            return Err(ManualOperationError::InvalidInput);
        }

        Ok(())
    }
    fn validate_cache_id(&self, cache_id: &'static str) -> anyhow::Result<(), ManualOperationError> {
        let result = self.cache_ids.contains(cache_id);
        if !result {
            return Err(ManualOperationError::CacheNotFound { cache_id });
        }
        Ok(())
    }


    pub fn cache_ids(&self) -> Iter<&'static str> {
        self.cache_ids.iter()
    }
    pub fn statistics(&self) -> &Arc<CacheManagerStatistics> {
        &self.statistics
    }
}







