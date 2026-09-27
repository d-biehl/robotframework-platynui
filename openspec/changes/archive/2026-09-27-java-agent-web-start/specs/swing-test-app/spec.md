## MODIFIED Requirements

### Requirement: Test-app CLI conventions
The app SHALL support the CLI surface of the existing fixture apps: `--title <text>` (window title, default "PlatynUI Swing TestApp"), `--auto-close <seconds>` (self-terminate for CI), `--dialogs <n>` and `--open-modal` (reserved stage-4 flags that are accepted and, until dialogs exist, act as no-ops so launcher scripts stay stable). Unknown arguments SHALL fail with a usage message.

It SHALL additionally offer the launch modes that reproduce, on a plain JVM, the two conditions a Java Web Start target imposes — so that coverage of them costs no Web Start installation: one that builds the UI inside a **second AWT `AppContext`** while leaving a launcher's furniture in the first, and one that **asserts the fixture is running under a security policy** which trusts the application's own code base and nothing else. Both are opt-in, and the default launch SHALL remain exactly what it is: the acceptance lane must not be able to tell that these modes exist.

A mode that cannot apply SHALL fail the launch rather than fall back to the default shape. This is not defensiveness — a fixture that quietly ran in a single `AppContext`, or quietly ran unsandboxed, would still satisfy every assertion that does not depend on the condition, which is the entirety of what these modes exist to provide. The sandboxed mode in particular SHALL verify its own permissions rather than trust the policy to have applied, because a policy whose `codeBase` matches nothing loads without any error and produces a harsher, different target in which the application itself is sandboxed too.

The sandboxed mode is bound to a JVM that still has a security manager: JEP 486 disabled it permanently in JDK 24, where the launch flag makes the JVM refuse to start. Whoever launches that mode SHALL check this and say so, rather than let a toolchain bump present as a fixture that will not come up.

#### Scenario: Custom title
- **WHEN** the app is started with `--title "My Swing Window"`
- **THEN** the top-level frame's title (and its accessible name) is "My Swing Window"

#### Scenario: Auto-close for CI
- **WHEN** the app is started with `--auto-close 5`
- **THEN** the process exits with code 0 no later than a few seconds after the 5-second deadline without user interaction

#### Scenario: Unknown argument
- **WHEN** the app is started with `--bogus`
- **THEN** it prints a usage message naming the unknown argument and exits with a non-zero code

#### Scenario: The application runs in a toolkit world of its own
- **WHEN** the app is started in its second-`AppContext` mode
- **THEN** the JVM has two AWT `AppContext`s: the application's window lives in one, and a never-shown launcher window in the other — so an observer whose threads are in neither finds the application only by looking across all of them

#### Scenario: A launch mode that cannot apply fails the launch
- **WHEN** the second-`AppContext` mode is asked of a JVM that denies the internal it needs, or the sandboxed mode is launched with a policy that does not actually grant the fixture its permissions
- **THEN** the process exits with a non-zero code naming what was missing, and no window is shown
