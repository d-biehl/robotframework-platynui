# Design

## Context

Verified against the code:

- AT-SPI computes `IsVisible` and `IsInView` identically from the state set (`Visible || Showing`), with no geometry (`crates/provider-atspi/src/node.rs:1805-1814`). Both are listed only for nodes with the `Component` interface (`node.rs:1374-1395`).
- Standard attributes resolve through a `LazyNodeData` that is created anew for every `attributes()` or `attribute()` call (`node.rs:1131-1141`). A node that reads its parent's bounds therefore reads them again every time; nothing is shared between siblings.
- `AtspiNode` already caches per node in `ClearableCell`s (`state`, `interfaces`, `cached_name`, `cached_child_count`), which `invalidate()` clears (`node.rs:519-524`).
- A node reaches its parent only as `Weak<dyn UiNode>` (`node.rs:349-351`). `UiNode` offers no downcast. Children of a node are built in one place, `AtspiNode::children` (`node.rs:414-424`). The hit-test and popup paths build nodes elsewhere (`crates/provider-atspi/src/lib.rs:554`, `:609`, `:660`).
- An application node's parent is the runtime's desktop node, reached through the `parent` `Weak` (`node.rs:83-88`). The desktop lists `Bounds` (the bounding box of the monitors) and `IsVisible = true` (`crates/runtime/src/runtime/desktop.rs:50-53`).
- Python reads attributes straight from the node (`src/PlatynUI/core/adapters/ui_node.py:55-61`), not through XPath. The value must therefore come from the provider; a derivation in the runtime's XPath layer would give XPath and `Get Attribute Value` different answers.
- Hit-test gates on `IsVisible` and `IsInView` (`crates/provider-atspi/src/lib.rs:726-731`; mock `crates/provider-mock/src/node.rs:81-87`).
- `platynui-provider-atspi` depends on `platynui-core`, not on `platynui-runtime` (`crates/provider-atspi/Cargo.toml`).
- The runtime keeps its XPath snapshot between queries, and the Robot Framework library keeps it as long as a lookup finds its element (`crates/runtime/src/xpath.rs`, `XdmCache`; `src/PlatynUI/core/adapter_factory.py`). The provider nodes of a snapshot therefore live across many keywords. Before a query reuses the snapshot, the runtime forgets the attribute values it read (`prepare_for_evaluation`), so attribute values hold for one query. A provider node knows nothing of queries.
- XPath evaluation runs on the thread that pulls its results; the engine starts no threads of its own.

## Decisions

### D1: The containment check lives in `platynui-core` as a pure function

A small module `platynui_core::ui::in_view` holds a `VisibleArea` and the check:

- `VisibleArea::Unbounded`, `Within(Rect)` or `Nothing`;
- `VisibleArea::narrow(self, visible: Option<bool>, bounds: Option<Rect>)` adds one ancestor: `Some(false)` makes it `Nothing`; missing or empty bounds leave it unchanged; non-empty bounds intersect it, and no overlap makes it `Nothing`;
- `VisibleArea::contains(&self, bounds: Rect, tolerance: f64)` answers whether non-empty `bounds` lie inside, edges inclusive.

It knows no tolerance value and reads no attribute. Providers depend on core, so any provider can use it later; this change uses it only in AT-SPI.

### D2: The tolerance is the rounding of the provider's coordinates, not a setting

AT-SPI reports whole pixels. Chromium lays out in fractions of a pixel, and the reported boxes of a child and its parent are rounded independently, so an inline text box can be reported one pixel beyond its line box on each side. Each edge can be off by less than one pixel. The AT-SPI provider therefore passes a named constant of one pixel with this justification. A setting was rejected: there is no evidence that any value other than the reporting resolution is needed, and it would add API.

### D3: Each node caches the visible area it gives its descendants, for one query

`AtspiNode` gets two fields:

