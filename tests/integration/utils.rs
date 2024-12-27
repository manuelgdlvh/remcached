use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sqlx::{Pool, Postgres, query, Row};
use tokio::runtime::Handle;
use tokio::time::sleep;
use remcached::async_executor::tokio::TokioAsyncExecutor;
use remcached::cache_manager::CacheManager;
use remcached::cache_manager_config::CacheManagerConfig;
use remcached::r_cache::RCache;
use remcached::r_cache_config::RCacheConfig;
use remcached::r_commands::RCommands;
use remcached::types::GenericError;

use crate::containers::get_db_pool;

pub const MAX_POLL_INTERVAL_AWAIT: u64 = 200;


pub const MAX_POLL_AWAIT: u64 = 20000;
pub const MAX_PENDING_FIRST_POLL_MS_AWAIT: u64 = 2000;
pub const MAX_PENDING_MS_AWAIT: u64 = 5000;
pub const MAX_PENDING_BULK_POLL_MS_AWAIT: u64 = 50;
const MAX_TASK_DRAIN_SIZE: u64 = 4096;


pub const EXPECT_GET_SUCCESSFULLY: &str = "Get operation successfully";
pub const EXPECT_ENTRY_FOUND: &str = "Exists entry for given id";

pub const LOWER_EXP_TIME: u64 = 100;
pub const STORED_GAME_ID: i64 = 47557;
pub const NOT_STORED_GAME_ID: i64 = 4723423557;
pub const CACHE_ID: &str = "game_repo";
pub const CACHE_ID_2: &str = "game_repo_2";


pub async fn async_await_until<F>(poll_interval: u64, max_poll_await: u64, predicate: F)
where
    F: Fn() -> bool,
{
    let now = Instant::now();
    let max_await_time = Duration::from_millis(max_poll_await);
    loop {
        if now.elapsed() > max_await_time {
            panic!("Max await time reached!")
        }

        if predicate() {
            break;
        }

        sleep(Duration::from_millis(poll_interval)).await;
    }
}

pub async fn get_instances_not_init(configs: &[(&'static str, u64)]) -> (Vec<Arc<RCache<GameDbCommands>>>, CacheManager<TokioAsyncExecutor>)
{
    let db_pool = get_db_pool().await;
    let mut cache_manager = CacheManager::new(TokioAsyncExecutor::new(Handle::current())
                                              , CacheManagerConfig::new(MAX_PENDING_MS_AWAIT, MAX_PENDING_FIRST_POLL_MS_AWAIT, MAX_PENDING_BULK_POLL_MS_AWAIT, MAX_TASK_DRAIN_SIZE));

    let mut result = Vec::new();
    for &(cache_id, exp_time) in configs {
        let r_cache = RCache::build(
            &mut cache_manager,
            RCacheConfig::new(cache_id, exp_time),
            GameDbCommands::new(db_pool.clone()),
        );

        result.push(r_cache);
    }

    (result, cache_manager)
}


pub async fn get_instances(configs: &[(&'static str, u64)]) -> (Vec<Arc<RCache<GameDbCommands>>>, CacheManager<TokioAsyncExecutor>)
{
    let (instances, mut cache_manager) = get_instances_not_init(configs).await;
    cache_manager.start();
    (instances, cache_manager)
}


// Commands for this entity
pub struct GameDbCommands {
    db_pool: Pool<Postgres>,
}

impl GameDbCommands {
    pub fn new(db_pool: Pool<Postgres>) -> Self {
        Self { db_pool }
    }
}
impl RCommands for GameDbCommands {
    type Key = i64;
    type Value = Game;

    fn get(&self, key: &Self::Key) -> impl Future<Output=Result<Option<Self::Value>, GenericError>> + Send {
        async move {
            let val = query("SELECT game_id, name FROM game.game where game_id = $1")
                .bind(key)
                .fetch_one(&self.db_pool)
                .await?;

            Ok(Some(Game::new(val.try_get::<i64, &str>("game_id")? as u64
                              , val.try_get("name")?)))
        }
    }


    fn put(&self, key: &Self::Key, value: &Self::Value) -> impl Future<Output=Result<(), GenericError>> + Send {
        async move {
            query("INSERT INTO game.game (game_id, name) VALUES ($1, $2)")
                .bind(key)
                .bind(&value.title)
                .execute(&self.db_pool)
                .await?;

            Ok(())
        }
    }
}

// Entity
#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    pub id: u64,
    pub title: String,
}
impl Game {
    pub fn new(id: u64, title: String) -> Self {
        Self { id, title }
    }
}
