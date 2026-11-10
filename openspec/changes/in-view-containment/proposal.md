# Proposal

## Why

`control:IsInView` is meant to say whether an element is in view, that is, not scrolled or clipped out of it (`crates/core/src/ui/attributes.rs:23-26`). On AT-SPI it does not. It reads the same state as `IsVisible` (`crates/provider-atspi/src/node.rs:1805-1814`) and ignores geometry.

Observed with Qt applications on X11:

- **Scrolled-out tabs are in view.** In a `QTabBar` with scroll buttons, every tab reports `Showing` and `Visible`, including tabs that lie entirely outside the tab list. A `Pointer Click` on such a tab lands on a different element (a scroll button, an element behind the dialog, a widget of another dialog) and does not fail. A test cannot scroll until a tab is in view, because `IsInView` is `true` from the start.
- **Elements of closed dialogs are in view.** Hundreds of elements report `IsVisible` and `IsInView` as `true` below an ancestor that reports `IsVisible` as `false` with 0×0 bounds. They are embedded web content of closed dialogs (Chromium does not know that its host widget is hidden) and the list entries of closed Qt combo boxes.
- **Partly clipped web content is in view.** Chromium drops `Showing` only for content that lies entirely outside its viewport.

## What Changes

- **On AT-SPI, `IsInView` is `true` only for an element that is fully inside the visible area of its ancestors.** In addition to `IsVisible`:
  - no ancestor reports `IsVisible` as `false`;
  - the element's own bounds are not empty;
  - the element's bounds lie inside the intersection of the non-empty bounds of all its ancestors, the desktop included. Touching an edge counts as inside. The comparison allows for the rounding of the integer coordinates AT-SPI reports.

  The value follows the geometry at the time it is read, also when the runtime keeps its snapshot between queries.

  A popup — a menu, tooltip, dialog or window, or the list of a combo box — is judged against the desktop, not against the widget below which the toolkit places it.
- **Hit-test no longer excludes a node because of `IsInView`.** It excludes only nodes that report `IsVisible` as `false`, on AT-SPI and in the mock. Otherwise a partly visible element could no longer be picked once `IsInView` is strict.

Not part of this change:

- Occlusion by siblings or by other windows. A tab that lies under the tab bar's own scroll buttons, inside the tab list's bounds, stays in view. This is a documented limitation.
- `IsVisible`, which keeps its meaning.
- Other providers. Windows UIA keeps `IsInView = !IsOffscreen`, so a partly visible element stays in view there.

## Impact

- **Behavior change for users (AT-SPI only).** `IsInView` turns `false` for elements that are partly or fully clipped, and for elements below a hidden ancestor. XPath expressions using `@IsInView`, `Get Attribute Value … IsInView`, and the high-level library's in-view predicate before pointer actions (`src/PlatynUI/ui/element.py:205-210`) see the new value.
- **Crates:**
  - `platynui-core`: a geometry check for in-view containment, and `Evaluation`, which tells a provider which query runs on the thread;
  - `platynui-runtime`: an XPath evaluation runs as its own `Evaluation`;
  - `platynui-provider-atspi`: the `IsInView` value, a cache of the ancestors' visible area that holds for one query, and the hit-test gate;
  - `platynui-provider-mock`: the hit-test gate.
- **Python/RF:** no code change. The BareMetal keyword documentation describes `IsInView`.
- **Tests:** the Qt test app gains a scrolling tab bar and a combo box behind `--in-view`, used by a new X11 acceptance suite `tests/acceptance/qt/in_view.robot`.
- **Native rebuild needed.** Platforms: Linux X11 and Wayland through AT-SPI.
- **Specs:** a new capability `in-view-attribute`; `element-at-point` changes one requirement.
