//! Util waktu: run id + timestamp unix.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn new_run_id() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}-{:06}", d.as_secs(), d.subsec_micros())
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
        let (sec, micro) = id.split_once('-').expect("format secs-micros");
        assert!(sec.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(micro.len(), 6);
    }

    #[test]
    fn timestamps_monotonic_enough() {
        let a = now_unix();
        let b = now_unix();
        assert!(b >= a);
    }
}
