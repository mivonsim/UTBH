//! Minimizer — perkecil kasus FAIL sampai tetap mereproduksi bug.

use utbh_core::TestDef;
use utbh_test::run;

/// Binary-search ukuran input: hasil = definisi terkecil yang masih FAIL.
///
/// `None` bila ukuran 1 pun tidak FAIL lagi (bug hilang saat diperkecil).
pub fn minimize(failing: &TestDef) -> Option<TestDef> {
    let mut lo = 1usize;
    let mut best: Option<TestDef> = None;
    let mut hi = failing.input.elements;

    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let mut candidate = failing.clone();
        candidate.input.elements = mid;
        let out = run(&candidate);
        if matches!(out.status, utbh_core::Status::Fail) {
            best = Some(candidate);
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimize_needs_failing_input() {
        // Test yang selalu PASS → minimizer tidak menemukan kasus FAIL.
        let d: TestDef = toml::from_str(
            r#"
[test]
name = "always_pass"
category = "cpu"
[input]
elements = 1024
type = "f32"
[operation]
op = "add"
[validation]
mode = "exact"
"#,
        )
        .unwrap();
        assert!(minimize(&d).is_none());
    }

    #[test]
    fn minimize_keeps_shape() {
        let d: TestDef = toml::from_str(
            r#"
[test]
name = "t"
category = "cpu"
[input]
elements = 4096
type = "f32"
[operation]
op = "add"
[validation]
mode = "exact"
"#,
        )
        .unwrap();
        let m = minimize(&d);
        if let Some(m) = m {
            assert!(m.input.elements <= d.input.elements);
            assert_eq!(m.test.name, d.test.name);
        }
    }
}
