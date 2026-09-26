//! Tipe data trace.

use serde::{Deserialize, Serialize};

/// Satu event dalam trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub cycle: u64,
    pub kind: String,
    pub detail: String,
}

/// Trace lengkap satu test yang FAIL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub run_id: String,
    pub test: String,
    pub category: String,
    pub events: Vec<TraceEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_event_serializes() {
        let e = TraceEvent { cycle: 42, kind: "instruction".into(), detail: "add".into() };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"cycle\":42"));
    }
}
