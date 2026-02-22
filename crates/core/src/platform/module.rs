use super::PlatformError;

pub trait PlatformModule: Send + Sync {
    fn name(&self) -> &'static str;
    fn initialize(&self) -> Result<(), PlatformError>;

    /// Allows the runtime to signal shutdown so the platform module can release
    /// resources (e.g. X11 connections, highlight threads, COM state).  The
    /// default implementation does nothing so modules without persistent
    /// resources can ignore this call.
    fn shutdown(&self) {}
}
