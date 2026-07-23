## Why

The fixture matrix has no Qt Quick/QML app, yet QML is how most modern Qt applications are built — and it is accessibility-wise its own technology: scene-graph rendering (no native widgets), names and roles from `Accessible` attached properties, and menus/popups that are by default in-scene items rather than native windows. Those are exactly the deviations the semantic keyword layer's shipped technology proxies must know about. This change adds the QML fixture as the **first full instance of the `test-app-blueprint`** and delivers the blueprint's catalog acceptance suite as its reference implementation.

## What Changes

- A new fixture **`apps/test-app-qml`**: Python/PySide6 (already a dev dependency — same reasoning as `apps/test-app-qt`), a thin `main.py` (PEP 723 inline metadata, argparse) loading a QML scene that implements the blueprint's **core-tier catalog** under the canonical names via `Accessible` attached properties (extended tier follows in a later change).
- **Both popup realities**: default in-scene menus/popups (Qt Quick's default, the hard case for bounds/hit-test), plus a `--popup-mode native` switch using Qt ≥ 6.8 `popupType`/native menus so the same catalog can be exercised against native popup windows. Dialogs cover both faces too: `dialog-modeless` as a real child `Window`, `dialog-modal` as an in-scene modal `Dialog`.
- **Custom-controls chapter implemented** (blueprint's optional chapter): a self-drawn `custom-button` (`Rectangle` + `MouseArea` + manual `Accessible` wiring) with its `custom-status-label` counter, and one deliberately non-exposed drawn element as the negative case.
- **Catalog suite (reference implementation)**: the blueprint's canonical catalog test set as the self-contained `tests/acceptance/qml/catalog.robot` (test bodies directly against `PlatynUI.BareMetal`, instance pinned via `Set Root`), plus QML-specific suites for the popup-mode and custom-control coverage. What later fixtures share is the contract (names, observables, test set), not keyword code.
- **Windows + Linux lanes from the start**: fixture launch handed over as `PLATYNUI_TEST_APP_QML_*` by `scripts/platynui-robot-session.sh` (Linux) and the `test-acceptance-windows` recipe (Windows), mirroring the Qt Widgets wiring; surfaced `@Name`/roles verified against the real UIA and AT-SPI trees before the suites encode them.

## Capabilities

### New Capabilities

- `qml-test-app`: the Qt Quick/QML fixture — blueprint-conforming core catalog with QML `Accessible` wiring, dual popup modes, implemented custom-controls chapter, and the first onboarding of the catalog acceptance suite on Windows (UIA) and Linux (AT-SPI).

### Modified Capabilities

- `test-app-blueprint`: first-contact adjustments from verifying the QML fixture against the real UIA tree — shared catalog locators address controls by `@Name` alone (roles differ across bridges); window naming falls back to launch-configuration matching where a bridge derives the window name from its title; the `--open-modal` scenario asserts modal state only where the bridge surfaces it (documented-deviation rule otherwise). Additionally, the core tier gains a **multi-line text area** (`textarea-basic`) — multi-line editing behaves differently from the single-line field in every toolkit and was previously a deliberate gap.

## Impact

- **New**: `apps/test-app-qml/**` (`main.py`, QML sources, README), `tests/acceptance/qml/**` suites.
- **Modified**: root `Cargo.toml` (`exclude` entry — `apps/*` is a workspace-member glob and this is not a crate, same as `apps/test-app-qt`), `justfile` (`test-acceptance-windows` env wiring, run recipe), `scripts/platynui-robot-session.sh` (Linux lane wiring), `pyproject.toml` (`[tool.mypy] files` entry for `main.py`; PySide6 itself is already in the dev group — no new dependency), fixture-app doc pointers.
- **No Rust or Python library code changes, no native rebuild** — fixture + test surface only. No BREAKING changes.
- **Platforms**: Windows (UIA) and Linux/X11 (AT-SPI) acceptance from the start, reusing the Qt Widgets session mechanics; Wayland inherits the same caveats as the Qt Widgets lane (no client-side global coordinates). macOS out of scope.
- **Depends on**: `test-app-blueprint` (catalog, names, suite conventions — this change is its first proof; friction found here feeds spec adjustments back before both are archived). **Unblocks**: catalog onboarding for every later fixture (`add-swt-test-app`, `add-javafx-test-app`, WPF/Avalonia/Win32, retrofits), and the QML row of the technology matrix the semantic-proxy work will build on.
