//! Util waktu: run id + timestamp unix.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn new_run_id() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // PID mencegah dua run paralel menimpa file hasil yang sama.
    format!(
        "{}-{:06}-{}",
        d.as_secs(),
        d.subsec_micros(),
        std::process::id()
    )
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_id_format() {
        let id = new_run_id();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 3, "format secs-micros-pid: {}", id);
        assert!(parts[0].chars().all(|c| c.is_ascii_digit()));
        assert_eq!(parts[1].len(), 6);
        assert_eq!(parts[2].parse::<u32>().unwrap(), std::process::id());
    }

    #[test]
    fn timestamps_monotonic_enough() {
        let a = now_unix();
        let b = now_unix();
        assert!(b >= a);
    }
}
