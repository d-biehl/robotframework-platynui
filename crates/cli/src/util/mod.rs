use platynui_core::platform::PlatformError;
use platynui_core::provider::ProviderError;
use platynui_core::ui::Namespace;
use platynui_runtime::EvaluateError;
use std::collections::HashSet;
use std::error::Error;
use std::str::FromStr;

pub type CliResult<T> = Result<T, Box<dyn Error>>;

pub fn map_provider_error(err: ProviderError) -> Box<dyn Error> {
    Box::new(err)
}

pub fn map_evaluate_error(err: EvaluateError) -> Box<dyn Error> {
    Box::new(err)
}

pub fn map_platform_error(err: PlatformError) -> Box<dyn Error> {
    Box::new(err)
}

pub fn parse_namespace_filters(values: &[String]) -> CliResult<Option<HashSet<Namespace>>> {
    if values.is_empty() {
        return Ok(None);
    }

    let mut filters = HashSet::new();
    for value in values {
        let namespace =
            Namespace::from_str(value).map_err(|_| format!("unknown namespace prefix: {value}"))?;
        filters.insert(namespace);
    }
    Ok(Some(filters))
}

pub fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
