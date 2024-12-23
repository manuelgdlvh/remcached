# RCache

## Overview

The RCache is a caching system designed for efficient storage and retrieval of entities from remote repositories (
REST APIs, Database, ...etc). This cache system allows you to set custom expiration times for cache entries, enabling
automatic
cleanup of expired data to prevent memory bloat and ensure that the cache remains fresh and accurate. The cache
management system is flexible and can be integrated into your application to enhance performance by reducing remote
operations
load and improving response times for frequently accessed data.

Manual Operations Supported:
1. Force Entry Expiration
2. Invalidation
3. Flush All
4. Initialize
5. Stop

Note: When the cache is stopped, all requests will be directed to the remote repository. This is particularly useful for flushing all data or performing intensive operations in the cache without blocking.

Automatic Operations:

1. Entry Expiration

RCache and Cache Manager provide read-only structures that expose detailed statistics for monitoring.

## Key Features

1. Automatic Cache Cleanup: Once an entry expires, it is automatically removed from the cache, maintaining memory
   efficiency.
2. Declarative Cache Design: Cache configuration is intuitive and allows developers to easily define expiration policies
   for different types of data.
3. Cache Efficiency: Reduces the need to repeatedly query the remote repository by storing frequently accessed data, leading to
   faster response times.

## How to Use

1. Implement the RCommands Trait

The first step is to implement the RCommands trait. This trait should define the essential operations for your specific
repository:

    PUT: This operation stores a new entity or updates an existing one in the cache and remote repository.
    GET: This operation retrieves the entity from the cache, if it exists, otherwise it can fetch the data from the remote repository.

The trait ensures that all operations are standardized and consistent across different repositories.

2. Construct a Cache Manager

Once the RCommands trait is implemented, you need to construct a CacheManager struct.
The CacheManager is responsible for managing multiple cache repositories and handling events such as entry expiration,
invalidation, flush all, ...etc.

3. Build Your Cache Repositories

For each type of entity you want to cache, you need to create a corresponding repository that leverages the cache
system. Each repository should handle the specific operations for the entity, ensuring that put and get operations
interact with both the cache and the underlying remote repository in a consistent manner.

Repositories can be easily built using the RCommands trait, and you can define custom expiration policies based on the
entity type (for example, a short-lived session cache or a longer-lived configuration data cache).

4. Start the Cache Manager

Once the cache repositories are built, you can start the CacheManager. It will initialize and begin managing cache
operations for all the repositories that have been registered. The CacheManager will automatically handle cache
expiration according to the rules you have set.

## Example of usage

```

async fn main() {
    let db_pool = get_db_pool().await;
    let mut cache_manager = CacheManager::new(CacheManagerConfig::new(MAX_PENDING_MS_AWAIT, 20, 2048));
    let r_cache = RCache::build(
            &mut cache_manager,
            RCacheConfig::new("cache_id", 60000),
            GameDbCommands::new(db_pool),
        );

    cache_manager.start();
    let result = r_cache.get(&1234).await;
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



```