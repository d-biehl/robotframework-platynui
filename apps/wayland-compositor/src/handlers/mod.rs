//! Wayland protocol handler implementations.
//!
//! Each sub-module implements one or more protocol handler traits required
//! by smithay's `delegate_*!()` macros.

pub mod compositor;
pub mod cursor_shape;
pub mod data_control;
pub mod decoration;
pub mod dmabuf;
pub mod layer_shell;
pub mod output;
pub mod seat;
pub mod selection;
pub mod shm;
pub mod xdg_shell;

pub mod foreign_toplevel;
pub mod fractional_scale;
pub mod idle_inhibit;
pub mod idle_notify;
pub mod keyboard_shortcuts_inhibit;
pub mod output_management;
pub mod pointer_constraints;
pub mod pointer_warp;
pub mod screencopy;
pub mod security_context;
pub mod session_lock;
pub mod single_pixel_buffer;
pub mod text_input;
pub mod viewporter;
pub mod virtual_keyboard;
pub mod virtual_pointer;
pub mod xdg_activation;
pub mod xdg_dialog;
pub mod xdg_foreign;
pub mod xdg_system_bell;
