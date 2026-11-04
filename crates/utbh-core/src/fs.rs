//! Penulisan file artefak yang aman terhadap proses berhenti di tengah jalan.

use std::path::Path;

/// Tulis `bytes` ke `path` secara atomik (tulis file sementara lalu rename).
///
/// Pembaca tidak akan pernah melihat JSON setengah jadi. File sementara
/// dibersihkan bila rename gagal.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("utbh-tmp");
    std::fs::write(&tmp, bytes)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_file_without_leaving_temp() {
        let dir = std::env::temp_dir().join(format!("utbh-fs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("a.utbh-result.json");
        write_atomic(&path, b"satu").unwrap();
        write_atomic(&path, b"dua").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"dua");
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            1,
            "tidak boleh ada file sementara tersisa"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
