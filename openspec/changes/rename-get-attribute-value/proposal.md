## Why

`Get Attribute` returns the value of an attribute, not the attribute. In the library's own vocabulary the two are different things: `Query    …/@Name` returns the attribute itself, with its `.value` and its `.owner()`, and `Wait Until Attribute Value` waits for a value. Calling the reading keyword `Get Attribute Value` says what it returns, and pairs it with the wait: one keyword reads the value, the other waits for it. The library is at `0.13.0-dev`, so the rename is cheapest now, before 1.0.

## What Changes

- Add **`Get Attribute Value`**, which is `Get Attribute` under its new name. Arguments, the typed value it returns, the optional assertion that is checked once, and the wait for the element (query settings and `query_overrides`) stay exactly as they are.
- Deprecate **`Get Attribute`**. It stays as an alias with identical behavior. Its documentation starts with Robot Framework's deprecation marker, names `Get Attribute Value` as the replacement and says that the alias goes away before 1.0. Robot Framework then logs a deprecation warning whenever a suite uses it, and editors show it as deprecated. The Python method `get_attribute` stays for Python callers.
- Record the removal of the alias before 1.0 in the roadmap (`dev-docs/planning.md` §5.3). The removal itself is a later change.
- Move the repository to the new name:
  - all Robot Framework suites and resources, about 260 calls in 45 files;
  - the library documentation, meaning the keyword docs and the introduction sections;
  - `dev-docs/testing-strategy.md` and the robot-test-style skill;
  - the main specs that name the keyword.
- Test the alias at the Python test level (pytest) rather than in a Robot Framework suite, so that no suite run carries a standing deprecation warning.
- Point the open change `xpath-document-order` at the new name. Three scenario lines in its `baremetal-selector-resolution` delta use `Get Attribute`, and archiving that change later would otherwise bring the old name back into a main spec.

Not part of this change:
- the planned keyword names of the high-level `PlatynUI` library in `dev-docs/python-library-design.md`, which is that library's own design;
- the talk transcript under `docs/talks`;
- removing the alias.

## Capabilities

### New Capabilities

- `baremetal-attribute-reading`: `Get Attribute Value` reads one attribute of one element (typed value, optional assertion checked once), and `Get Attribute` remains as its deprecated alias until it is removed before 1.0.

### Modified Capabilities

Only the wording changes in these capabilities: they name the reading keyword, and the name changes. What they require stays the same.

- `baremetal-waiting`: the requirements for `Wait Until Query` and `Wait Until Attribute Value` refer to `Get Attribute` for their arguments, their result and one scenario. The Purpose paragraph names it too; it lies outside the delta and is updated by hand.
- `description-attribute`: the scenario "Description is readable via attribute lookup" gives RF `Get Attribute` as its example.
- `test-app-blueprint`: the scenario "Name verified against the real tree before encoding" names `Get Attribute` as a way to verify a name.
- `qml-test-app`: the requirement "Blueprint-conforming core catalog in QML" names `Get Attribute` for the same purpose.

## Impact

- **Python / Robot Framework:** `src/PlatynUI/BareMetal/__init__.py`. `get_attribute_value` carries the keyword, and `get_attribute` becomes the deprecated alias that shares its implementation. The library documentation moves to the new name: the examples under "Finding elements", "Reading and checking values", "Process attributes", "Waiting explicitly" and "A short example", the docstrings of `Wait Until Query` and `Wait Until Attribute Value`, and one internal docstring.
- **Tests:**
  - the rename in the suites and resources under `tests/BareMetal`, `tests/acceptance` (egui, swing, qt, win32), `tests/PlatynUI/robot` and `tests/playground`;
  - a new pytest module under `tests/PlatynUI` for the alias. It checks that Robot Framework recognizes it as deprecated and names the replacement, and that it behaves exactly like `Get Attribute Value`.
- **Docs and guidance:** `dev-docs/testing-strategy.md`, `.claude/skills/robot-test-style/SKILL.md`, and `dev-docs/planning.md` §5.3 for the planned removal.
- **Specs:** the new `baremetal-attribute-reading`, plus wording in `baremetal-waiting`, `description-attribute`, `test-app-blueprint` and `qml-test-app`. Outside this change, only the `xpath-document-order` delta changes.
- **Rust / native binding:** none, so no native rebuild.
- **Platforms and providers:** the keyword is provider-independent. The acceptance suites on every lane switch to the new name and have to stay green.
- **Compatibility:** not breaking. Existing suites keep working and see a deprecation warning. Removing the alias before 1.0 will be **BREAKING** for suites that still use it; that is a separate change.
