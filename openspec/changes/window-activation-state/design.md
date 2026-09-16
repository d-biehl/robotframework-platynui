## Context

See proposal.md (Why) for the defect. The facts below shape the approach. Each one is marked **verified** (read in the code) or **assumed** (to be confirmed in a lane).

**Where the defect lives.**
- `Runtime::bring_to_front` resolves the top-level window, calls `restore()` when the window has a Restorable pattern, and then calls `activate()` (`crates/runtime/src/runtime/window.rs:57-76`). **Verified.**
- Every `restore()` returns the window to the normal state, so each of these un-maximizes (**verified**):
  - UIA: `SetWindowVisualState(Normal)` (`crates/provider-windows-uia/src/node.rs:648-653`)
  - Win32: `ShowWindow(SW_RESTORE)` (`crates/platform-windows/src/window_manager.rs:259-267`)
  - X11: removes `_NET_WM_STATE_MAXIMIZED_*` (`crates/platform-linux-x11/src/window_manager.rs:594-617`)
  - PlatynUI compositor: `restore_window` → `do_unmaximize` (`apps/wayland-compositor/src/handlers/foreign_toplevel.rs:636-650`)
  - Mock provider: clears `is_maximized` (`crates/provider-mock/src/window.rs:341-348`)
- The implicit callers are BareMetal's `_maybe_bring_to_front`, used by every pointer keyword and by keyboard keywords with a descriptor (`src/PlatynUI/BareMetal/__init__.py:1606-1630`), and the CLI pointer commands (`crates/cli/src/commands/pointer.rs:591-593`). The CLI computes the click point *before* activating (`pointer.rs:249-253`). **Verified.**

**What activation does today.**

| Backend | Activation | Brings a minimized window back? |
|---|---|---|
| Win32 `WindowManager` | `ShowWindow(SW_RESTORE)` when `IsIconic`, then foreground-lock-safe `SetForegroundWindow` (`window_manager.rs:190-240`) | yes (**verified**); returns to maximized if minimized from maximized (**assumed**: Win32 restore-to-maximized placement semantics) |
| UIA provider | `SetFocus` + wait for foreground, no WindowManager (`node.rs:574-589`; UIA takes no injected WM, `crates/core/src/provider/tree_provider.rs:50`) | not handled (**verified**) |
| X11 EWMH | `_NET_ACTIVE_WINDOW` client message only (`window_manager.rs:544-551`) | depends on the WM; EWMH does not require it. The X11 lane runs IceWM (`scripts/startxsession.sh:280-291`) |
| PlatynUI compositor via IPC | `focus_window` command (`platynui_ipc.rs:128-132`) → `control.rs:1220-1232`, which only focuses and raises mapped windows | no. A minimized window is unmapped (`foreign_toplevel.rs:750-769`), and the index lookup only walks mapped windows (`control.rs:953-985`) (**verified**) |
| Mock provider | sets active, clears `is_minimized`, keeps `is_maximized` (`window.rs:285-314`) | yes, but `minimize` clears `is_maximized` (`window.rs:316-328`), so the mock forgets that the window was maximized (**verified**) |
| JAB, Java agent | delegate to the injected WindowManager (`provider-java-jab/src/node.rs:1228-1246`, `provider-java/src/agent/node.rs:706-712`) | inherit the WM behavior |

**Minimized windows on the PlatynUI compositor cannot be resolved today.**
- The IPC backend's `resolve_window` matches only the `windows` array of `list_windows` (`platynui_ipc.rs:67-90`, `276-286`).
- The compositor lists minimized windows separately, without size or state (`MinimizedWindowInfo`, `control.rs:179-187`; `list_windows`, `control.rs:384-388`).
- `get_window` by `window_id` searches only mapped windows (`control.rs:833-847`).
- The one command that already looks up a minimized window is `restore_window_by_selector` (`control.rs:886-915`). **Verified.**

**State reporting today.**
- The trait has `is_active` but no state query (`crates/core/src/platform/window_manager.rs:61-129`). Optional capabilities follow a default-`CapabilityUnavailable` pattern (`window_at_point`, `popups`, lines 116-128). **Verified.**
- Attribute names already exist: `minimizable::IS_MINIMIZED`, `maximizable::IS_MAXIMIZED`, `window_state::IS_TOPMOST` (`crates/core/src/ui/attributes.rs:43-47`, `104-112`). **Verified.**
- Providers that already report state:
  - UIA reads them from WindowPattern (`node.rs:458-462`, `1513-1565`).
  - The Java agent reads the frame's extended state (`provider-java/src/agent/node.rs:660-672`).
  - The mock keeps its own state.
