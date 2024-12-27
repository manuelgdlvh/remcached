use crate::utils::EXPECT_ENTRY_FOUND;
use crate::utils::EXPECT_GET_SUCCESSFULLY;
use std::time::{Duration, Instant};

use rstest::rstest;
use sqlx::query;

use crate::containers::get_db_pool;
use crate::utils::{async_await_until, get_instances_not_init, LOWER_EXP_TIME, MAX_PENDING_BULK_POLL_MS_AWAIT, MAX_PENDING_MS_AWAIT, NOT_STORED_GAME_ID, STORED_GAME_ID};
use crate::utils::CACHE_ID;
use crate::utils::CACHE_ID_2;
use crate::utils::Game;
use crate::utils::get_instances;
use crate::utils::MAX_POLL_AWAIT;
use crate::utils::MAX_POLL_INTERVAL_AWAIT;

#[tokio::test]
#[rstest]
#[case(47530, "The Legend of Zelda: Ocarina of Time 3D", 1000)]
#[case(47557, "The Last of Us", 2500)]
#[case(47573, "American Truck Simulator", 5000)]
#[case(47577, "Euro Truck Simulator 2", 10000)]
async fn should_get_successfully(#[case] game_id: i64, #[case] title: &str, #[case] exp_time: u64) {
    let (instances, cache_manager) = get_instances(&[(CACHE_ID, exp_time)]).await;
    let cache_manager_statistics = cache_manager.statistics();
    let game_repo = instances.get(0).unwrap();

    assert_eq!(false, game_repo.contains_key(&game_id));
    let result = game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    assert_eq!(game_id as u64, result.id);
    assert_eq!(title, result.title);
    assert_eq!(1, game_repo.statistics().miss_total());

    // Cache hit
    assert_eq!(true, game_repo.contains_key(&game_id));
    let result = game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    assert_eq!(game_id as u64, result.id);
    assert_eq!(title, result.title);
    assert_eq!(1, game_repo.statistics().hits_total());

    // Wait invalidation
    awaitility::at_most(Duration::from_millis(MAX_POLL_AWAIT))
        .poll_interval(Duration::from_millis(MAX_POLL_INTERVAL_AWAIT))
        .until(|| !game_repo.contains_key(&game_id));

    game_repo.get(&game_id).await.unwrap();
    assert_eq!(2, game_repo.statistics().miss_total());
    assert_eq!(0, cache_manager_statistics.pending_tasks_total());
    assert_eq!(1, cache_manager_statistics.tasks_total());
}


#[tokio::test]
#[rstest]
#[case(47523423, "Dragon Ball Sparking Zero", 1000)]
#[case(42342347, "Naruto Shippuden", 2500)]
#[case(46556473, "Amazing Spiderman 2", 5000)]
async fn should_put_successfully(#[case] game_id: i64, #[case] title: &str, #[case] exp_time: u64) {
    let (instances, cache_manager) = get_instances(&[(CACHE_ID, exp_time)]).await;
    let cache_manager_statistics = cache_manager.statistics();
    let game_repo = instances.get(0).unwrap();

    // Non exists entry
    assert_eq!(true, game_repo.get(&game_id).await.is_err());
    assert_eq!(1, game_repo.statistics().miss_total());

    // Add entry
    let game = Game::new(game_id as u64, title.to_string());
    assert_eq!(true, game_repo.put(&game_id, &game).await.is_ok());
    assert_eq!(true, game_repo.contains_key(&game_id));

    game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    assert_eq!(1, game_repo.statistics().hits_total());

    // Wait invalidation
    awaitility::at_most(Duration::from_millis(MAX_POLL_AWAIT))
        .poll_interval(Duration::from_millis(MAX_POLL_INTERVAL_AWAIT))
        .until(|| !game_repo.contains_key(&game_id));

    game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    assert_eq!(2, game_repo.statistics().miss_total());

    assert_eq!(0, cache_manager_statistics.pending_tasks_total());
    assert_eq!(1, cache_manager_statistics.tasks_total());
}


#[tokio::test]
#[rstest]
#[case(47557, 5000)]
#[case(47573, 7500)]
#[case(47577, 10000)]
async fn should_process_events_in_priority_order(#[case] game_id: i64, #[case] exp_time: u64) {
    let (instances, cache_manager) = get_instances(&[(CACHE_ID, exp_time), (CACHE_ID_2, LOWER_EXP_TIME)]).await;
    let cache_manager_statistics = cache_manager.statistics();
    let game_repo = instances.get(0).unwrap();
    let game_repo_2 = instances.get(1).unwrap();

    // Access the cache for the repository with the higher expiration time first
    game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    game_repo_2.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);

    assert!(game_repo.contains_key(&game_id));
    assert!(game_repo_2.contains_key(&game_id));

    // Wait for the expiration of the lower expiration time configured

    awaitility::at_most(Duration::from_millis(MAX_POLL_AWAIT))
        .poll_interval(Duration::from_millis(MAX_POLL_INTERVAL_AWAIT))
        .until(|| !game_repo_2.contains_key(&game_id));

    // Assert that game_repo still holds the cached item, but game_repo_2 has invalidated it
    assert!(game_repo.contains_key(&game_id));
    assert_eq!(1, cache_manager_statistics.pending_tasks_total());
    assert_eq!(2, cache_manager_statistics.tasks_total());
}


