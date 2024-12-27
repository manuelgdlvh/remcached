# RCache

## Overview

The RCache is a caching system designed for efficient storage and retrieval of entities from remote repositories (
REST APIs, Database, ...etc). This cache system allows you to set custom expiration times for cache entries, enabling
automatic
cleanup of expired data to prevent memory bloat and ensure that the cache remains fresh and accurate. The cache
management system is flexible and can be integrated into your application to enhance performance by reducing remote
operations
load and improving response times for frequently accessed data.

<b>Manual Operations through Cache Manager:</b>
1. Force Entry Expiration
2. Invalidation with properties (Replace entry with provided tuple of Key - Value)
3. Invalidation (Replace entry with latest value from remote repository)
4. Flush All
5. Initialize
6. Stop

<b>Note: When the cache is stopped, all requests will be directed to the remote repository. This is particularly useful for flushing all data or performing intensive operations in the cache without blocking.</b>

<b>Automatic Operations:</b>

1. Entry Expiration


## Crate features
1. <b>tokio</b> - Allow us to use TokioAsyncExecutor passing the Handle of your runtime for async cache tasks (Invalidation only at the moment!)

## Async Executor

The AsyncExecutor is a way to manage tasks that run asynchronously, separate from the Cache Manager (which does not support async tasks). You can either create your own async executor using the AsyncExecutor and AsyncTask traits or use the one that is already implemented, such as TokioAsyncExecutor (which is the only option available right now).

## Cache Manager Cycle. What is?
![cache_manager_task_cycle](https://github.com/user-attachments/assets/55b09b6d-9e39-49b8-9625-2bee02f2356c)



## Configuration
### Cache Manager
1. <b>max_pending_ms_await</b> - Maximum wait time (in milliseconds) between moving tasks from the input channel to the binary heap to process them.
2. <b>max_pending_first_poll_ms_await</b> - Maximum wait time to poll the first item in the channel.
3. <b>max_pending_bulk_poll_ms_await</b> - Maximum wait time to poll the next items of the channel after first poll.
4. <b>max_task_drain_size</b> - Maximum number of tasks polled from the channel after first poll.


### Cache
1. <b>cache_id</b> - Unique identifier of cache
2. <b>entry_expires_in</b> - Expiration time of cached entry



## Monitoring
RCache and Cache Manager provide read-only structures that expose detailed statistics for monitoring.
### Cache Manager
1. <b>tasks_total</b> - Total of tasks processed or ready to be processed.
2. <b>merged_tasks_total</b> - Tasks merged to reduce processing of same Operation - Key.
3. <b>expired_tasks_total</b> - Total tasks expired (Currently only can be expired invalidations using expires_in value).
4. <b>pending_tasks_total</b> - Total pending tasks waiting to be processed.
5. <b>pending_async_tasks_total</b> - Total pending async tasks scheduled in AsyncExecutor but not finished.
6. <b>expired_async_tasks_total</b> - Total expired async tasks that were scheduled but not finished on time and aborted.
7. <b>cycles_total</b> - Number of executed cycles.
8. <b>cycles_time_ms</b> - Total amount in milliseconds executing cycles.

### Cache
1. <b>hits_total</b> - Total amount of cache hits.
2. <b>hits_time_ms</b> - Total amount in milliseconds of cache hits.
3. <b>miss_total</b> - Total amount of cache miss.
4. <b>miss_time_ms</b> - Total amount in milliseconds of cache miss.
5. <b>gets_errors_total</b> - Total amount of errors in GET operations.
6. <b>puts_total</b> - Total amount of PUT operations.
7. <b>puts_errors_total</b> - Total amount of errors in PUT operations.
8. <b>puts_time_ms</b> - Total amount in milliseconds of PUT operations.
9. <b>invalidations_processed_total</b> - Total amount of invalidations processed. (With/out properties)
10. <b>invalidations_processed_time_ms</b> - Total amount in milliseconds of invalidations processed. (With/out properties)
11. <b>expirations_processed_total</b> - Total amount of entry expirations processed.
12. <b>expirations_processed_time_ms</b> - Total amount in milliseconds of entry expirations processed.




## How to Use (Using Database and Tokio runtime)


```
#[tokio::main]
async fn main() {
    
    -------------------- Cache Manager -------------------- 
    let async_executor = TokioAsyncExecutor::new(Handle::current());
    let mut cache_manager = CacheManager::new(async_executor, CacheManagerConfig::new(3000, 2000, 20, 2048));
    -------------------- Cache's --------------------
    let db_pool = get_db_pool().await;
    
    let r_cache = RCache::build(
            &mut cache_manager,
            RCacheConfig::new("cache_id", 60000),
            GameDbCommands::new(db_pool.clone()),
        );
    let r_cache_2 = RCache::build(
            &mut cache_manager,
            RCacheConfig::new("cache_id_2", 60000),
            GameDbCommands::new(db_pool),
        );
            
    -------------------- Start -------------------- 
    cache_manager.start();
    
    let result = r_cache.get(&1234).await;
    let result = r_cache_2.get(&1234).await;

}


-------------------- Commands -------------------- 

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

    fn get(&self, key: &Self::Key) -> impl Future<Output=Option<Self::Value>> + Send {
        async move {
            query("SELECT game_id, name FROM game.game where game_id = $1")
                .bind(key)
                .fetch_one(&self.db_pool)
                .await.ok().map(|val| {
                Game::new(val.try_get::<i64, &str>("game_id").unwrap() as u64
                          , val.try_get("name").unwrap())
            })
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

-------------------- Entity -------------------- 
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



```
