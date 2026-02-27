# egui Accessibility API Guide

<!-- Extracted from testing-strategy.md for reference during test app implementation. -->

> **Context:** This document describes how egui exposes accessibility information via [AccessKit](https://github.com/AccessKit/accesskit) and how the PlatynUI test app can leverage these APIs. For the overall testing strategy, see [testing-strategy.md](testing-strategy.md).

## AccessKit Integration in egui

[AccessKit](https://github.com/AccessKit/accesskit) has been integrated as a default feature since [eframe 0.20.0 (Dec 2022)](https://github.com/emilk/egui/blob/main/crates/eframe/CHANGELOG.md#0200---2022-12-08---accesskit-integration-and-wgpu-web-support) ([PR #2294](https://github.com/emilk/egui/pull/2294)) and automatically exposes egui widgets via native accessibility APIs:
- **Windows:** [UI Automation](https://docs.rs/accesskit_windows) via `accesskit_windows`
- **Linux:** [AT-SPI](https://docs.rs/accesskit_unix) via `accesskit_unix`
- **macOS:** [NSAccessibility](https://docs.rs/accesskit_macos) via `accesskit_macos`

The integration uses the [AccessKit winit adapter](https://crates.io/crates/accesskit_winit), which is built into eframe.
For the complete list of all AccessKit roles see [`accesskit::Role`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html) (182 variants).

## Available Widgets and Their AccessKit Roles

egui exposes ~18 widget types via [`WidgetType`](https://github.com/emilk/egui/blob/main/crates/egui/src/lib.rs), each mapped to an [`accesskit::Role`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html):

| egui Widget | AccessKit Role | Test Purpose |
|---|---|---|
| `Button` ("OK", "Cancel") | [`Button`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.Button) | Click, Focus, Bounds, ActivationPoint |
| `TextEdit` (singleline) | [`TextInput`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.TextInput) | TextContent, Keyboard Input |
| `TextEdit` (multiline) | [`TextInput`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.TextInput) | Multi-line Text, Selection |
| `Checkbox` | [`CheckBox`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.CheckBox) | Toggleable (Toggled::True/False/Mixed) |
| `RadioButton` | [`RadioButton`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.RadioButton) | Toggleable, Mutual Exclusion |
| `ComboBox` | [`ComboBox`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.ComboBox) | Expandable Drop-Down |
| `Slider` | [`Slider`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.Slider) | NumericValue, Min/Max, Step |
| `DragValue` | [`SpinButton`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.SpinButton) | NumericValue (Keyboard/Drag) |
| `Label` | [`Label`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.Label) | TextContent (read-only) |
| `Hyperlink`/`Link` | [`Link`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.Link) | Navigation |
| `ProgressBar` | [`ProgressIndicator`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.ProgressIndicator) | Progress display |
| `Image` | [`Image`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.Image) | Image content |
| `CollapsingHeader` | [`Button`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.Button) ¹ | Expandable (toggle semantics) |
| `ScrollArea` (Scrollbar) | [`ScrollBar`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.ScrollBar) | Scrollable |
| `Panel` | [`Pane`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.Pane) | Container/Layout |
| `Window` | [`Window`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.Window) | Window root |
| `ColorPicker` | [`ColorWell`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.ColorWell) | Color selection |
| `SelectableLabel` | [`Button`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html#variant.Button) | Selectable (Toggle) |

¹ `CollapsingHeader` is exposed as `Button` with toggled state, not as `TreeItem`; real tree hierarchies are therefore not represented.

**Note on `egui_extras::Table`:** The crate [`egui_extras`](https://docs.rs/egui_extras/0.33.0/egui_extras/) (also already in the project as `egui_extras = "0.33"`) provides a [`TableBuilder`](https://docs.rs/egui_extras/0.33.0/egui_extras/struct.TableBuilder.html) widget with header, scrollable body, and configurable columns. However, it is primarily a **layout mechanism** — the [Table source code](https://github.com/emilk/egui/blob/main/crates/egui_extras/src/table.rs) (~1380 lines) does not set AccessKit roles like `Table`, `Row`, `Cell`, or `ColumnHeader`. Individual cells are regular `Ui` instances whose contents (labels, buttons, etc.) expose their respective roles. For accessibility, an `egui_extras::Table` therefore appears as a **flat list of widgets**, not a structured table with row/column semantics. For proper table accessibility tests, the Qt app (Tier 2 with `QTableWidget`) is needed.

## Accessibility API Levels

egui automatically sets AccessKit roles and attributes for standard widgets (via [`Response::widget_info()`](https://github.com/emilk/egui/blob/main/crates/egui/src/response.rs), which internally maps `WidgetType` → `accesskit::Role`). For the test app, this suffices in many cases. However, when **custom widgets** need to be built or **missing roles need to be added** (e.g., `Table`, `TreeItem`, `ListItem`), egui offers three API levels:

### Level 1: `Response::widget_info()` — Standard Approach for Custom Widgets

Every egui widget internally calls `widget_info()`. For custom widgets, this method can be called after the interaction to set accessibility information:

```rust
let response = ui.allocate_response(size, Sense::click());
response.widget_info(|| egui::WidgetInfo::labeled(
    egui::WidgetType::Button,    // determines the AccessKit role
    response.enabled(),
    "My Custom Button",          // exposed as Name/Label
));
```

Internally, `widget_info()` calls two methods:
- [`fill_accesskit_node_common()`](https://github.com/emilk/egui/blob/main/crates/egui/src/response.rs) — sets bounds, focus/click actions, disabled state
- [`fill_accesskit_node_from_widget_info()`](https://github.com/emilk/egui/blob/main/crates/egui/src/response.rs) — maps `WidgetType` → `accesskit::Role`, sets label, value, toggle state, numeric values

### Level 2: `Context::accesskit_node_builder()` — Full Control Over the AccessKit Node

The central low-level API. You get a `&mut accesskit::Node` and can set or override **all** properties directly — including those that egui does not set automatically:

```rust
let response = ui.button("Click me");

// After rendering, directly manipulate the AccessKit node:
ctx.accesskit_node_builder(response.id, |node| {
    // Override role (e.g., Button → TreeItem)
    node.set_role(accesskit::Role::TreeItem);

    // Name, description, value
    node.set_name("Dashboard");
    node.set_description("Navigation element");
    node.set_value("open");

    // Toggle/expanded state
    node.set_toggled(accesskit::Toggled::True);
    node.set_expanded(true);

    // Numeric values (for Slider, ProgressBar, etc.)
    node.set_numeric_value(42.0);
    node.set_min_numeric_value(0.0);
    node.set_max_numeric_value(100.0);
    node.set_numeric_value_step(1.0);

    // Add actions
    node.add_action(accesskit::Action::Click);
    node.add_action(accesskit::Action::Expand);
    node.add_action(accesskit::Action::Collapse);

    // Manually add child nodes
    node.push_child(child_id.accesskit_id());
});
```

The method returns `Option<R>` — `None` if AccessKit is disabled.
This allows setting roles that egui does not natively expose (e.g., `Table`, `Row`, `Cell`, `TreeItem`, `ListItem`, `Menu`).

### Automatic Parent-Child Relationships (since PR #7386, egui 0.33)

Since [PR #7386](https://github.com/emilk/egui/pull/7386), **every `Ui`** automatically creates an AccessKit node with the role `GenericContainer`. Widgets within a `Ui` automatically become children of this node. The parent-child relationship is fully derived from the `Ui` nesting — **without any manual code**.

Specifically, in [`Context::accesskit_node_builder()`](https://github.com/emilk/egui/blob/main/crates/egui/src/context.rs): when creating a new AccessKit node, `find_accesskit_parent()` searches for the nearest ancestor with an AccessKit node and calls `parent_builder.push_child(id)`. This works transitively across multiple nesting levels.

This means: nested `Ui` calls (e.g., `ui.horizontal(|ui| { ui.vertical(|ui| { ... }) })`) automatically produce a correct AccessKit hierarchy.

### Level 3: `UiBuilder::accessibility_parent()` — Override Parent Node (Edge Cases)

Only in edge cases where the visual `Ui` nesting **does not** match the desired accessibility hierarchy, the parent node can be explicitly overridden:

```rust
// Edge case: A widget should be assigned to a different
// accessibility parent than its visual Ui parent.
let custom_parent_id = ui.id().with("my_list");
ctx.accesskit_node_builder(custom_parent_id, |node| {
    node.set_role(accesskit::Role::List);
    node.set_name("Task List");
});

// Child Ui with explicit accessibility parent (instead of the visual parent)
let mut item_ui = ui.new_child(
    UiBuilder::new()
        .id_salt("item_0")
        .accessibility_parent(custom_parent_id),  // ← overrides the auto parent
);
```

In practice, `accessibility_parent()` is rarely needed — the automatic derivation from the `Ui` nesting is sufficient in most cases.

## Widget IDs and `AutomationId`

egui identifies widgets internally via [`Id`](https://github.com/emilk/egui/blob/main/crates/egui/src/id.rs) — a `NonZeroU64` hash (via ahash with fixed seeds `1,2,3,4`, deterministic but **opaque/not human-readable**). The conversion to AccessKit is a direct cast: `Id.value() → accesskit::NodeId`.

How widgets get their `Id`:
- **Auto-IDs** (positional): Every widget gets `Id::new(next_auto_id_salt)`, a counter per `Ui`. **Unstable** — changes when widgets before it are added/removed.
- **Persistent IDs** (named): `ui.make_persistent_id("my_button")` or `ui.id().with("my_button")` → hash of parent ID + salt. **Stable**, but not human-readable.

For UI automation purposes (e.g., UIA `AutomationId`), AccessKit provides the property [`author_id`](https://docs.rs/accesskit/latest/accesskit/struct.Node.html#method.set_author_id):

```rust
// accesskit::Node
pub fn set_author_id(&mut self, value: impl Into<Box<str>>)
pub fn author_id(&self) -> Option<&str>
```

> *"A way for application authors to identify this node for automated testing purpose. The value must be unique among this node's siblings."*

The AccessKit Windows adapter maps `author_id` directly to the UIA property `AutomationId`. It must be set via Level 2:

```rust
let response = ui.button("OK");
ctx.accesskit_node_builder(response.id, |node| {
    node.set_author_id("btn_ok");  // → UIA AutomationId = "btn_ok"
});
```

## Convenience APIs

**Only built-in convenience method:** [`Response::labelled_by()`](https://github.com/emilk/egui/blob/main/crates/egui/src/response.rs) — the only chainable accessibility method on `Response`:

```rust
let label_resp = ui.label("Your Name:");
ui.text_edit_singleline(&mut text).labelled_by(label_resp.id);
```

Beyond this, egui provides **no ergonomic API** for accessibility properties — no `response.set_role()`, no `response.set_automation_id()`, no builder chain.

## Proposed `AccessKitExt` Trait for the Test App

For the test app, a **custom convenience trait** is recommended:

```rust
/// Convenience extension for accessibility in the test app
trait AccessKitExt {
    fn set_automation_id(&self, ctx: &egui::Context, automation_id: &str);
    fn set_a11y_role(&self, ctx: &egui::Context, role: accesskit::Role);
    fn set_a11y_description(&self, ctx: &egui::Context, description: &str);
}

impl AccessKitExt for egui::Response {
    fn set_automation_id(&self, ctx: &egui::Context, automation_id: &str) {
        ctx.accesskit_node_builder(self.id, |node| {
            node.set_author_id(automation_id);
        });
    }
    fn set_a11y_role(&self, ctx: &egui::Context, role: accesskit::Role) {
        ctx.accesskit_node_builder(self.id, |node| {
            node.set_role(role);
        });
    }
    fn set_a11y_description(&self, ctx: &egui::Context, description: &str) {
        ctx.accesskit_node_builder(self.id, |node| {
            node.set_description(description);
        });
    }
}

// Usage in the test app — significantly more ergonomic:
let resp = ui.button("OK");
resp.set_automation_id(ctx, "btn_ok");
resp.set_a11y_description(ctx, "Confirms the input");
```

## API Level Quick Reference

| Use Case | API Level | Example |
|---|---|---|
| Standard widgets (Button, TextEdit, Slider …) | Automatic | No code needed — egui sets role, bounds, name, actions |
| Parent-child hierarchy | Automatic | Since PR #7386: `Ui` nesting → AccessKit tree |
| Label association (labelled-by) | `Response::labelled_by()` | `text_edit.labelled_by(label.id)` |
| Setting `AutomationId` | Level 2 / Trait | `resp.set_automation_id(ctx, "btn_ok")` |
| Custom widgets with known `WidgetType` | Level 1 | `response.widget_info(\|\| WidgetInfo::labeled(...))` |
| Adding missing roles (`TreeItem`, `ListItem`, `Tab` …) | Level 2 / Trait | `resp.set_a11y_role(ctx, Role::TreeItem)` |
| Correcting/adding attributes after the fact | Level 2 | `node.set_description(...)`, `node.set_expanded(...)` |
| Edge case: Visual layout ≠ a11y hierarchy | Level 3 | `UiBuilder::accessibility_parent(other_id)` |
