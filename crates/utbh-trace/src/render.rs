//! Render trace sebagai teks untuk `utbh trace <run-id>`.

use crate::event::Trace;

pub fn render(trace: &Trace) -> String {
    let mut out = format!(
        "Trace {} — {} ({})\n",
        trace.run_id, trace.test, trace.category
    );
    for e in &trace.events {
        out.push_str(&format!("{:>12}  {:<16} {}\n", e.cycle, e.kind, e.detail));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::TraceEvent;

    #[test]
    fn render_lists_events() {
        let t = Trace {
            run_id: "r".into(),
            test: "t".into(),
            category: "cpu".into(),
            events: vec![TraceEvent {
                cycle: 7,
                kind: "validation".into(),
                detail: "x".into(),
            }],
        };
        let s = render(&t);
        assert!(s.contains("validation"));
        assert!(s.contains("Trace r"));
    }
}
