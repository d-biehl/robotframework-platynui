//! With the `log` feature, `log` records pass through the same filter as
//! `tracing` events. Its own test binary, because it installs the global
//! subscriber.

use std::io::Write;
use std::sync::{Arc, Mutex};

use tracing as _;
use tracing_subscriber as _;

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn log_records_follow_the_platynui_filter() {
    let buffer = Buffer::default();
    let writer = buffer.clone();
    platynui_log_filter::init_with_writer(Some(platynui_log_filter::LevelFilter::DEBUG), move || writer.clone());

    log::debug!(target: "platynui_inspector::tree", "ours through log");
    log::debug!(target: "eframe", "theirs through log");
    log::warn!(target: "eframe", "their warning through log");

    let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
    assert!(output.contains("ours through log"), "{output}");
    assert!(!output.contains("theirs through log"), "{output}");
    assert!(output.contains("their warning through log"), "{output}");
}
