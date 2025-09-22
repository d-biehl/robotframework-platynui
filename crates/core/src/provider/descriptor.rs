use crate::ui::identifiers::TechnologyId;

/// Metadata describing a provider implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub technology: TechnologyId,
    pub kind: ProviderKind,
    pub priority: ProviderPriority,
}

impl ProviderDescriptor {
    pub fn new(
        id: &'static str,
        display_name: &'static str,
        technology: TechnologyId,
        kind: ProviderKind,
        priority: ProviderPriority,
    ) -> Self {
        Self { id, display_name, technology, kind, priority }
    }
}

/// Differentiates between native/in-process and external provider variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    Native,
    External,
}

impl Default for ProviderKind {
    fn default() -> Self {
        ProviderKind::Native
    }
}

/// Expresses the provider priority within a technology segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProviderPriority(pub u8);

impl ProviderPriority {
    pub const PRIMARY: ProviderPriority = ProviderPriority(0);
    pub const DEFAULT: ProviderPriority = ProviderPriority(50);
    pub const FALLBACK: ProviderPriority = ProviderPriority(100);
}

impl Default for ProviderPriority {
    fn default() -> Self {
        ProviderPriority::DEFAULT
    }
}
