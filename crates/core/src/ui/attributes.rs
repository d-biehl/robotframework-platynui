/// Canonical attribute keys according to `docs/patterns.md` (PascalCase).
pub mod names {
    pub const BOUNDS: &str = "Bounds";
    pub const ROLE: &str = "Role";
    pub const NAME: &str = "Name";
    pub const IS_VISIBLE: &str = "IsVisible";
    pub const IS_OFFSCREEN: &str = "IsOffscreen";
    pub const RUNTIME_ID: &str = "RuntimeId";
    pub const TECHNOLOGY: &str = "Technology";
    pub const SUPPORTED_PATTERNS: &str = "SupportedPatterns";
    pub const OS_NAME: &str = "OsName";
    pub const OS_VERSION: &str = "OsVersion";
    pub const DISPLAY_COUNT: &str = "DisplayCount";
    pub const MONITORS: &str = "Monitors";
}

/// Convenience list of every mandatory attribute.
pub const REQUIRED: &[&str] = &[
    names::BOUNDS,
    names::ROLE,
    names::NAME,
    names::IS_VISIBLE,
    names::RUNTIME_ID,
    names::TECHNOLOGY,
    names::SUPPORTED_PATTERNS,
];

/// Optional attributes defined in the core contract.
pub const OPTIONAL: &[&str] = &[
    names::IS_OFFSCREEN,
    names::OS_NAME,
    names::OS_VERSION,
    names::DISPLAY_COUNT,
    names::MONITORS,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_attributes_have_expected_length() {
        assert_eq!(REQUIRED.len(), 7);
    }
}
