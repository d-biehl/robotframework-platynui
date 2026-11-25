<#
.SYNOPSIS
    A WinForms window with a tab control, for the Windows acceptance lane.

.DESCRIPTION
    Shows one form, probeForm, holding the TabControl tabMain with the pages General, Advanced,
    Properties, Auto-Type and History. Each page holds one TextBox. The script runs on Windows
    PowerShell 5.1, that is on .NET Framework 4.8, where UI Automation reaches the tab headers
    only through its MSAA proxy and reports no runtime id for them: the case of capability
    uia-runtime-id.

    The window closes by itself after -AutoClose seconds, so that a failed teardown leaves no
    process behind, and the script ends once the window is closed.

.EXAMPLE
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File winforms-test-window.ps1 -Title 'PlatynUI WinForms Tabs'
#>
[CmdletBinding()]
param(
    [string] $Title = 'PlatynUI WinForms Test Window',
    [ValidateRange(1, 86400)]
    [int] $AutoClose = 60
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms

$form = New-Object System.Windows.Forms.Form
$form.Name = 'probeForm'
$form.Text = $Title
$form.Width = 480
$form.Height = 320
$form.StartPosition = 'CenterScreen'

$tabs = New-Object System.Windows.Forms.TabControl
$tabs.Name = 'tabMain'
$tabs.Dock = 'Fill'
foreach ($caption in 'General', 'Advanced', 'Properties', 'Auto-Type', 'History') {
    $key = $caption -replace '-', ''
    $page = New-Object System.Windows.Forms.TabPage
    $page.Name = "page$key"
    $page.Text = $caption
    $textBox = New-Object System.Windows.Forms.TextBox
    $textBox.Name = "tb$key"
    $textBox.Left = 12
    $textBox.Top = 12
    $textBox.Width = 240
    $page.Controls.Add($textBox)
    $tabs.TabPages.Add($page)
}
$form.Controls.Add($tabs)

$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = $AutoClose * 1000
$timer.Add_Tick({ $form.Close() })
$timer.Start()

[System.Windows.Forms.Application]::Run($form)
