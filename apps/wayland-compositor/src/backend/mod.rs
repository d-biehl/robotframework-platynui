//! Backend abstraction — headless, winit, and DRM backends.
//!
//! Each backend module provides a `run()` function that:
//! 1. Creates the appropriate renderer and output
//! 2. Sets up the Wayland display and socket
//! 3. Runs the event loop
//! 4. Handles rendering and frame callbacks

pub mod drm;
pub mod headless;
pub mod winit;

/// Create a `GlowRenderer` that shares the GL object namespace with an existing renderer.
///
/// Uses [`EGLContext::new_shared`] to derive a new EGL context that shares
/// textures, programs, sync objects, and other GL resources with the parent.
/// Smithay automatically propagates the parent's [`ContextId`] via shared
/// `user_data`, so texture-compatibility checks pass without extra work.
///
/// This is the preferred way to create a screenshot renderer when a main
/// renderer already exists (winit or DRM backends).
///
/// # Errors
///
/// Returns an error if EGL context sharing or GL initialization fails.
pub fn create_shared_glow_renderer(
    parent: &smithay::backend::renderer::glow::GlowRenderer,
) -> Result<smithay::backend::renderer::glow::GlowRenderer, Box<dyn std::error::Error>> {
    use smithay::backend::egl::EGLContext;
    use smithay::backend::renderer::glow::GlowRenderer;

    let parent_ctx = parent.egl_context();
    let shared_ctx = EGLContext::new_shared(parent_ctx.display(), parent_ctx)?;

    #[allow(unsafe_code)]
    // SAFETY: The shared EGLContext is valid and has the same GL namespace as the parent.
    let renderer = unsafe { GlowRenderer::new(shared_ctx)? };

    Ok(renderer)
}

/// Create a standalone `GlowRenderer` for offscreen use (screenshots, headless).
///
/// Opens the first available DRI render node (e.g. `/dev/dri/renderD128`),
/// creates an EGL display + context via GBM, and wraps it in a `GlowRenderer`.
///
/// In environments without a hardware GPU, set `LIBGL_ALWAYS_SOFTWARE=1` so
/// Mesa uses its software rasterizer (llvmpipe).
///
/// # Errors
///
/// Returns an error if no render node is found or EGL/GL initialization fails.
pub fn create_offscreen_glow_renderer()
-> Result<smithay::backend::renderer::glow::GlowRenderer, Box<dyn std::error::Error>> {
    use smithay::backend::allocator::gbm::GbmDevice;
    use smithay::backend::egl::{EGLContext, EGLDisplay};
    use smithay::backend::renderer::glow::GlowRenderer;

    let render_node = find_render_node()?;

    let fd = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&render_node)
        .map_err(|e| format!("failed to open {}: {e}", render_node.display()))?;

    let gbm = GbmDevice::new(fd)?;

    #[allow(unsafe_code)]
    // SAFETY: GbmDevice implements EGLNativeDisplay — standard EGL-on-GBM init.
    let egl_display = unsafe { EGLDisplay::new(gbm)? };

    let egl_context = EGLContext::new(&egl_display)?;

    #[allow(unsafe_code)]
    // SAFETY: The EGLContext is valid.  GlowRenderer manages GL state internally.
    let renderer = unsafe { GlowRenderer::new(egl_context)? };

    Ok(renderer)
}

/// Find the first available DRI render node (e.g. `/dev/dri/renderD128`).
fn find_render_node() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dri_dir = std::path::Path::new("/dev/dri");
    if !dri_dir.exists() {
        return Err("no /dev/dri directory found — is a GPU or Mesa available?".into());
    }

    let mut entries: Vec<_> = std::fs::read_dir(dri_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("renderD")))
        .collect();

    entries.sort();
    entries.into_iter().next().ok_or_else(|| {
        "no DRI render node found (e.g. /dev/dri/renderD128) — \
         install Mesa or set LIBGL_ALWAYS_SOFTWARE=1"
            .into()
    })
}
