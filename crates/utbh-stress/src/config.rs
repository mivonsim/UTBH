//! Parameter stress run.

/// Konfigurasi stress.
pub struct StressConfig {
    /// Iterasi per test (berulang, beban berkelanjutan).
    pub iterations: usize,
    /// Ukuran window untuk deteksi degradasi latensi.
    pub window: usize,
}

impl Default for StressConfig {
    fn default() -> Self {
        StressConfig {
            iterations: 100,
            window: 20,
        }
    }
}

impl StressConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.iterations == 0 {
            return Err("iterations harus > 0".into());
        }
        if self.window == 0 {
            return Err("window harus > 0".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_valid() {
        assert!(StressConfig::default().validate().is_ok());
    }

    #[test]
    fn rejects_zero() {
        assert!(StressConfig {
            iterations: 0,
            window: 10
        }
        .validate()
        .is_err());
        assert!(StressConfig {
            iterations: 10,
            window: 0
        }
        .validate()
        .is_err());
    }
}
