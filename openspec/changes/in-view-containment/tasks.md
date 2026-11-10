# Tasks

## 1. Containment check in core

- [x] 1.1 Unit tests for `VisibleArea` in `crates/core/src/ui/in_view.rs`, from the spec scenarios:
  - every tab of three scroll positions, tab list x 100..400 (21 cases, rstest), including tabs that end at either edge;
  - an ancestor that reports `IsVisible = false` with 0×0 bounds makes the area `Nothing`;
  - an ancestor without bounds, and a visible one with empty bounds, leave the area unchanged;
  - ancestors that do not overlap make the area `Nothing`;
  - the desktop: an element that extends beyond it;
  - tolerance: one pixel beyond is inside, two are not;
  - empty element bounds are never inside.
- [x] 1.2 Implement `VisibleArea` (design D1) and export it from `platynui_core::ui`.

## 2. Hit-test gate

- [x] 2.1 Mock test: a node with `IsVisible = true` and `IsInView = false` whose bounds contain the point is returned.
- [x] 2.2 Drop `IsInView` from the pickability gate in `crates/provider-atspi/src/lib.rs` and `crates/provider-mock/src/node.rs`, with their doc comments.

## 3. Query scope

- [x] 3.1 Unit tests for `platynui_core::ui::Evaluation`: a query is current only while entered, nested queries restore the outer one, every query is new.
- [x] 3.2 Implement `Evaluation` (design D3) and export it from `platynui_core::ui`.
- [x] 3.3 Runtime test: attribute values are read inside their own query, for `evaluate` and for `EvaluationStream`, and no query is current afterwards.
- [x] 3.4 Run every XPath evaluation as its own `Evaluation` in `crates/runtime/src/xpath.rs`, the conversion of attribute values included.

## 4. AT-SPI `IsInView`

- [x] 4.1 Qt acceptance suite `tests/acceptance/qt/in_view.robot`, with a scrolling tab bar and a combo box in the Qt test app behind `--in-view`: a tab comes into view by scrolling, and the entries of an open combo box and of an open menu are in view, closed ones not.
- [x] 4.2 Cache the visible area per node for the query that read it, and link children to their parent (design D3); read anew outside a query; clear it in `invalidate()`.
- [x] 4.3 Compute `IsInView` from `IsVisible`, the element's bounds and its parent's visible area, with the rounding tolerance (design D2).
- [x] 4.4 Judge popups (transient-popup roles and a combo box's list) against the desktop, keeping hidden ancestors (design D6).

## 5. Documentation

- [x] 5.1 BareMetal library documentation (`src/PlatynUI/BareMetal/__init__.py`): what `@IsInView` means on Linux and on Windows, and the occlusion limitation (design D5).
- [x] 5.2 `dev-docs/platform-linux.md`: how AT-SPI computes `IsInView`.
- [x] 5.3 Doc comment of `IS_IN_VIEW` (`crates/core/src/ui/attributes.rs`): the full-view meaning and the UIA difference.

## 6. Verification

- [x] 6.1 `just check`, `just test-crate platynui-core`, `just test-crate platynui-runtime`, `just test-crate platynui-provider-atspi`, `just test-crate platynui-provider-mock`, `just test-python`, `just test-baremetal`, `just test-acceptance-x11`.
- [x] 6.2 Against the real Qt application on X11 (release builds), with snapshots of the whole application before and after:
  - every tab of every scroll step: `IsInView` equals "fully inside the tab list";
  - `count(//*[@IsInView=true() and ancestor::*[@IsVisible=false()]])` is 0 (hundreds before);
  - a partly clipped web column header is not in view; the inline text labels that exceed their parent by one pixel stay in view;
  - every other element keeps its `IsInView`, except the cases listed under Risks in the design;
  - the scroll loop below terminates; after dragging the dialog within one runtime, `IsInView` is still right;
  - `//CheckBox/@IsInView` over about 100 elements takes at most 20 % longer than before;
  - the hit-test the Inspector uses (`Runtime::element_at_point`) still picks a partly visible tab;
  - an open combo box's list entries are in view.

```robot
WHILE    True    limit=15
    ${in_view}=    Get Attribute Value    ${tab}    IsInView
    IF    ${in_view}    BREAK
    Pointer Click    ${scroll_right_button}
END
```