- AT-SPI and JAB report only `IsActive`, through the WM:
  - AT-SPI: `node.rs:1180-1190`, `1462-1465`, `1586-1589`
  - JAB: `node.rs:483`, `1176-1198`
- The compositor already computes `maximized`/`fullscreen` for mapped windows (`control.rs:152-154`, `1037-1039`), but the IPC decoder ignores both (`platynui_ipc.rs:443-482`). **Verified.**

## Goals / Non-Goals

**Goals:**
- One activation contract that every backend meets: a minimized window comes back to its pre-minimize state, and a maximized window stays maximized.
- A window-state query on the `WindowManager` that serves both the provider attributes and backend-internal decisions (X11 de-iconify).

**Non-Goals:**
- A fullscreen state. The attribute model has no `IsFullscreen`; adding one later is additive (see Open Questions).
- Folding `is_active` into the state query. Activeness is a desktop-global fact (foreground window, `_NET_ACTIVE_WINDOW` on the root window), not a per-window state, and existing callers keep using it.
- Changing what `Restorable.restore()` does, including the extra foreground activation that the Win32 and X11 `restore()` perform today.
- Window-manager backends for Wayland compositors other than PlatynUI, a macOS window manager, and virtual-desktop switching.
- Bounds of minimized windows. Resolving them is enough for this change.
- Replacing the toolkit-sourced `IsMinimized`/`IsMaximized` of the UIA and Java-agent providers.

## Decisions

### D1: Activation owns bringing a window back from minimized; `bring_to_front` only activates

`Runtime::bring_to_front` becomes "find the top-level window, then `activate()`". Each backend's activation brings a minimized window back to its pre-minimize state.

*Alternatives considered:*
- **Runtime checks `IsMinimized` and calls `restore()` only then.** Rejected. `restore()` means "normal state" on every backend, so a window minimized from maximized would come back un-maximized. The check also depends on `IsMinimized`, which AT-SPI and JAB do not have today.
- **A separate `unminimize` operation on the WindowManager, called by the runtime.** Rejected. Direct `Activate Window` calls would still leave a minimized window hidden on the UIA provider and the compositor. Activation that brings a window back is also what native activation already means: Win32 taskbar activation, foreign-toplevel `activate` (`foreign_toplevel.rs:578-589`), and `_NET_ACTIVE_WINDOW` in common X11 window managers.

### D2: One snapshot query that returns an exclusive visual state plus a topmost flag

`WindowManager` gains a state query returning a small value type:
- a visual state that is exactly one of Normal, Minimized or Maximized;
- a `topmost` flag.

The default implementation reports `CapabilityUnavailable`, following the pattern of `window_at_point`/`popups`.

*Alternatives considered:*
- **Separate `is_minimized`/`is_maximized` methods, mirroring `is_active`.** Rejected. That is several round trips per read (compositor IPC, X11 property reads) for data a single read returns.
- **Two booleans.** Rejected. They allow the invalid "minimized and maximized" combination. The exclusive enum matches UIA's `WindowVisualState`, which the UIA provider's attributes already follow. It also makes "a minimized window reports Minimized" (spec: window-state) hold by construction, even where the platform still keeps a maximized flag underneath (X11 `_NET_WM_STATE`, xdg toplevel state).

### D3: Where each backend reads the state

| Backend | Minimized | Maximized | Topmost |
|---|---|---|---|
| Win32 | `IsIconic` | `IsZoomed` | `WS_EX_TOPMOST` in the extended window style |
| X11 EWMH | `_NET_WM_STATE_HIDDEN` on the client window | both `_NET_WM_STATE_MAXIMIZED_VERT` and `_HORZ` | `_NET_WM_STATE_ABOVE` |
| PlatynUI compositor via IPC | window is in the compositor's minimized list | compositor's `maximized` flag | always false (the compositor has no always-on-top concept) |
| Platform mock | Normal | Normal | false; the call is recorded like every other mock WM call (`crates/platform-mock/src/window_manager.rs:10-21`) |

On X11, Minimized wins over the maximized atoms, as D2 requires. Two atoms (`_NET_WM_STATE_ABOVE`, and `WM_STATE` if needed) join the existing atom cache (`window_manager.rs:31-91`).

### D4: How each backend meets the activation contract

- **Win32 `WindowManager`: no change.** It already restores an iconic window first (`window_manager.rs:193-196`).
- **UIA provider.** Before `SetFocus`, if the element's native window handle is iconic, call `ShowWindow(SW_RESTORE)` on it. The handle is already read in `activate` (`node.rs:578-581`).
  - *Rejected:* `SetWindowVisualState(Normal)`, which means un-maximized.
  - *Rejected:* injecting the Win32 WindowManager into the UIA provider. That is a wiring change well beyond this fix.
