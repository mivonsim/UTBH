//! Render `utbh list` — daftar suite + test tanpa eksekusi.

use std::path::Path;

/// Format satu baris: `suite/category` + nama test + jumlah elemen.
pub fn render(suites_root: &Path) -> Result<String, String> {
    let suites = utbh_core::list_suites(suites_root);
    if suites.is_empty() {
        return Err(format!("tidak ada suite di {}", suites_root.display()));
    }

    let mut out = format!("UTBH Test Packages — {}\n", suites_root.display());
    for suite in &suites {
        let defs = match utbh_core::load_suite(suites_root, suite) {
            Ok(d) => d,
            Err(e) => {
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
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_repo_suites() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../suites");
        if !root.is_dir() {
            return;
        }
        let text = render(&root).unwrap();
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
}
