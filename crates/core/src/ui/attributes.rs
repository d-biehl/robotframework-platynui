//! Canonical attribute names grouped by ClientPattern namespaces.

pub mod pattern {
    /// Attributes shared by every `control:`/`item:` node regardless of Pattern.
    pub mod common {
        pub const ROLE: &str = "Role";
        pub const NAME: &str = "Name";
        pub const RUNTIME_ID: &str = "RuntimeId";
        pub const TECHNOLOGY: &str = "Technology";
        pub const SUPPORTED_PATTERNS: &str = "SupportedPatterns";
    }

    /// Base attributes for visible UI elements (Element-Pattern).
    pub mod element {
        pub const BOUNDS: &str = "Bounds";
        pub const IS_VISIBLE: &str = "IsVisible";
        pub const IS_ENABLED: &str = "IsEnabled";
        pub const IS_OFFSCREEN: &str = "IsOffscreen";
    }

    /// Desktop root attributes (Desktop-Pattern).
    pub mod desktop {
        pub const BOUNDS: &str = "Bounds";
        pub const DISPLAY_COUNT: &str = "DisplayCount";
        pub const MONITORS: &str = "Monitors";
        pub const OS_NAME: &str = "OsName";
        pub const OS_VERSION: &str = "OsVersion";
    }

    pub mod text_content {
        pub const TEXT: &str = "Text";
        pub const LOCALE: &str = "Locale";
        pub const IS_TRUNCATED: &str = "IsTruncated";
    }

    pub mod text_editable {
        pub const IS_READ_ONLY: &str = "IsReadOnly";
        pub const MAX_LENGTH: &str = "MaxLength";
        pub const SUPPORTS_PASSWORD_MODE: &str = "SupportsPasswordMode";
    }

    pub mod text_selection {
        pub const CARET_POSITION: &str = "CaretPosition";
        pub const SELECTION_RANGES: &str = "SelectionRanges";
        pub const SELECTION_ANCHOR: &str = "SelectionAnchor";
        pub const SELECTION_ACTIVE: &str = "SelectionActive";
    }

    pub mod selectable {
        pub const IS_SELECTED: &str = "IsSelected";
        pub const SELECTION_CONTAINER_ID: &str = "SelectionContainerId";
        pub const SELECTION_ORDER: &str = "SelectionOrder";
    }

    pub mod selection_provider {
        pub const SELECTION_MODE: &str = "SelectionMode";
        pub const SELECTED_IDS: &str = "SelectedIds";
        pub const SUPPORTS_RANGE_SELECTION: &str = "SupportsRangeSelection";
    }

    pub mod toggleable {
        pub const TOGGLE_STATE: &str = "ToggleState";
        pub const SUPPORTS_THREE_STATE: &str = "SupportsThreeState";
    }

    pub mod stateful_value {
        pub const CURRENT_VALUE: &str = "CurrentValue";
        pub const MINIMUM: &str = "Minimum";
        pub const MAXIMUM: &str = "Maximum";
        pub const SMALL_CHANGE: &str = "SmallChange";
        pub const LARGE_CHANGE: &str = "LargeChange";
        pub const UNIT: &str = "Unit";
    }

    pub mod activatable {
        pub const IS_ACTIVATION_ENABLED: &str = "IsActivationEnabled";
        pub const DEFAULT_ACCELERATOR: &str = "DefaultAccelerator";
    }

    pub mod activation_target {
        pub const ACTIVATION_POINT: &str = "ActivationPoint";
        pub const ACTIVATION_AREA: &str = "ActivationArea";
        pub const ACTIVATION_HINT: &str = "ActivationHint";
    }

    pub mod focusable {
        pub const IS_FOCUSED: &str = "IsFocused";
    }

    pub mod scrollable {
        pub const HORIZONTAL_PERCENT: &str = "HorizontalPercent";
        pub const VERTICAL_PERCENT: &str = "VerticalPercent";
        pub const CAN_SCROLL_HORIZONTALLY: &str = "CanScrollHorizontally";
        pub const CAN_SCROLL_VERTICALLY: &str = "CanScrollVertically";
        pub const HORIZONTAL_VIEW_SIZE: &str = "HorizontalViewSize";
        pub const VERTICAL_VIEW_SIZE: &str = "VerticalViewSize";
        pub const SCROLL_GRANULARITY: &str = "ScrollGranularity";
    }

    pub mod expandable {
        pub const IS_EXPANDED: &str = "IsExpanded";
        pub const HAS_CHILDREN: &str = "HasChildren";
    }

    pub mod item_container {
        pub const ITEM_COUNT: &str = "ItemCount";
        pub const IS_VIRTUALIZED: &str = "IsVirtualized";
        pub const VIRTUALIZATION_HINT: &str = "VirtualizationHint";
        pub const SUPPORTS_CONTAINER_SEARCH: &str = "SupportsContainerSearch";
    }

    pub mod window_surface {
        pub const IS_MINIMIZED: &str = "IsMinimized";
        pub const IS_MAXIMIZED: &str = "IsMaximized";
        pub const IS_TOPMOST: &str = "IsTopmost";
        pub const SUPPORTS_RESIZE: &str = "SupportsResize";
        pub const SUPPORTS_MOVE: &str = "SupportsMove";
        pub const ACCEPTS_USER_INPUT: &str = "AcceptsUserInput";
    }

    pub mod dialog_surface {
        pub const IS_MODAL: &str = "IsModal";
        pub const DEFAULT_RESULT: &str = "DefaultResult";
    }

    pub mod application {
        pub const PROCESS_ID: &str = "ProcessId";
        pub const NAME: &str = "Name";
        pub const EXECUTABLE_PATH: &str = "ExecutablePath";
        pub const COMMAND_LINE: &str = "CommandLine";
        pub const USER_NAME: &str = "UserName";
        pub const START_TIME: &str = "StartTime";
        pub const ARCHITECTURE: &str = "Architecture";
    }

    pub mod highlightable {
        pub const SUPPORTS_HIGHLIGHT: &str = "SupportsHighlight";
        pub const HIGHLIGHT_STYLES: &str = "HighlightStyles";
    }

    pub mod annotatable {
        pub const ANNOTATIONS: &str = "Annotations";
    }
}
