//! Render `utbh list` — daftar suite + test tanpa eksekusi.

use std::path::Path;

/// Hasil listing: teks untuk stdout + apakah ada suite yang gagal dimuat.
pub struct Listing {
    pub text: String,
    pub had_errors: bool,
}

/// Format satu baris: `suite/category` + nama test + jumlah elemen.
pub fn render(suites_root: &Path) -> Result<Listing, String> {
    let suites = utbh_core::list_suites(suites_root);
    if suites.is_empty() {
        return Err(format!("tidak ada suite di {}", suites_root.display()));
    }

    let mut out = format!("UTBH Test Packages — {}\n", suites_root.display());
    let mut had_errors = false;
    for suite in &suites {
        let defs = match utbh_core::load_suite(suites_root, suite) {
            Ok(d) => d,
            Err(e) => {
                had_errors = true;
                out.push_str(&format!("\n{}/  (error: {})\n", suite, e));
                continue;
            }
        };
        out.push_str(&format!("\n{}/  — {} test\n", suite, defs.len()));
        for d in &defs {
            out.push_str(&format!(
                "  {:<28} {:<22} op={:<14} elements={:<9} mode={}\n",
                d.test.name, d.test.category, d.operation.op, d.input.elements, d.validation.mode
            ));
        }
    }
    out.push_str("\nTambah test = tambah file *.utbh (tanpa rebuild).\n");
    Ok(Listing {
        text: out,
        had_errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
[test]
name = "ok_test"
category = "cpu"
[input]
elements = 8
type = "f32"
[operation]
op = "add"
[validation]
mode = "exact"
"#;

    fn temp_root(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("utbh-list-{}-{}", std::process::id(), label));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lists_repo_suites() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../suites");
        if !root.is_dir() {
            return;
        }
        let listing = render(&root).unwrap();
        assert!(!listing.had_errors, "suite repo harus valid");
        let text = listing.text;
        assert!(text.contains("cpu/"), "suite cpu harus ada:\n{}", text);
        assert!(
            text.contains("atomic_add_race"),
            "test baru harus terdaftar"
        );
        assert!(text.contains("Tambah test"));
    }

    #[test]
    fn missing_root_errors() {
        assert!(render(Path::new("/no/such/suites")).is_err());
    }

    #[test]
    fn broken_suite_sets_error_flag_but_still_lists_others() {
        let root = temp_root("broken");
        std::fs::create_dir_all(root.join("cpu")).unwrap();
        std::fs::create_dir_all(root.join("gpu")).unwrap();
        std::fs::write(root.join("cpu/ok.utbh"), VALID).unwrap();
        std::fs::write(root.join("gpu/bad.utbh"), "ini bukan toml [").unwrap();

        let listing = render(&root).unwrap();
        assert!(listing.had_errors, "suite rusak harus dilaporkan");
        assert!(listing.text.contains("ok_test"));
        assert!(listing.text.contains("gpu/"));
        assert!(listing.text.contains("error:"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