- `visible_area: Mutex<Option<(Evaluation, VisibleArea)>>`: the area this node leaves for its descendants, that is, its parent's area narrowed by this node's `IsVisible` and `Bounds`, together with the query it was read in;
- `area_parent: Mutex<Option<Weak<AtspiNode>>>`: the parent as an `AtspiNode`, set in `children()` for each child it builds. `self_atspi: OnceLock<Weak<AtspiNode>>` is set in `build` so `children()` has a typed weak reference to hand out.

A node computes its area from its `area_parent`'s area. Without an `area_parent` (hit-test and popup paths, application nodes under the desktop), it walks the remaining ancestors through `UiNode::parent` and reads `IsVisible` and `Bounds` from each. Siblings share their parent node object, so in one query each ancestor's state and bounds are read once, not once per element.

The cached area holds only for the query that read it. `platynui_core::ui::Evaluation` names one query: the runtime gives each XPath evaluation a new one and marks it as running on the thread while it builds the evaluation and while it produces each result, the conversion of attribute values included (`crates/runtime/src/xpath.rs`, `evaluate_iter`, `EvaluationStream`). A node uses its cached area only when it was read in the query that is running now. Outside a query, as for `node.attribute("IsInView")` on a node a caller holds, nothing is cached and the ancestors are read anew.

Rejected:

- **Cache until `invalidate()`.** It is the fastest, but a node of a retained snapshot keeps its area across queries. Verified against a real Qt application: after a dialog was dragged, every tab in it read as not in view, because the dialog's old area did not overlap the tab list's new one. Scrolling a container that holds other containers would go stale the same way, which is the case this attribute exists for.
- **No cache.** Correct, but every element reads all its ancestors. `//CheckBox/@IsInView` over about 100 check boxes of a dialog took 39 % longer than before, against 10 % with the cache per query.
- **Clearing the cache in `prepare_for_evaluation`.** It needs a new `UiNode` method, it runs only when a snapshot is reused, and it leaves direct reads between two queries stale.

`IsInView` itself is still read lazily, only when asked. It reads the element's own state and bounds through its `LazyNodeData`, and its parent's area through D3. `invalidate()` clears the cached area like the other node caches.

A visible ancestor with empty bounds is skipped and logged at `debug`, because it could also be a toolkit that reports no geometry for a container.

### D4: Hit-test gates on `IsVisible` only

With a strict `IsInView`, a partly visible tab could no longer be picked: the resolver would fall through to the tab list. Clipping does not need the gate. A fully clipped node loses to the smaller or frontmost node at the point, or to the window that is in front. The gate drops `IsInView` in AT-SPI and in the mock, which mirrors it.

### D5: Strict on AT-SPI; Windows keeps the UIA meaning

UIA's `IsOffscreen` counts an element as in view while any part of it is visible, and `GetClickablePoint` then finds a point that a hit-test confirms (`crates/provider-windows-uia/src/node.rs:946-949`). Taking that meaning over to AT-SPI was rejected:

- AT-SPI has no clickable point. The geometric substitute, the centre of the visible part, does not notice occlusion. When the tab bar's scroll buttons cover the end of the tab list, the centre of a partly visible tab's visible part can lie on a scroll button, and a tab can be visible only under them. Both would be in view, and a click would hit a scroll button.
- Users ask whether an element is shown in full, and a lenient `IsInView` cannot express that.
- Changing `ActivationPoint` would change every pointer action on Linux.

The platforms therefore differ: a partly visible element is in view on Windows and not on Linux. The doc comment of `IS_IN_VIEW` (`crates/core/src/ui/attributes.rs`) and the BareMetal documentation say so. Making UIA strict as well is left to a change of its own.

### D6: A popup starts again from the desktop

Toolkits place some popups below the widget that opens them, although each is a window of its own and extends beyond that widget. Verified with the Qt test app (`apps/test-app-qt`, `--in-view`):

- the list of an open combo box hangs below the `ComboBox`, extends beyond it, and reports a width of 880 px while its entries are 947 px wide; a list that opens upwards also extends above its dialog;
- the menu bar's File menu is a `PopupMenu` below the `File` entry (y 24..46) and opens below it (y 46..121).

