# text-input-policy Specification

## Purpose
TBD - created by syncing change remove-programmatic-set-text. Update Purpose after archive.
## Requirements
### Requirement: Text entry is keyboard-driven
Setting the text of an editable element SHALL happen through synthesized keyboard input (focus, select-all, type) at the client layer. Providers SHALL NOT expose programmatic text-write actions through the backing accessibility APIs (e.g. JAB `setTextContents`, UIA `ValuePattern.SetValue`, AT-SPI text mutation, in-process agent writes).

#### Scenario: No set-text action from any provider
- **WHEN** `pattern_by_name(TextEditable)` is called on any provider node
- **THEN** no action instance with a text-write operation is returned

#### Scenario: Client-side text entry
- **WHEN** the Python proxy layer sets the text of an editable field
- **THEN** it performs focus/click, select-all, and typing via the input devices, and the resulting `control:Text` reflects the typed value

### Requirement: TextEditable is a capability marker
The `TextEditable` pattern SHALL be advertised in `SupportedPatterns` purely as a capability marker for elements that genuinely accept text input, accompanied by editability metadata (`IsReadOnly`; `MaxLength` where available). Advertising the pattern without an action instance is contract-conform; the core pattern vocabulary SHALL NOT contain a programmatic set-text trait.

#### Scenario: Marker without action
- **WHEN** an editable text field advertises `TextEditable`
- **THEN** clients can derive editability from the marker and `IsReadOnly` = false, while `pattern_by_name(TextEditable)` yields no instance

#### Scenario: Read-only field is not marked
- **WHEN** a text-bearing element rejects edits (read-only state)
- **THEN** `TextEditable` is absent from its `SupportedPatterns` and `IsReadOnly` = true is exposed
