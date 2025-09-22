use super::PlatformModule;
use inventory::collect;

pub struct PlatformRegistration {
    pub module: &'static dyn PlatformModule,
}

collect!(PlatformRegistration);

pub fn platform_modules() -> impl Iterator<Item = &'static dyn PlatformModule> {
    inventory::iter::<PlatformRegistration>.into_iter().map(|entry| entry.module)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{PlatformError, register_platform_module};

    struct DummyPlatform;

    impl PlatformModule for DummyPlatform {
        fn name(&self) -> &'static str {
            "dummy-platform"
        }

        fn initialize(&self) -> Result<(), PlatformError> {
            Ok(())
        }
    }

    static MODULE: DummyPlatform = DummyPlatform;

    register_platform_module!(&MODULE);

    #[test]
    fn registered_platform_is_exposed() {
        let modules: Vec<_> = platform_modules().collect();
        assert!(modules.iter().any(|module| module.name() == "dummy-platform"));
    }
}
