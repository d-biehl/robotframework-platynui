## 1. Scaffolding

- [x] 1.1 Create `apps/test-app-swing/` with `src/platynui/testapp/` layout and add `apps/test-app-swing` to the root `Cargo.toml` workspace `exclude` (next to `apps/test-app-qt`); verify `cargo metadata` still resolves
- [x] 1.2 Write `apps/test-app-swing/README.md`: purpose (JAB provider fixture), build/run instructions, the per-process enablement flag (and `jabswitch` as manual fallback), accessible-name discipline, staged-growth rules (existing names never change)

## 2. Swing app

- [x] 2.1 Implement `Main.java`: hand-rolled argument parsing for `--title` (default "PlatynUI Swing TestApp"), `--auto-close N`, `--dialogs N` / `--open-modal` (accepted no-ops for now); unknown args print usage and exit non-zero; `--auto-close` uses a Swing timer calling `System.exit(0)`
- [x] 2.2 Implement stage 1 (`Stage1Panel.java` + frame wiring): menu bar (File→Exit, Help→About), push button, single-line text field, status label — each with explicit unique `accessibleName`; frame title doubles as its accessible name
- [x] 2.3 Implement the click observable: button click increments a counter and sets the status label's text and accessible name to end with `clicks-<n>`
- [x] 2.4 Implement stage 2 (`Stage2Panel.java`, titled panel): checkbox, radio group (≥2), combo box, slider, spinner, progress bar — each with explicit unique `accessibleName`
- [x] 2.5 Review pass against the spec: no interactive control without a name, no duplicate names, no Windows-specific code

## 3. just recipes

- [x] 3.1 Add `build-test-app-swing`: fail fast with a clear message when `javac` is missing; compile `src/**/*.java` with `-encoding UTF-8 -source 8 -target 8` into `build/classes` (works in PowerShell on Windows and sh on Linux, following existing per-OS recipe patterns in the justfile)
- [x] 3.2 Add `run-test-app-swing *ARGS`: depends on the build recipe; on Windows launches `java -Djavax.accessibility.assistive_technologies=com.sun.java.accessibility.AccessBridge -cp build/classes platynui.testapp.Main {{ARGS}}`; on Linux launches without the Windows flag (ATK enablement documented in the README for the future lane)
- [x] 3.3 Manual verification of the CLI spec scenarios: default title, `--title`, `--auto-close 5` exits 0, `--bogus` exits non-zero with usage; confirm no `.accessibility.properties` is created

## 4. JAB spike (crates/playground, throwaway)

- [x] 4.1 Add a `jab_spike` bin target to `crates/playground` (Windows-only via `cfg`), with `libloading` and the needed `windows` crate features; item-level `#[allow(unsafe_code)]` per workspace lint policy
- [x] 4.2 Define the minimal FFI surface from the local headers (`include\win32\bridge`): `JOBJECT64`=`i64`, `AccessibleContextInfo` (`#[repr(C)]`, fixed UTF-16 arrays, trailing interface bitfield), function pointer types for `Windows_run`, `isJavaWindow`, `getAccessibleContextFromHWND`, `getAccessibleContextInfo`, `getAccessibleChildFromContext`, `getAccessibleParentFromContext`, `releaseJavaObject`, `isSameObject`, `getVersionInfo`
- [x] 4.3 Implement DLL discovery (`PLATYNUI_JAB_DLL` → `%JAVA_HOME%\jre\bin` → `%JAVA_HOME%\bin` → `PATH`) and bind the lowercase cdecl exports
- [x] 4.4 Implement the dedicated JAB thread: load+bind, call `Windows_run()`, run a Win32 message pump, service tree-walk requests from the main thread via a channel
- [x] 4.5 Implement discovery + dump: bounded retry loop (pump between attempts) until `EnumWindows`+`isJavaWindow` finds the fixture app; walk the tree via `getAccessibleContextInfo`/`getAccessibleChildFromContext`, print role/`role_en_US`/name/states/bounds/`indexInParent` per node with indentation; release every handle (`releaseJavaObject`), walk twice to exercise release discipline
- [x] 4.6 Run the spike against the fixture app and record the validation checklist from the design: rendezvous timing, struct-layout sanity, verbatim role strings for every stage-1/2 control, DPI/bounds behavior on a scaled monitor, release-discipline observations

## 5. Verification & handover

- [x] 5.1 Run `just check` and `just test` to confirm the workspace (including the new playground bin) is clean and the exclude works
- [x] 5.2 Write the spike findings into a findings section prepared for `add-jab-provider` (role-string table, DPI result, timing numbers, any surprises); mark the spike explicitly as throwaway in the playground source header
