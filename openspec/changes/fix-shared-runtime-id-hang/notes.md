# Verification notes

Measured on the Windows 11 VM on 2026-11-25 against KeePass 2.62 with its "Add Entry" dialog (`PwEntryForm`) open, using the repo build of this change (Release `platynui_native`). Each query ran in its own process with a 20-second hard timeout. Most of the roughly 2 seconds per query is the runtime's start-up in that process.

| Query | `0.13.0.dev301` and `main` before the change | With the change |
|---|---|---|
| all matches of `…//Pane[@Id='m_tabEntry']/Edit[@Id="m_tbTitle"]` | hung | 2.6 s, 1 item |
| first match of `…//Pane[@Id='m_tabEntry']//Edit[@Id="m_tbUsername"]` (wrong id) | hung | 2.6 s, none |
| `.//Edit[@Id="m_tbUsername"]` from the KeePass application node | hung | 2.7 s, none |
| `…/Tab[@Id="m_tabMain"]/*` | hung | 2.0 s, 6 items (the page and the five headers) |
| first match of `…/Tab[@Id="m_tabMain"]/TabItem[@Name="Properties"]` | hung | 2.0 s, `uia://app/3116/2a.e062c/child:3` |
| first match of `…/Tab/Pane[@Id="m_tabEntry"]/Edit[@Id="m_tbUserName"]` | hung | 2.3 s, found |
| all matches of `…/Window[@Id='PwEntryForm']//Edit` | hung | 2.6 s, 5 items |
| `count(…/Window[@Id='PwEntryForm']//*)` | hung | 2.6 s, `39` |

The five tab headers now carry `uia://app/3116/2a.e062c/child:1` to `…/child:5`, in tab order.

The demo suite `C:\develop\keepass_demo\tests\first.robot` was run unchanged with the repo build. It no longer hangs:

- The title is typed.
- The misspelled id `m_tbUsername` (the dialog's field is `m_tbUserName`) fails with `ElementNotFoundError` after its 30-second timeout.
- The whole run ends after 35 seconds.

The remaining failures of that suite are its own:

- Its `Set Root` is LOCAL, so it does not reach the nested `Set Edit Value` keyword, which therefore searches the whole application.
- It uses several ids and roles that the dialog does not have: `m_tbNotes` is `Document` `m_rtNotes`, and `m_tbExpires` is `ComboBox` `m_dtExpireDateTime`.

Other results:

- The full Windows lane `just test-acceptance-windows` passed on the VM: 165 of 165 Robot tests, the new `tests/acceptance/winforms` suite included, after the Rust and Java parts of the recipe.
- `just check`, `just test` (2,741 tests), `just test-python` (979 tests) and `just cross-target-checks` passed on the host. In one of two `just test` runs, `platynui-wayland-compositor::ipc_tests ipc_window_at_point_skips_the_callers_own_window` failed under full parallel load. It passed three times on its own and in the second full run, so it is a timing flake that this change does not touch.
