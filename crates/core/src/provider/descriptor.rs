use crate::ui::identifiers::TechnologyId;
use bitflags::bitflags;

/// Metadata describing a provider implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub technology: TechnologyId,
    pub kind: ProviderKind,
    pub event_capabilities: ProviderEventCapabilities,
}

impl ProviderDescriptor {
    pub fn new(id: &'static str, display_name: &'static str, technology: TechnologyId, kind: ProviderKind) -> Self {
        Self { id, display_name, technology, kind, event_capabilities: ProviderEventCapabilities::NONE }
    }

    pub const fn with_event_capabilities(mut self, capabilities: ProviderEventCapabilities) -> Self {
        self.event_capabilities = capabilities;
        self
    }

    pub const fn event_capabilities(&self) -> ProviderEventCapabilities {
        self.event_capabilities
    }
}

/// Differentiates between native/in-process and external provider variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProviderKind {
    #[default]
    Native,
    External,
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub struct ProviderEventCapabilities: u8 {
        const NONE = 0;
        const CHANGE_HINT = 0b0001;
        const STRUCTURE = 0b0010;
        const STRUCTURE_WITH_PROPERTIES = 0b0100;
    }
}

impl ProviderEventCapabilities {
    pub fn supports_change_hint(self) -> bool {
        self.intersects(Self::CHANGE_HINT | Self::STRUCTURE | Self::STRUCTURE_WITH_PROPERTIES)
    }

    pub fn supports_structure(self) -> bool {
        self.intersects(Self::STRUCTURE | Self::STRUCTURE_WITH_PROPERTIES)
    }

    pub fn supports_property_changes(self) -> bool {
        self.contains(Self::STRUCTURE_WITH_PROPERTIES)
    }
}
