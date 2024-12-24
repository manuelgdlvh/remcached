use std::time::Duration;

use rstest::rstest;

use crate::utils::{get_instances_not_init, LOWER_EXP_TIME, NOT_STORED_GAME_ID, STORED_GAME_ID};
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
    let result = game_repo.get(&game_id).await.unwrap();
    assert_eq!(game_id as u64, result.id);
    assert_eq!(title, result.title);
    assert_eq!(1, game_repo.statistics().miss_total());

    // Cache hit
    assert_eq!(true, game_repo.contains_key(&game_id));
    let result = game_repo.get(&game_id).await.unwrap();
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
    assert_eq!(None, game_repo.get(&game_id).await);
    assert_eq!(1, game_repo.statistics().miss_total());

    // Add entry
    let game = Game::new(game_id as u64, title.to_string());
    assert_eq!(true, game_repo.put(&game_id, &game).await.is_ok());
    assert_eq!(true, game_repo.contains_key(&game_id));

    game_repo.get(&game_id).await.unwrap();
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
#[case(47557, 5000)]
#[case(47573, 7500)]
#[case(47577, 10000)]
async fn should_process_events_in_priority_order(#[case] game_id: i64, #[case] exp_time: u64) {
    let (instances, cache_manager) = get_instances(&[(CACHE_ID, exp_time), (CACHE_ID_2, LOWER_EXP_TIME)]).await;
    let cache_manager_statistics = cache_manager.statistics();
    let game_repo = instances.get(0).unwrap();
    let game_repo_2 = instances.get(1).unwrap();

    // Access the cache for the repository with the higher expiration time first
    let _ = game_repo.get(&game_id).await.unwrap();
    let _ = game_repo_2.get(&game_id).await.unwrap();

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
        game_repo.get(game_id).await;
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
async fn should_invalidate_successfully(#[case] game_id: i64, #[case] exp_time: u64) {
    let (instances, cache_manager) = get_instances(&[(CACHE_ID, exp_time)]).await;
    let cache_manager_statistics = cache_manager.statistics();
    let game_repo = instances.get(0).unwrap();

    let mut total_tasks = 0;
    game_repo.get(&game_id).await;
    assert_eq!(true, game_repo.contains_key(&game_id));
    total_tasks += 1;

    cache_manager.invalidate(CACHE_ID, game_id, Game::new(game_id as u64, "Game test".to_string())).unwrap();
    total_tasks += 1;
    awaitility::at_most(Duration::from_millis(MAX_POLL_AWAIT))
        .poll_interval(Duration::from_millis(MAX_POLL_INTERVAL_AWAIT))
        .until(|| cache_manager_statistics.pending_tasks_total() == total_tasks - 1 && (cache_manager_statistics.tasks_total() == total_tasks
            || cache_manager_statistics.tasks_total() == total_tasks + 1));

    let game = game_repo.get(&game_id).await.unwrap();
    assert_eq!(game_id as u64, game.id);
    assert_eq!("Game test", game.title);
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
    game_repo.get(&game_id).await;
    assert_eq!(true, game_repo.contains_key(&game_id));
    total_tasks += 1;

    // Generate one expiration and one invalidation (both expiration merged)
    cache_manager.invalidate(CACHE_ID, game_id, Game::new(game_id as u64, "Game test".to_string())).unwrap();
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
    assert_eq!(true, cache_manager.invalidate(CACHE_ID, valid_key, invalid_value).is_err());

    assert_eq!(true, cache_manager.invalidate(CACHE_ID, valid_key, valid_value).is_ok());
}


#[tokio::test]
async fn should_bypass_cache_when_not_initialized() {
    let (instances, _) = get_instances_not_init(&[(CACHE_ID, LOWER_EXP_TIME)]).await;
    let game_repo = instances.get(0).unwrap();

    assert_eq!(true, game_repo.get(&STORED_GAME_ID).await.is_some());
    assert_eq!(false, game_repo.contains_key(&STORED_GAME_ID));
    let game = Game::new(NOT_STORED_GAME_ID as u64, "Amazing Spiderman 3".to_string());
    assert_eq!(true, game_repo.put(&NOT_STORED_GAME_ID, &game).await.is_ok());
    assert_eq!(false, game_repo.contains_key(&NOT_STORED_GAME_ID));
}


