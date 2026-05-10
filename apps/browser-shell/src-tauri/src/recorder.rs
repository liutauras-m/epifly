#[allow(unused_imports)]
use base64::Engine as _; // needed once capture_image() is enabled (Tauri >= 2.2)
use common::trace::{SessionRecorder, SessionTrace, UserStep};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
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

/// Capture a screenshot of the webview window for `tab-{tab_id}` and return it
/// as a base64-encoded PNG string.
///
/// NOTE: `WebviewWindow::capture_image()` is only available in Tauri ≥ 2.2.
/// Until the dependency is upgraded this command returns an error. The PNG
/// encoder and base64 helpers below are ready for when it becomes available.
#[tauri::command]
pub async fn capture_tab_screenshot(app: AppHandle, tab_id: String) -> Result<String, String> {
    let label = format!("tab-{tab_id}");
    let _win = app
        .get_webview_window(&label)
        .ok_or_else(|| format!("no window with label {label}"))?;

    // `capture_image()` is not yet available in tauri 2.11.x.
    // Once upgraded, replace this block with:
    //   let img = _win.capture_image().map_err(|e| e.to_string())?;
    //   let png_bytes = encode_png_uncompressed(img.rgba(), img.width(), img.height())
    //       .map_err(|e| format!("PNG encode error: {e}"))?;
    //   return Ok(base64::engine::general_purpose::STANDARD.encode(&png_bytes));
    Err("capture_image requires Tauri >= 2.2; upgrade the dependency".to_owned())
}

// ---------------------------------------------------------------------------
// Minimal uncompressed PNG encoder (no additional crates required).
//
// Uses zlib non-compressed blocks (BFINAL=1, BTYPE=00) and PNG filter type 0
// (None) on every row. Output is a valid PNG readable by all viewers.
// Ready for use once `WebviewWindow::capture_image()` is available (Tauri ≥ 2.2).
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn encode_png_uncompressed(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, &'static str> {
    // Build raw filtered image data (filter byte 0x00 before each row).
    let stride = (width * 4) as usize;
    let mut raw: Vec<u8> = Vec::with_capacity((stride + 1) * height as usize);
    for row in 0..height as usize {
        raw.push(0x00); // filter type None
        raw.extend_from_slice(&rgba[row * stride..(row + 1) * stride]);
    }

    // Wrap `raw` in a zlib non-compressed stream.
    let idat_data = zlib_no_compress(&raw);

    let mut out: Vec<u8> = Vec::new();

    // PNG signature
    out.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

    // IHDR chunk
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type: RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_chunk(&mut out, b"IHDR", &ihdr);

    // IDAT chunk
    write_chunk(&mut out, b"IDAT", &idat_data);

    // IEND chunk
    write_chunk(&mut out, b"IEND", &[]);

    Ok(out)
}

#[allow(dead_code)]
fn write_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    let len = data.len() as u32;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let mut crc = crc32_init();
    crc = crc32_update(crc, tag);
    crc = crc32_update(crc, data);
    out.extend_from_slice(&crc32_finalize(crc).to_be_bytes());
}

/// Produces a valid zlib stream with a single non-compressed deflate block.
#[allow(dead_code)]
fn zlib_no_compress(data: &[u8]) -> Vec<u8> {
    // zlib header: CMF=0x78 (deflate, window 32KB), FLG chosen so CMF*256+FLG % 31 == 0.
    // 0x78 * 256 = 0x7800 = 30720; 30720 % 31 = 30720 - 991*31 = 30720-30721 ... let's calc:
    // 30720 / 31 = 991.0, 991*31=30721, so we need FLG = 31 - (30720 % 31) ... 30720 % 31:
    // 31*990=30690, 30720-30690=30, so FLG must be (31-30)%31 = 1 → but FLG=1 → bits[5:0]=1
    // and FCHECK ensures (0x78*256+FLG)%31==0: (30720+1)%31=30721%31=0 ✓
    let cmf: u8 = 0x78;
    let flg: u8 = 0x01;

    let mut out = Vec::new();
    out.push(cmf);
    out.push(flg);

    // Deflate non-compressed blocks; max block payload = 65535 bytes.
    let max_block: usize = 65535;
    let mut pos = 0;
    while pos < data.len() || data.is_empty() {
        let end = (pos + max_block).min(data.len());
        let is_last = end == data.len();
        let block_data = &data[pos..end];
        let blen = block_data.len() as u16;
        let blen_c = !blen;
        out.push(if is_last { 0x01 } else { 0x00 }); // BFINAL | BTYPE=00
        out.extend_from_slice(&blen.to_le_bytes());
        out.extend_from_slice(&blen_c.to_le_bytes());
        out.extend_from_slice(block_data);
        pos = end;
        if is_last || data.is_empty() {
            break;
        }
    }

    // Adler-32 checksum (big-endian).
    let adler = adler32(data);
    out.extend_from_slice(&adler.to_be_bytes());

    out
}

#[allow(dead_code)]
fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;
    for &b in data {
        s1 = (s1 + b as u32) % MOD;
        s2 = (s2 + s1) % MOD;
    }
    (s2 << 16) | s1
}

// CRC-32 for PNG chunks (IEEE 802.3 polynomial 0xEDB88320).
#[allow(dead_code)]
fn crc32_init() -> u32 {
    0xFFFF_FFFF
}

#[allow(dead_code)]
fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    crc
}

#[allow(dead_code)]
fn crc32_finalize(crc: u32) -> u32 {
    crc ^ 0xFFFF_FFFF
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
