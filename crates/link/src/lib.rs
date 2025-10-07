//! Helper macros for linking platform + provider crates in applications.
//!
//! This crate exports a single macro `platynui_link_os_providers!()` which, when
//! invoked inside an application crate, links the appropriate platform and
//! provider crates for the current target OS. The linking happens only for
//! non-test builds, so unit tests can link mock providers explicitly in their
//! own modules.

#[macro_export]
macro_rules! platynui_link_os_providers {
    () => {
        #[cfg(all(not(test), target_os = "windows"))]
        const _: () = {
            use platynui_platform_windows as _;
            use platynui_provider_windows_uia as _;
        };

        #[cfg(all(not(test), target_os = "linux"))]
        const _: () = {
            use platynui_platform_linux_x11 as _;
            use platynui_provider_atspi as _;
        };

        #[cfg(all(not(test), target_os = "macos"))]
        const _: () = {
            use platynui_platform_macos as _;
            use platynui_provider_macos_ax as _;
        };
    };
}

/// Links either mock providers (when the `mock-provider` feature is enabled on
/// the calling crate) or the real OS providers otherwise. Linking happens only
/// in non-test builds; unit tests should use `platynui_link_mock_for_tests!()`.
///
/// NOTE: Mock providers linked via this macro are NOT registered in the inventory.
/// They are available only through explicit factory usage.
#[macro_export]
macro_rules! platynui_link_providers {
    () => {
        #[cfg(all(not(test), feature = "mock-provider"))]
        const _: () = {
            use platynui_platform_mock as _;
            use platynui_provider_mock as _;
        };

        #[cfg(all(not(test), not(feature = "mock-provider")))]
        $crate::platynui_link_os_providers!();
    };
}

/// Links the mock platform + provider into the current test module/binary.
/// Intended to be called at the top of test modules to ensure mock providers
/// are available, though NOT registered in the inventory.
#[macro_export]
macro_rules! platynui_link_mock_for_tests {
    () => {
        #[cfg(test)]
        const _: () = {
            use platynui_platform_mock as _;
            use platynui_provider_mock as _;
        };
    };
}
