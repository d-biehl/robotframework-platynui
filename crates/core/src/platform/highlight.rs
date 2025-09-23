use crate::platform::PlatformError;
use crate::types::Rect;
use std::time::Duration;

/// Request structure describing a region that should be highlighted on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct HighlightRequest {
    /// Bounding box in desktop coordinates.
    pub bounds: Rect,
    /// Optional duration that the highlight should stay visible before it
    /// disappears automatically.
    pub duration: Option<Duration>,
}

impl HighlightRequest {
    pub fn new(bounds: Rect) -> Self {
        Self { bounds, duration: None }
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}

/// Trait implemented by platform crates to render highlight overlays.
pub trait HighlightProvider: Send + Sync {
    /// Draws the given highlight regions. Providers decide whether the highlight
    /// persists until cleared or fades automatically.
    fn highlight(&self, requests: &[HighlightRequest]) -> Result<(), PlatformError>;

    /// Clears any active highlight overlays.
    fn clear(&self) -> Result<(), PlatformError>;
}

pub struct HighlightRegistration {
    pub provider: &'static dyn HighlightProvider,
}

inventory::collect!(HighlightRegistration);

pub fn highlight_providers() -> impl Iterator<Item = &'static dyn HighlightProvider> {
    inventory::iter::<HighlightRegistration>.into_iter().map(|entry| entry.provider)
}

#[macro_export]
macro_rules! register_highlight_provider {
    ($provider:expr) => {
        inventory::submit! {
            $crate::platform::HighlightRegistration { provider: $provider }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{PlatformError, PlatformErrorKind};
    use crate::types::Rect;

    struct StubHighlightProvider;

    impl HighlightProvider for StubHighlightProvider {
        fn highlight(&self, _requests: &[HighlightRequest]) -> Result<(), PlatformError> {
            Ok(())
        }

        fn clear(&self) -> Result<(), PlatformError> {
            Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "clear not supported"))
        }
    }

    static PROVIDER: StubHighlightProvider = StubHighlightProvider;

    register_highlight_provider!(&PROVIDER);

    #[test]
    fn registration_exposes_provider() {
        let providers: Vec<_> = highlight_providers().collect();
        assert!(providers.iter().any(|provider| provider.highlight(&[]).is_ok()));
    }

    #[test]
    fn highlight_request_builder_assigns_style() {
        let rect = Rect::new(0.0, 0.0, 1.0, 1.0);
        let duration = Duration::from_millis(750);
        let request = HighlightRequest::new(rect).with_duration(duration);
        assert_eq!(request.duration, Some(duration));
    }
}
