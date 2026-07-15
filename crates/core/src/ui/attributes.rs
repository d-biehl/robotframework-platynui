//! Canonical attribute names grouped by ClientPattern namespaces.

pub mod pattern {
    /// Attributes shared by every `control:`/`item:` node regardless of Pattern.
    pub mod common {
        pub const ROLE: &str = "Role";
        pub const NAME: &str = "Name";
        /// Developer-provided stable element identifier (optional)
        pub const ID: &str = "Id";
        pub const RUNTIME_ID: &str = "RuntimeId";
        pub const TECHNOLOGY: &str = "Technology";
        pub const SUPPORTED_PATTERNS: &str = "SupportedPatterns";
    }

    /// Base attributes for visible UI elements (Element-Pattern).
    pub mod element {
        pub const BOUNDS: &str = "Bounds";
        pub const IS_VISIBLE: &str = "IsVisible";
        pub const IS_ENABLED: &str = "IsEnabled";
        /// Element is currently within its container's viewport (i.e. not
        /// scrolled or clipped out of view). The positive complement to the
        /// UIA `IsOffscreen` / AT-SPI `!Showing` flags.
        pub const IS_IN_VIEW: &str = "IsInView";
    }

    /// Desktop root attributes (Desktop-Pattern).
    pub mod desktop {
        pub const BOUNDS: &str = "Bounds";
        pub const DISPLAY_COUNT: &str = "DisplayCount";
        pub const MONITORS: &str = "Monitors";
        pub const OS_NAME: &str = "OsName";
        pub const OS_VERSION: &str = "OsVersion";
    }

    pub mod activatable {
        pub const IS_ACTIVATION_ENABLED: &str = "IsActivationEnabled";
        pub const DEFAULT_ACCELERATOR: &str = "DefaultAccelerator";
    }

    pub mod window_state {
        pub const IS_ACTIVE: &str = "IsActive";
        pub const IS_TOPMOST: &str = "IsTopmost";
        pub const IS_MODAL: &str = "IsModal";
    }

    pub mod activation_target {
        pub const ACTIVATION_POINT: &str = "ActivationPoint";
        pub const ACTIVATION_AREA: &str = "ActivationArea";
        pub const ACTIVATION_HINT: &str = "ActivationHint";
    }

    pub mod focusable {
        pub const IS_FOCUSED: &str = "IsFocused";
    }

    /// Read-only text content (TextContent-Pattern).
    pub mod text_content {
        /// Current textual content of a text-bearing element, sourced only from
        /// a genuine accessibility text interface (never the accessible name).
        pub const TEXT: &str = "Text";
    }

    /// Writable text elements (TextEditable-Pattern; extends TextContent).
    pub mod text_editable {
        pub const IS_READ_ONLY: &str = "IsReadOnly";
        pub const MAX_LENGTH: &str = "MaxLength";
    }

    /// Two/tri-state toggles (Toggleable-Pattern).
    pub mod toggleable {
        /// Canonical values: "On", "Off", "Indeterminate".
        pub const TOGGLE_STATE: &str = "ToggleState";
    }

    /// Numeric value holders like sliders and progress bars (StatefulValue-Pattern).
    pub mod stateful_value {
        pub const VALUE: &str = "Value";
        pub const MIN_VALUE: &str = "MinValue";
        pub const MAX_VALUE: &str = "MaxValue";
    }

    /// Items that can be selected within a container (Selectable-Pattern).
    pub mod selectable {
        pub const IS_SELECTED: &str = "IsSelected";
    }

    /// Containers managing a selection (SelectionProvider-Pattern).
    pub mod selection_provider {
        /// RuntimeIds of the currently selected child items.
        pub const SELECTED_ITEMS: &str = "SelectedItems";
        pub const CAN_SELECT_MULTIPLE: &str = "CanSelectMultiple";
    }

    /// Elements that can expand/collapse (Expandable-Pattern).
    pub mod expandable {
        pub const IS_EXPANDED: &str = "IsExpanded";
        pub const CAN_EXPAND: &str = "CanExpand";
    }

    pub mod minimizable {
        pub const IS_MINIMIZED: &str = "IsMinimized";
        pub const CAN_MINIMIZE: &str = "CanMinimize";
    }

    pub mod maximizable {
        pub const IS_MAXIMIZED: &str = "IsMaximized";
        pub const CAN_MAXIMIZE: &str = "CanMaximize";
    }

    pub mod closeable {
        pub const CAN_CLOSE: &str = "CanClose";
    }

    pub mod movable {
        pub const CAN_MOVE: &str = "CanMove";
    }

    pub mod resizable {
        pub const CAN_RESIZE: &str = "CanResize";
    }

    pub mod application {
        pub const PROCESS_ID: &str = "ProcessId";
        pub const PROCESS_NAME: &str = "ProcessName";
        pub const EXECUTABLE_PATH: &str = "ExecutablePath";
        pub const COMMAND_LINE: &str = "CommandLine";
        pub const USER_NAME: &str = "UserName";
        pub const START_TIME: &str = "StartTime";
        pub const ARCHITECTURE: &str = "Architecture";
    }
}
