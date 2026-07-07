use crate::platform::PlatformError;
use crate::types::Rect;
use std::time::Duration;

/// Request structure describing one or more regions that should be highlighted on screen.
#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct HighlightRequest {
    /// One or more bounding boxes in desktop coordinates.
    pub rects: Vec<Rect>,
    /// Optional duration that the highlight should stay visible before it
    /// disappears automatically.
    pub duration: Option<Duration>,
}

impl HighlightRequest {
    /// Create a request for a single rectangle.
    pub fn new(bounds: Rect) -> Self {
        Self { rects: vec![bounds], duration: None }
    }

    /// Create a request for multiple rectangles.
    pub fn from_rects(rects: Vec<Rect>) -> Self {
        Self { rects, duration: None }
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Iterate the rectangles in this request.
    pub fn rects(&self) -> impl Iterator<Item = &Rect> {
        self.rects.iter()
    }
}

/// Trait implemented by platform crates to render highlight overlays.
pub trait HighlightProvider: Send + Sync {
    /// Draws the given highlight regions. Providers decide whether the highlight
    /// persists until cleared or fades automatically.
    fn highlight(&self, request: &HighlightRequest) -> Result<(), PlatformError>;

    /// Clears any active highlight overlays.
    fn clear(&self) -> Result<(), PlatformError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Rect;

    #[test]
    fn highlight_request_builder_assigns_style() {
        let rect = Rect::new(0.0, 0.0, 1.0, 1.0);
        let duration = Duration::from_millis(750);
        let request = HighlightRequest::new(rect).with_duration(duration);
        assert_eq!(request.duration, Some(duration));
    }
}
