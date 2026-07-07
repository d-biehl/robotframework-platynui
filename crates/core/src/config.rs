//! Runtime session configuration.
//!
//! [`RuntimeConfig`] is the parsed form of the construction-time `config`
//! dictionary a runtime is built with (see the `runtime-session-config`
//! capability). It carries two open, id-keyed sections — `platform` and
//! `providers` — and each platform/provider factory reads only its own slice.
//!
//! The type is deliberately schema-less. Unknown ids and keys are retained but
//! never required, so a single dictionary can carry settings for every
//! operating system and a component simply ignores what it does not recognise
//! (the runtime debug-logs the leftovers). An absent or empty config resolves
//! to today's behaviour: every backend falls back to the environment.

use std::collections::BTreeMap;

/// Reserved key inside the `platform` section that forces a specific backend id
/// (e.g. `platform.backend = "x11"`), overriding environment auto-detection.
pub const PLATFORM_BACKEND_KEY: &str = "backend";

/// A single configuration value inside a [`ConfigMap`].
///
/// The variant set is intentionally small — connection settings are almost
/// always scalars (a display name, a bus address, a colour, a flag). `Map` and
/// `List` keep the representation open for nested settings without a schema.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    List(Vec<ConfigValue>),
    Map(ConfigMap),
}

impl From<&str> for ConfigValue {
    fn from(value: &str) -> Self {
        ConfigValue::Str(value.to_owned())
    }
}

impl From<String> for ConfigValue {
    fn from(value: String) -> Self {
        ConfigValue::Str(value)
    }
}

impl From<i64> for ConfigValue {
    fn from(value: i64) -> Self {
        ConfigValue::Int(value)
    }
}

impl From<f64> for ConfigValue {
    fn from(value: f64) -> Self {
        ConfigValue::Float(value)
    }
}

impl From<bool> for ConfigValue {
    fn from(value: bool) -> Self {
        ConfigValue::Bool(value)
    }
}

impl From<ConfigMap> for ConfigValue {
    fn from(value: ConfigMap) -> Self {
        ConfigValue::Map(value)
    }
}

/// An open string-keyed map of [`ConfigValue`]s — one component's settings, or
/// a whole config section.
///
/// The typed accessors return `None` both when a key is absent and when it
/// holds the wrong type, so a mistyped value degrades to the fallback rather
/// than erroring (matching the tolerant-resolution rule).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigMap(BTreeMap<String, ConfigValue>);

impl ConfigMap {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Insert a value, returning `&mut self` for fluent building.
    pub fn with(mut self, key: impl Into<String>, value: impl Into<ConfigValue>) -> Self {
        self.0.insert(key.into(), value.into());
        self
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<ConfigValue>) {
        self.0.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.0.get(key)
    }