- **X11 EWMH.** When the state query reports Minimized, first send `MapWindow` for the client window (ICCCM §4.1.4: a client leaves Iconic state by mapping the window; the WM receives it as a MapRequest and keeps `_NET_WM_STATE`, so maximized survives). Then send `_NET_ACTIVE_WINDOW` as today.
  - *Rejected:* removing `_NET_WM_STATE_HIDDEN` by client message, which today's `restore()` does (`window_manager.rs:614`). EWMH reserves `_NET_WM_STATE_HIDDEN` for the window manager, and window managers may ignore client requests to change it.
- **PlatynUI compositor.**
  - `focus_window` activates through the same path as foreign-toplevel activation (`foreign_toplevel.rs:578-589`). That path maps a minimized window back at its saved position and focuses and raises it. Un-minimizing does not touch the xdg toplevel's maximized state (`foreign_toplevel.rs:775-782`).
  - The `window_id` lookup for `focus_window` also searches the minimized list, the way `restore_window_by_selector` already does.
- **Mock provider.** `minimize` remembers whether the window was maximized; activation of a minimized window restores that; `restore` and `maximize` clear the memory. This keeps the contract testable in the fast mock lane. The mock's activation state was already a reliable signal for effect tests.

### D5: Minimized windows become resolvable on the compositor without changing the mapped-window list

- The compositor's minimized-window entries gain what matching and state reporting need: content size and the maximized flag.
- `get_window` by `window_id` also finds minimized windows and reports them as minimized.
- The IPC backend decodes both arrays, marks each window as minimized or not, and passes both to the existing match.

The `windows` array keeps meaning "mapped windows". Its `id` is the space index used by index-based commands (`control.rs:967-969`) and by the window-at-point logic, so putting unmapped windows into it would change those meanings. *Rejected:* merging minimized windows into `windows`.

### D6: AT-SPI and JAB attributes copy `IsActive`

`IsMinimized`, `IsMaximized` and `IsTopmost` are added wherever `IsActive` is emitted: AT-SPI's lazy standard attributes on window surfaces, and JAB's top-level attribute list. Each read resolves the window and asks the WM for its state. When it cannot, the value is `False`, matching `IsActive`'s `unwrap_or(false)`. Each attribute's value is read on its own, like `IsActive`.

*Rejected:* caching one state snapshot per node. Attributes are read live by design, and a stale cache would break "State is read live" (spec: window-state).

## Risks / Trade-offs

- **[IceWM or another X11 WM does not de-iconify on `MapWindow`]** → `_NET_ACTIVE_WINDOW` is still sent afterwards, and common window managers de-iconify on it. The X11 acceptance scenario "Bring To Front on an element of a minimized window" catches a WM that does neither.
- **[Assumption: `SW_RESTORE` brings a window minimized from maximized back maximized]** → Confirm in the Windows lane. If it is wrong, read the window placement's restore-to-maximized flag and show the window maximized instead, confined to the Win32 WM and the UIA provider.
- **[Every state-attribute read costs a WindowManager round trip]** → Same cost profile as `IsActive` today. It applies only to top-level windows and only when the attribute is read.
- **[`False` on an unresolvable window hides a failure behind a plausible value]** → Chosen for consistency with `IsActive` (spec: window-state). State-query failures are logged at debug level so a lane investigation can see them.
- **["Maximize Button Toggles The Window State" (`tests/acceptance/egui/inspector_window_controls.robot:43-50`) may currently pass by accident of the race]** → It is part of the Wayland verification run. A change in its outcome is a finding to investigate, not something to paper over.
- **[Compositor control-socket additions drift from the IPC decoder]** → Both live in this repo and ship together. The new JSON fields are additive, and the decoder treats missing fields as absent.

## Migration Plan

- **Nature of the change:**
  - *Bug fix:* activation keeps the maximized state, and `bring_to_front` no longer restores. This is not a breaking change; un-maximizing on activation was never intended behavior.
  - *Additive:* the trait method (default implementation), the new AT-SPI/JAB attributes, and the compositor JSON fields.
- **Rebuilds:** Python/RF users need a native rebuild (`just build-native`), and the Linux lanes need the rebuilt compositor. There is no persisted data and no configuration change.
- **Rollout order within the change:**
  1. Core type and default method.
  2. Backends and compositor.
  3. Providers.
  4. Runtime `bring_to_front`.
  5. Docs.

  Each step leaves the workspace buildable and tested.
- **Rollback:** revert the change's commits. Because the trait method is additive, the runtime part can also be reverted alone (put back `restore()` before `activate()`) without touching backends or attributes.

## Open Questions

- Should fullscreen become part of the reported state (a fourth visual state or a separate flag) with a matching `IsFullscreen` attribute? This is deferrable and additive either way. The compositor and X11 can already tell; Win32 has no native concept.
