use std::collections::{BinaryHeap, HashMap, HashSet};
use std::collections::hash_set::Iter;
use std::sync::{Arc, mpsc};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;
use std::thread;

use crate::cache_manager_config::CacheManagerConfig;
use crate::cache_task::CacheTask;
use crate::metrics::measure;
use crate::r_cache::CacheTaskProcessor;
use crate::types::{GenericError};

#[derive(Default, Debug)]
pub struct CacheManagerStatistics {
    tasks_total: AtomicU64,
    pending_tasks_total: AtomicU64,
    cycles_total: AtomicU64,
    cycles_time_ms: AtomicU64,
}

impl CacheManagerStatistics {
    fn init() -> Self {
        Self {
            tasks_total: Default::default(),
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
    fn push_tasks(&mut self) -> u64 {
        let mut tasks_pushed = 0;
        if let Ok(task) = self.rx.recv_timeout(self.config.max_pending_ms_await()) {
            self.tasks.push(task);
            tasks_pushed += 1;
            while tasks_pushed < self.config.max_task_drain_size() {
                match self.rx.recv_timeout(self.config.max_pending_bulk_ms_await()) {
                    Ok(task) => {
                        self.tasks.push(task);
                        tasks_pushed += 1;
                    }
                    Err(_) => break,
                }
            }
        }

        tasks_pushed
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
        match task {
            CacheTask::Invalidation { cache_id, invalidation, .. } => {
                self.task_processors.get(cache_id)
                    .expect("Invalidator found")
                    .invalidate(invalidation)
            }
            CacheTask::Init { cache_id, .. } => {
                self.task_processors.get(cache_id)
                    .expect("Invalidator found")
                    .init()
            }
            CacheTask::Stop { cache_id, .. } => {
                self.task_processors.get(cache_id)
                    .expect("Invalidator found")
                    .stop()
            }
            CacheTask::FlushAll { cache_id, .. } => {
                self.task_processors.get(cache_id)
                    .expect("Invalidator found")
                    .flush_all()
            }
            CacheTask::EntryExpiration { cache_id, key, .. } => {
                self.task_processors.get(cache_id)
                    .expect("Invalidator found")
                    .expire(key)
            }
        }
    }
}

pub struct CacheManager {
    inner: Option<CacheManagerInner>,
    cache_ids: HashSet<&'static str>,
    statistics: Arc<CacheManagerStatistics>,
    tx: mpsc::Sender<CacheTask>,
}


impl CacheManager {
    pub fn new(config: CacheManagerConfig) -> Self {
        let (tx, rx) = mpsc::channel::<CacheTask>();
        let inner = Some(CacheManagerInner::new(config, rx));
        Self { inner, cache_ids: Default::default(), statistics: CacheManagerStatistics::share_init(), tx }
    }
    pub fn register<T>(&mut self, task_processor: T)
    where
        T: CacheTaskProcessor + 'static,
    {
        self.cache_ids.insert(task_processor.cache_id());
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
                        let pushed_tasks = inner.push_tasks();
                        log::debug!("#{pushed_tasks} pushed cache tasks");
                        statistics.tasks_total.fetch_add(pushed_tasks, Relaxed);
                        let processed_tasks = inner.process_tasks();
                        log::debug!("#{processed_tasks} processed cache tasks");
                        statistics.pending_tasks_total.store(inner.tasks.len() as u64, Relaxed);
                        log::debug!("#{} remaining cache tasks to be processed", inner.tasks.len());
                    });
                }
            }
        });
    }


    pub fn init(&self, cache_id: &'static str) -> Result<(), GenericError> {
        self.validate_cache_id(cache_id)?;
        let task = CacheTask::init(cache_id);
        self.send(task)?;
        Ok(())
    }

    pub fn stop(&self, cache_id: &'static str) -> Result<(), GenericError> {
        self.validate_cache_id(cache_id)?;
        let task = CacheTask::stop(cache_id);
        self.send(task)?;
        Ok(())
    }
    pub fn flush_all(&self, cache_id: &'static str) -> Result<(), GenericError> {
        self.validate_cache_id(cache_id)?;
        let task = CacheTask::flush_all(cache_id);
        self.send(task)?;
        Ok(())
    }

    pub fn force_expiration<K>(&self, cache_id: &'static str, key: K) -> Result<(), GenericError>
    where
        K: Send + 'static,
    {
        self.validate_cache_id(cache_id)?;
        let task = CacheTask::entry_expiration(0, cache_id, key);
        self.send(task)?;
        Ok(())
    }
    pub fn invalidate<K, V>(&self, cache_id: &'static str, key: K, value: V) -> Result<(), GenericError>
    where
        K: Send + 'static,
        V: Send + 'static,
    {
        self.validate_cache_id(cache_id)?;
        let task = CacheTask::invalidation(cache_id, key, value);
        self.send(task)?;
        Ok(())
    }
    fn send(&self, task: CacheTask) -> Result<(), GenericError> {
        let result = self.tx.send(task);
        if result.is_err() {
            log::error!("error sending task from manual operation");
            return Err("MANUAL_OPERATION_FAILED".into());
        }

        Ok(())
    }
    fn validate_cache_id(&self, cache_id: &'static str) -> Result<(), GenericError> {
        let result = self.cache_ids.contains(cache_id);
        if !result {
            log::error!("#{} cache id is not registered", cache_id);
            return Err("NO_CACHE_FOUND".into());
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