#[tokio::test]
#[should_panic]
async fn should_panic_when_duplicated_cache_ids() {
    let _ = get_instances(&[(CACHE_ID, LOWER_EXP_TIME), (CACHE_ID, LOWER_EXP_TIME)]).await;
}


#[tokio::test]
#[rstest]
#[case([47557, 47573, 47577], 2500)]
async fn should_flush_all_successfully(#[case] input: [i64; 3], #[case] exp_time: u64) {
    let (instances, cache_manager) = get_instances(&[(CACHE_ID, exp_time)]).await;
    let cache_manager_statistics = cache_manager.statistics();
    let game_repo = instances.get(0).unwrap();

    let mut total_tasks = 0;
    for game_id in input.iter() {
        game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
            .expect(EXPECT_ENTRY_FOUND);
        assert_eq!(true, game_repo.contains_key(game_id));
        total_tasks += 1;
    }

    cache_manager.flush_all(CACHE_ID).unwrap();
    total_tasks += 1;
    awaitility::at_most(Duration::from_millis(MAX_POLL_AWAIT))
        .poll_interval(Duration::from_millis(MAX_POLL_INTERVAL_AWAIT))
        .until(|| cache_manager_statistics.pending_tasks_total() == 0 && cache_manager_statistics.tasks_total() == total_tasks);

    for game_id in input.iter() {
        assert_eq!(false, game_repo.contains_key(game_id));
    }
}


#[tokio::test]
#[rstest]
#[case(47557, 10000)]
async fn should_invalidate_with_properties_successfully(#[case] game_id: i64, #[case] exp_time: u64) {
    let (instances, cache_manager) = get_instances(&[(CACHE_ID, exp_time)]).await;
    let cache_manager_statistics = cache_manager.statistics();
    let game_repo = instances.get(0).unwrap();

    let mut total_tasks = 0;

    game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    assert_eq!(true, game_repo.contains_key(&game_id));
    total_tasks += 1;

    cache_manager.invalidate_with_properties(CACHE_ID, game_id, Game::new(game_id as u64, "Game test".to_string())).unwrap();
    total_tasks += 1;
    awaitility::at_most(Duration::from_millis(MAX_POLL_AWAIT))
        .poll_interval(Duration::from_millis(MAX_POLL_INTERVAL_AWAIT))
        .until(|| cache_manager_statistics.pending_tasks_total() == total_tasks - 1 && (cache_manager_statistics.tasks_total() == total_tasks
            || cache_manager_statistics.tasks_total() == total_tasks + 1));

    let result = game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    assert_eq!(game_id as u64, result.id);
    assert_eq!("Game test", result.title);
}


#[tokio::test]
#[rstest]
#[case(47557, 10000)]
async fn should_merge_related_cache_tasks(#[case] game_id: i64, #[case] exp_time: u64) {
    let (instances, cache_manager) = get_instances(&[(CACHE_ID, exp_time)]).await;
    let cache_manager_statistics = cache_manager.statistics();
    let game_repo = instances.get(0).unwrap();

    // Generate one expiration
    let mut total_tasks = 0;
    game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    assert_eq!(true, game_repo.contains_key(&game_id));
    total_tasks += 1;

    // Generate one expiration and one invalidation (both expiration merged)
    cache_manager.invalidate_with_properties(CACHE_ID, game_id, Game::new(game_id as u64, "Game test".to_string())).unwrap();
    total_tasks += 1;
    awaitility::at_most(Duration::from_millis(MAX_POLL_AWAIT))
        .poll_interval(Duration::from_millis(MAX_POLL_INTERVAL_AWAIT))
        .until(|| cache_manager_statistics.pending_tasks_total() == total_tasks - 1 && (cache_manager_statistics.tasks_total() == total_tasks
            || cache_manager_statistics.tasks_total() == total_tasks + 1));

    assert_eq!(1, cache_manager_statistics.merged_tasks_total())
}

#[tokio::test]
async fn should_manual_operations_fails_given_wrong_input() {
    let (_, cache_manager) = get_instances(&[(CACHE_ID, LOWER_EXP_TIME)]).await;

    // Expected key is i64
    let key: u64 = 1234;
    let valid_key: i64 = 1234;
    let invalid_value = String::from("invalid!");
    let valid_value = Game::new(valid_key as u64, "valid".to_string());

    assert_eq!(true, cache_manager.flush_all("not_valid_cache_id").is_err());
    assert_eq!(true, cache_manager.force_expiration(CACHE_ID, key).is_err());
    assert_eq!(true, cache_manager.invalidate_with_properties(CACHE_ID, valid_key, invalid_value).is_err());

    assert_eq!(true, cache_manager.invalidate_with_properties(CACHE_ID, valid_key, valid_value).is_ok());
}