The ancestor chain clipped every entry, so the entries of open combo boxes and menus were not in view, and the high-level library checks `IsInView` before it selects an item (`src/PlatynUI/ui/item.py`). The provider itself grafts event-driven popups, such as GTK4 popover menus, below their owner (`popups.rs`).

A node therefore counts as a popup (`AtspiNode::is_popup`) when its role is one of the transient-popup roles the provider already knows (`PopupMenu`, `Menu`, `ToolTip`, `Window`, `Dialog`; `popups.rs`), or when it is a `List` whose parent is a `ComboBox`, which Qt exposes with no popup role. A popup's area starts from the desktop: the ancestors between narrow nothing, but a hidden one still leaves nothing, so the entries of a closed combo box or menu stay out of view. A popup's own bounds narrow its contents, except a combo box's list, whose reported width is too small.

Rejected:

- **Only the combo box's list.** Menus below their menu bar entry, submenus beside their parent menu, GTK combo boxes (a `Menu`) and grafted popovers would stay out of view.
- **Popup windows from the window manager.** X11 only, and the size match fails on Qt's combo box list.
- **A child entirely outside its parent.** It cannot be told apart from a tab scrolled out of its tab list.
- **Documenting the limitation.** Items of open combo boxes and menus could not pass the high-level library's in-view check.
- **Letting only container roles clip.** It would also cover web content whose boxes overflow their parents, but it is a heuristic over every toolkit's roles.

Entries scrolled out of a long open list are judged against the desktop only, as before this change. A web element with the role `dialog` is no longer clipped by the document's viewport.

## Risks / Trade-offs

- **Occlusion is not detected.** A tab under the tab bar's own scroll buttons lies inside the tab list's bounds and stays in view. Documented as a limitation.
- **Platforms differ.** UIA reports a partly visible element as in view (D5). Documented.
- **A shown child under a parent reported as not visible.** The hit-test still searches below hidden nodes because "a shown child can live under a parent the toolkit reports as not visible" (`crates/provider-atspi/src/lib.rs:716-718`). No concrete case is recorded. Such a child would now be out of view. Before/after snapshots of a real Qt application changed only elements below a hidden ancestor that is a closed dialog or combo box, elements clipped by their ancestors, and the two cases below.
- **Popups** below the application, below their owner or below the widget that opens them are handled by D6. A toolkit that exposes a popup with a role outside that list stays clipped by its ancestors.
- **Content that overflows its parent without being clipped.** The check assumes that an ancestor clips its descendants. Qt widgets and GTK do; web boxes and Qt Quick items do not by default (`overflow: visible`, `clip: false`). A web text box that exceeds its label box by two pixels, more than the rounding allows, is not in view, although its text is likely shown in full. Absolutely positioned web dropdowns and Qt Quick children placed outside their parent are affected the same way. Letting only the web document or only clipping items narrow the area would address this; it is left to a change of its own once concrete cases need it.
- **Elements that report empty bounds.** A canvas can report 0×0 bounds while it is shown. Its `IsInView` is `false`, as for every element with empty bounds.
- **Substituted top-level geometry.** When the window manager cannot answer, a top-level reports toolkit geometry, which is window-relative on Wayland. Its descendants are then compared against the desktop in a different coordinate space. Window-relative coordinates start at 0,0, so they usually lie inside the desktop. Not handled.
- **Desktop bounds are a bounding box.** Gaps between monitors of different sizes are not detected.
- **Performance.** An `IsInView` read now also reads the element's bounds, and ancestors are read once per query. Measured against a real Qt application (release builds, median): `//CheckBox/@IsInView` over about 100 check boxes of a dialog took 10 % longer; over all check boxes of the application, most of them below hidden ancestors, about a quarter longer. A single read in the Robot Framework loop is dominated by the query that finds the element and took 2 % longer.

## Migration Plan

- Behavioral change on AT-SPI. No change to the Python or Robot Framework API; `platynui-core` gains `Evaluation`.
- Needs a native rebuild (`just build-native`).
- Rollback: revert the provider and runtime commits. The hit-test change is correct on its own and can stay.
