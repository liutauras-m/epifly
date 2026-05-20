#[allow(unused_imports)]
use base64::Engine as _; // needed once capture_image() is enabled (Tauri >= 2.2)
use common::trace::{SessionRecorder, SessionTrace, UserStep};
use std::sync::{Arc, Mutex};
use ulid::Ulid;

use crate::telemetry;

pub struct Recorder {
    trace_id: String,
    steps: Vec<UserStep>,
    started_at: chrono::DateTime<chrono::Utc>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            trace_id: Ulid::new().to_string(),
            steps: Vec::new(),
            started_at: chrono::Utc::now(),
        }
    }
}

impl SessionRecorder for Recorder {
    fn record_step(&self, step: UserStep) {
        // Safety: interior-mutability via outer Mutex in RecorderState
        drop(step);
        unreachable!("use RecorderState::record_step instead");
    }

    fn snapshot(&self) -> SessionTrace {
        let urls: Vec<String> = self
            .steps
            .iter()
            .map(|s| s.url.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        SessionTrace {
            id: self.trace_id.clone(),
            started_at: self.started_at,
            ended_at: None,
            steps: self.steps.clone(),
            urls,
        }
    }

    fn reset(&self) {
        unreachable!("use RecorderState::reset instead");
    }
}

pub struct RecorderState {
    inner: Option<Recorder>,
}

impl RecorderState {
    pub fn new() -> Self {
        Self { inner: None }
    }

    pub fn start(&mut self) {
        self.inner = Some(Recorder::new());
    }

    pub fn record_step(&mut self, mut step: UserStep) {
        let Some(rec) = &mut self.inner else { return };
        step.seq = rec.steps.len();
        rec.steps.push(redact_pii(step));
    }

    pub fn stop(&mut self) -> Option<SessionTrace> {
        let rec = self.inner.take()?;
        let mut trace = rec.snapshot();
        trace.ended_at = Some(chrono::Utc::now());
        Some(trace)
    }

    pub fn is_recording(&self) -> bool {
        self.inner.is_some()
    }

    pub fn step_count(&self) -> usize {
        self.inner.as_ref().map(|r| r.steps.len()).unwrap_or(0)
    }
}

pub type RecorderStateHandle = Arc<Mutex<RecorderState>>;

/// Hard-coded PII filter — not bypassable by config or CLI.
fn redact_pii(mut step: UserStep) -> UserStep {
    // Wipe value if the selector hints at a password or sensitive field.
    if let Some(ref sel) = step.selector {
        let sel_lower = sel.to_lowercase();
        let is_sensitive = sel_lower.contains("password")
            || sel_lower.contains("secret")
            || sel_lower.contains("token")
            || sel_lower.contains("ssn")
            || sel_lower.contains("social")
            || sel_lower.contains("cpf")
            || sel_lower.contains("cc-")
            || sel_lower.contains("type=\"password\"");
        if is_sensitive {
            step.value = None;
            step.screenshot_base64 = None;
        }
    }
    step
}

// Tauri commands

#[tauri::command]
pub async fn recorder_start(state: tauri::State<'_, RecorderStateHandle>) -> Result<(), String> {
    state.lock().unwrap().start();
    telemetry::emit_span("recorder.start", &[]).await;
    Ok(())
}

#[tauri::command]
pub fn recorder_record_step(state: tauri::State<RecorderStateHandle>, step: UserStep) {
    state.lock().unwrap().record_step(step);
}

#[tauri::command]
pub async fn recorder_stop(
    state: tauri::State<'_, RecorderStateHandle>,
) -> Result<Option<SessionTrace>, String> {
    let result = state.lock().unwrap().stop();
    telemetry::emit_span("recorder.stop", &[]).await;
    Ok(result)
}

#[tauri::command]
pub fn recorder_status(state: tauri::State<RecorderStateHandle>) -> serde_json::Value {
    let guard = state.lock().unwrap();
    serde_json::json!({
        "recording": guard.is_recording(),
        "step_count": guard.step_count(),
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use common::trace::StepKind;

    fn make_step(selector: &str, value: &str) -> UserStep {
        UserStep {
            seq: 0,
            kind: StepKind::Input,
            selector: Some(selector.to_owned()),
            value: Some(value.to_owned()),
            url: "https://example.com".to_owned(),
            timestamp_ms: 0,
            screenshot_base64: Some("abc".to_owned()),
        }
    }

    #[test]
    fn redacts_password_selector() {
        let step = redact_pii(make_step("#password", "s3cr3t"));
        assert!(step.value.is_none());
        assert!(step.screenshot_base64.is_none());
    }

    #[test]
    fn redacts_ssn_selector() {
        let step = redact_pii(make_step("#ssn-field", "123-45-6789"));
        assert!(step.value.is_none());
    }

    #[test]
    fn redacts_cc_selector() {
        let step = redact_pii(make_step("#cc-number", "4111111111111111"));
        assert!(step.value.is_none());
    }

    #[test]
    fn keeps_safe_selector_value() {
        let step = redact_pii(make_step("#search-query", "hello world"));
        assert_eq!(step.value.as_deref(), Some("hello world"));
        assert!(step.screenshot_base64.is_some());
    }

    #[test]
    fn recorder_state_lifecycle() {
        let mut state = RecorderState::new();
        assert!(!state.is_recording());
        assert_eq!(state.step_count(), 0);

        state.start();
        assert!(state.is_recording());

        state.record_step(make_step("#email", "user@example.com"));
        state.record_step(make_step("#password", "secret"));
        assert_eq!(state.step_count(), 2);

        let trace = state.stop().expect("trace should be returned");
        assert_eq!(trace.steps.len(), 2);
        // password step should be redacted
        assert!(trace.steps[1].value.is_none());
        // email step should be intact
        assert!(trace.steps[0].value.is_some());
        assert!(!state.is_recording());
    }
}