#[tokio::test]
async fn should_bypass_cache_when_not_initialized() {
    let (instances, _) = get_instances_not_init(&[(CACHE_ID, LOWER_EXP_TIME)]).await;
    let game_repo = instances.get(0).unwrap();

    assert_eq!(true, game_repo.get(&STORED_GAME_ID).await.expect(EXPECT_GET_SUCCESSFULLY).is_some());
    assert_eq!(false, game_repo.contains_key(&STORED_GAME_ID));
    let game = Game::new(NOT_STORED_GAME_ID as u64, "Amazing Spiderman 3".to_string());
    assert_eq!(true, game_repo.put(&NOT_STORED_GAME_ID, &game).await.is_ok());
    assert_eq!(false, game_repo.contains_key(&NOT_STORED_GAME_ID));
}

#[tokio::test]
async fn should_await_max_time_poll_tasks_successfully() {
    let (_, cache_manager) = get_instances(&[(CACHE_ID, LOWER_EXP_TIME)]).await;
    let cache_manager_statistics = cache_manager.statistics();

    let now = Instant::now();
    awaitility::at_most(Duration::from_millis(MAX_POLL_AWAIT))
        .poll_interval(Duration::from_millis(MAX_PENDING_BULK_POLL_MS_AWAIT / 2))
        .until(|| {
            cache_manager.flush_all(CACHE_ID).expect("Send task successfully");
            cache_manager_statistics.tasks_total() != 0
        });

    assert_eq!(true, now.elapsed().as_millis() < (MAX_PENDING_MS_AWAIT + MAX_PENDING_BULK_POLL_MS_AWAIT) as u128)
}


#[tokio::test]
#[rstest]
#[case(4755777, "The Last of Us 2", "Amazing Spiderman 2", 10000)]
async fn should_invalidate_successfully(#[case] game_id: i64, #[case] old_title: &str, #[case] new_title: &str, #[case] exp_time: u64) {
    let (instances, cache_manager) = get_instances(&[(CACHE_ID, exp_time)]).await;
    let cache_manager_statistics = cache_manager.statistics();
    let game_repo = instances.get(0).unwrap();
    let db_pool = get_db_pool().await;

    let mut total_tasks = 0;

    let result = game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    assert_eq!(old_title, result.title);
    total_tasks += 1;

    query("UPDATE game.game SET name = $1 WHERE game_id = $2")
        .bind(new_title)
        .bind(game_id)
        .execute(&db_pool)
        .await.expect("Update successfully");

    cache_manager.invalidate(exp_time, CACHE_ID, game_id).unwrap();
    total_tasks += 1;

    async_await_until(MAX_POLL_INTERVAL_AWAIT, MAX_POLL_AWAIT, || {
        cache_manager_statistics.pending_async_tasks_total() == 0
            && (cache_manager_statistics.tasks_total() == total_tasks || cache_manager_statistics.tasks_total() == total_tasks + 1)
    }).await;

    assert_eq!(true, game_repo.contains_key(&game_id));

    let result = game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    assert_eq!(game_id as u64, result.id);
    assert_eq!(new_title, result.title);
}

#[tokio::test]
#[rstest]
#[case(4755771, "The Last of Us 3", "Amazing Spiderman 2", 10000, 1)]
async fn should_abort_operation_when_expires(#[case] game_id: i64, #[case] old_title: &str, #[case] new_title: &str, #[case] entry_expires_in: u64, #[case] op_expires_in: u64) {
    let (instances, cache_manager) = get_instances(&[(CACHE_ID, entry_expires_in)]).await;
    let cache_manager_statistics = cache_manager.statistics();
    let game_repo = instances.get(0).unwrap();
    let db_pool = get_db_pool().await;

    let result = game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    assert_eq!(old_title, result.title);

    query("UPDATE game.game SET name = $1 WHERE game_id = $2")
        .bind(new_title)
        .bind(game_id)
        .execute(&db_pool)
        .await.expect("Update successfully");

    cache_manager.invalidate(op_expires_in, CACHE_ID, game_id).unwrap();

    async_await_until(MAX_POLL_INTERVAL_AWAIT, MAX_POLL_AWAIT, || {
        cache_manager_statistics.pending_async_tasks_total() == 0 && cache_manager_statistics.expired_tasks_total() == 1
    }).await;

    assert_eq!(1, cache_manager_statistics.expired_tasks_total());

    let result = game_repo.get(&game_id).await.expect(EXPECT_GET_SUCCESSFULLY)
        .expect(EXPECT_ENTRY_FOUND);
    assert_eq!(game_id as u64, result.id);
    assert_eq!(old_title, result.title);
}




