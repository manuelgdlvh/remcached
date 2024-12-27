use std::time::{SystemTime, UNIX_EPOCH};

pub fn calculate_time_until_next_ms(time_ms: u64) -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .expect("Time went backwards").as_millis() + time_ms as u128
}

pub fn is_now_after(target_time: u128) -> bool {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .expect("Time went backwards").as_millis() >= target_time
}