    /// The value at `key` if it is a string.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.0.get(key) {
            Some(ConfigValue::Str(value)) => Some(value.as_str()),
            _ => None,
        }
    }

    /// The value at `key` if it is a boolean.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.0.get(key) {
            Some(ConfigValue::Bool(value)) => Some(*value),
            _ => None,
        }
    }

    /// The value at `key` if it is an integer.
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        match self.0.get(key) {
            Some(ConfigValue::Int(value)) => Some(*value),
            _ => None,
        }
    }

    /// The value at `key` if it is a float.
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        match self.0.get(key) {
            Some(ConfigValue::Float(value)) => Some(*value),
            _ => None,
        }
    }

    /// The value at `key` if it is a nested map.
    pub fn get_map(&self, key: &str) -> Option<&ConfigMap> {
        match self.0.get(key) {
            Some(ConfigValue::Map(value)) => Some(value),
            _ => None,
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl FromIterator<(String, ConfigValue)> for ConfigMap {
    fn from_iter<I: IntoIterator<Item = (String, ConfigValue)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// The parsed construction-time session configuration for one runtime.
///
/// Holds the two open sections; a factory reads its own slice by id via
/// [`platform`](Self::platform) / [`provider`](Self::provider). The default is
/// empty — equivalent to passing no config at all.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeConfig {
    platform: ConfigMap,
    providers: ConfigMap,
}

impl RuntimeConfig {
    pub fn new(platform: ConfigMap, providers: ConfigMap) -> Self {
        Self { platform, providers }
    }

    /// The forced platform backend id (`platform.backend`), if set. When absent
    /// the runtime auto-detects the backend from the environment.
    pub fn platform_backend(&self) -> Option<&str> {
        self.platform.get_str(PLATFORM_BACKEND_KEY)
    }

    /// The settings sub-map for platform backend `id` (`platform.<id>`).
    pub fn platform(&self, id: &str) -> Option<&ConfigMap> {
        self.platform.get_map(id)
    }

    /// The settings sub-map for provider `id` (`providers.<id>`).
    pub fn provider(&self, id: &str) -> Option<&ConfigMap> {
        self.providers.get_map(id)
    }

    /// Ids present under `platform`, excluding the reserved `backend` selector.
    /// Used by the runtime to debug-log ids no registered backend claimed.
    pub fn platform_component_ids(&self) -> impl Iterator<Item = &str> {
        self.platform.keys().filter(|key| *key != PLATFORM_BACKEND_KEY)
    }

    /// Ids present under `providers`.
    pub fn provider_component_ids(&self) -> impl Iterator<Item = &str> {
        self.providers.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RuntimeConfig {
        let platform = ConfigMap::new()
            .with(PLATFORM_BACKEND_KEY, "x11")
            .with("x11", ConfigMap::new().with("display", ":1"))
            .with("windows", ConfigMap::new().with("highlight_color", "#ff0000"));
        let providers = ConfigMap::new().with("atspi", ConfigMap::new().with("bus_address", "unix:path=/run/bus"));
        RuntimeConfig::new(platform, providers)
    }

    #[test]
    fn empty_config_yields_nothing() {
        let config = RuntimeConfig::default();
        assert_eq!(config.platform_backend(), None);
        assert!(config.platform("x11").is_none());
        assert!(config.provider("atspi").is_none());
        assert_eq!(config.platform_component_ids().count(), 0);
        assert_eq!(config.provider_component_ids().count(), 0);
    }

    #[test]
    fn reads_backend_selector() {
        assert_eq!(sample().platform_backend(), Some("x11"));
    }

    #[test]
    fn component_reads_only_its_own_slice() {
        let config = sample();
        assert_eq!(config.platform("x11").and_then(|m| m.get_str("display")), Some(":1"));
        assert_eq!(config.provider("atspi").and_then(|m| m.get_str("bus_address")), Some("unix:path=/run/bus"));
        // The x11 slice does not see the provider's or another platform's keys.
        assert!(config.platform("x11").and_then(|m| m.get_str("bus_address")).is_none());
        assert!(config.platform("x11").and_then(|m| m.get_str("highlight_color")).is_none());
    }

    #[test]
    fn type_mismatch_and_missing_keys_return_none() {
        let map = ConfigMap::new().with("display", ":1").with("count", 3_i64).with("enabled", true);
        // Present but wrong type → None (degrade to fallback, never error).
        assert_eq!(map.get_bool("display"), None);
        assert_eq!(map.get_str("count"), None);
        assert_eq!(map.get_i64("enabled"), None);
        // Absent → None.
        assert_eq!(map.get_str("missing"), None);
        // Correct type → value.
        assert_eq!(map.get_str("display"), Some(":1"));
        assert_eq!(map.get_i64("count"), Some(3));
        assert_eq!(map.get_bool("enabled"), Some(true));
    }

    #[test]
    fn component_ids_exclude_backend_selector() {
        let config = sample();
        let ids: Vec<_> = config.platform_component_ids().collect();
        assert!(ids.contains(&"x11"));
        assert!(ids.contains(&"windows"));
        assert!(!ids.contains(&PLATFORM_BACKEND_KEY));
        let provider_ids: Vec<_> = config.provider_component_ids().collect();
        assert_eq!(provider_ids, vec!["atspi"]);
    }

    #[test]
    fn foreign_os_block_is_retained_but_unclaimed() {
        // A dict resolved on X11 still carries the windows block; it is simply
        // never read by a matching component and shows up as an id the runtime
        // can debug-log as unclaimed.
        let config = sample();
        assert!(config.platform("windows").is_some());
        let ids: Vec<_> = config.platform_component_ids().collect();
        assert!(ids.contains(&"windows"));
    }
}
