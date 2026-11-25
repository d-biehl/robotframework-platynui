# PlatynUI WinForms test window

A WinForms window with a tab control, for the Windows acceptance lane.

This is a **helper, not a fixture** of the test-app blueprint in
[`dev-docs/testing-strategy.md`](../../dev-docs/testing-strategy.md) §5:
- it has no control catalog and follows no blueprint;
- it holds one tab control, so that the suite
  [`tests/acceptance/winforms`](../../tests/acceptance/winforms) can check how the
  UI Automation provider names elements that UI Automation itself gives no runtime id
  (capability `uia-runtime-id`).

## Why WinForms on .NET Framework

The tab headers of a WinForms `TabControl` on .NET Framework have no window of their
own. UI Automation reaches them only through its MSAA proxy, as MSAA child ids 1 to 5
of the tab control, and `GetRuntimeId` returns an empty array for each of them. Real
applications show the same, KeePass 2.x among them. Windows PowerShell 5.1 runs on
.NET Framework 4.8, which every Windows 10 and 11 machine has, so the window needs no
build and no SDK.

## Running it

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File apps\winforms-test-window\winforms-test-window.ps1 [-Title <text>] [-AutoClose <seconds>]
```

The form `probeForm` holds the tab control `tabMain` with the pages General, Advanced,
Properties, Auto-Type and History (`pageGeneral` … `pageHistory`). Each page holds one
`TextBox` (`tbGeneral` … `tbHistory`). The window closes by itself after `-AutoClose`
seconds (default 60), so that a failed teardown leaves no process behind.

Like the other non-Rust apps, the directory is excluded from the Cargo workspace.
