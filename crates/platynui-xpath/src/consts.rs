//! Centralized constants for namespace and URI strings and small shared tunables.
//! Keep crate-visible unless needed publicly via re-exports in specific modules.

// Common namespace URIs
pub const XML_URI: &str = "http://www.w3.org/XML/1998/namespace";
pub const FNS: &str = "http://www.w3.org/2005/xpath-functions";
pub const XS: &str = "http://www.w3.org/2001/XMLSchema";
pub const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
pub const ERR_NS: &str = "http://www.w3.org/2005/xqt-errors";

// Collation URIs
pub const CODEPOINT_URI: &str = "http://www.w3.org/2005/xpath-functions/collation/codepoint";
pub const SIMPLE_CASE_URI: &str = "urn:platynui:collation:simple-case";
pub const SIMPLE_ACCENT_URI: &str = "urn:platynui:collation:simple-accent";
pub const SIMPLE_CASE_ACCENT_URI: &str = "urn:platynui:collation:simple-case-accent";

// Display / debug tunables
pub(crate) const DISPLAY_CLIP_MAX: usize = 32;
