//! Captured log output for unit tests: a scoped `fmt` subscriber at debug,
//! one line per record, without colours or timestamps.

use std::sync::{Arc, Mutex};

/// Runs `f` and returns what it logged, one line per record.
pub(crate) fn logged<R>(f: impl FnOnce() -> R) -> (R, String) {
    #[derive(Clone)]
    struct Captured(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Captured {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("log buffer").extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = Captured(Arc::clone(&buffer));
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .without_time()
        .with_writer(move || writer.clone())
        .finish();
    let result = tracing::subscriber::with_default(subscriber, f);
    let log = String::from_utf8(buffer.lock().expect("log buffer").clone()).expect("utf-8 log");
    (result, log)
}
