use super::PlatformError;

pub trait PlatformModule: Send + Sync {
    fn name(&self) -> &'static str;
    fn initialize(&self) -> Result<(), PlatformError>;
}
