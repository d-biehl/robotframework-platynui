//! `wp-security-context-v1` handler — sandboxed client support.
//!
//! Allows privileged clients (e.g., Flatpak portal) to create restricted
//! Wayland connections for sandboxed applications with limited protocol access.
//!
//! When `--restrict-protocols` is active, the security context's `app_id` is
//! checked against the whitelist before accepting the connection.

use std::sync::Arc;

use smithay::wayland::security_context::{SecurityContext, SecurityContextHandler, SecurityContextListenerSource};

use crate::{client::ClientState, state::State};

impl SecurityContextHandler for State {
    fn context_created(&mut self, source: SecurityContextListenerSource, context: SecurityContext) {
        // Check security policy: if restrictive, verify the context's app_id
        if self.security_policy.is_restrictive() {
            let app_id = context.app_id.as_deref().unwrap_or("");
            if !self.security_policy.is_allowed(app_id) {
                tracing::warn!(app_id = app_id, "security context rejected: app_id not in whitelist",);
                return;
            }
            tracing::debug!(app_id = app_id, "security context accepted by policy");
        }

        // Accept the new security context and register its listener.
        if let Err(err) = self.loop_handle.insert_source(source, |client_stream, _security_context, state| {
            if let Err(err) = state.display_handle.insert_client(client_stream, Arc::new(ClientState::default())) {
                tracing::warn!(%err, "failed to insert security-context client");
            }
        }) {
            tracing::warn!(%err, "failed to register security context listener");
        }
    }
}
