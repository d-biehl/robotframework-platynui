fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" {
        // The `xkbcommon` crate uses `#[link(name = "xkbcommon")]` without its
        // own build script, so the linker must find libxkbcommon.so in one of
        // its search paths. Cross-compiling toolchains (e.g. cargo-zigbuild)
        // do not include standard system library paths, causing the link to
        // fail.  Use pkg-config to discover the library and emit the search
        // path so the linker can find it regardless of the toolchain used.
        if let Ok(lib) = pkg_config::probe_library("xkbcommon") {
            for path in &lib.link_paths {
                println!("cargo:rustc-link-search=native={}", path.display());
            }
        }
    }
}
