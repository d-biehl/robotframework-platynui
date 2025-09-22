use crate::ui::identifiers::TechnologyId;

/// Metadata describing a provider implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub technology: TechnologyId,
    pub kind: ProviderKind,
}

impl ProviderDescriptor {
    pub fn new(
        id: &'static str,
        display_name: &'static str,
        technology: TechnologyId,
        kind: ProviderKind,
    ) -> Self {
        Self { id, display_name, technology, kind }
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
