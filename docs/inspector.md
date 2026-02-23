# Inspector

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document covers the PlatynUI GUI Inspector. For the platform-agnostic architecture, see `docs/architecture.md`.

Binary: `platynui-inspector-rs` (package `platynui-inspector`, Slint-based GUI)

## TreeView Architecture

The inspector uses a domain-agnostic TreeView component:

- **TreeData trait** — read-only interface to any tree structure
- **TreeViewAdapter trait** — UI port with flattened visible rows and commands
- **ViewModel** — implements both traits, flattens hierarchical data to visible rows with depth tracking
- **UiNodeData** — `TreeData` implementation backed by PlatynUI runtime

The TreeView knows only `TreeNodeVM` and string IDs — no coupling to `UiNode`.

## Current State

Phases 1-4 complete: skeleton, interaction (mouse/keyboard navigation), adapter & ViewModel, UiNode integration. Real desktop hierarchy is displayed, errors are visible and retryable.
