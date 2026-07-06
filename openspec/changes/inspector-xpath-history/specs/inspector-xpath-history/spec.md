## ADDED Requirements

### Requirement: Successfully evaluated XPath expressions are recorded to a history

The Inspector SHALL record an XPath expression into its search history only when that expression **evaluates successfully** — i.e. it compiles and evaluates without a parse or query error. Evaluation is triggered via Enter in the search field, the Search button, or the Ctrl/Cmd+Enter shortcut. The expression SHALL be trimmed of surrounding whitespace before recording, and empty or whitespace-only expressions SHALL NOT be recorded. An expression that fails to evaluate SHALL NOT be recorded. A valid expression that matches nothing is a success (not a failure) and SHALL be recorded.

#### Scenario: A successfully evaluated expression is added to history

- **WHEN** the user submits a non-empty XPath expression that evaluates without error
- **THEN** that expression (trimmed) SHALL be present in the history as the most-recent entry

#### Scenario: Empty submissions are ignored

- **WHEN** the user submits an empty or whitespace-only expression
- **THEN** the history SHALL be unchanged

#### Scenario: A failing expression is not recorded

- **WHEN** the user submits a non-empty expression that fails to evaluate (syntax or query error)
- **THEN** the history SHALL be unchanged, so only expressions that actually worked are kept

#### Scenario: A valid expression with no matches is still recorded

- **WHEN** the user submits a syntactically valid expression that evaluates without error but matches no nodes
- **THEN** that expression SHALL be recorded as the most-recent entry

### Requirement: History is de-duplicated with most-recent-first ordering

The history SHALL be ordered most-recent-first and SHALL NOT contain duplicate expressions. When an expression already present in the history is submitted again, its existing occurrence SHALL be removed and the expression SHALL be inserted at the front, rather than adding a second copy.

#### Scenario: Re-submitting an existing expression moves it to the front

- **WHEN** the user submits an expression that is already in the history at a lower position
- **THEN** the history SHALL contain that expression exactly once, at the front, with the relative order of the other entries preserved

#### Scenario: Newest entries come first

- **WHEN** the user submits several distinct expressions in sequence
- **THEN** the history SHALL list them newest-first, with the most recently submitted expression at the front

### Requirement: History is capped at 100 entries

The history SHALL retain at most 100 entries. When recording a new entry would exceed 100, the oldest entry (at the back) SHALL be dropped so the length never exceeds 100.

#### Scenario: The cap drops the oldest entry

- **WHEN** the history already holds 100 distinct entries and the user submits a new distinct expression
- **THEN** the history SHALL contain 100 entries, the new expression SHALL be at the front, and the previously-oldest entry SHALL no longer be present

#### Scenario: De-duplication does not shrink below the cap unnecessarily

- **WHEN** the history holds 100 entries and the user re-submits one already present
- **THEN** the history SHALL still contain 100 entries, with that expression moved to the front and no entry dropped

### Requirement: History persists across restarts

The history SHALL be stored on disk under the operating system's per-user config directory, namespaced to the Inspector, so that it survives closing and reopening the application. The Inspector SHALL load the stored history once at startup and SHALL save the history to disk after each recorded expression, so history is not lost on a non-graceful exit.

#### Scenario: History survives a restart

- **WHEN** the user submits expressions, then closes and reopens the Inspector
- **THEN** the previously recorded expressions SHALL be present in the history, in the same most-recent-first order

#### Scenario: Save happens on record, not only on clean exit

- **WHEN** an expression is recorded and the Inspector is then terminated without a clean shutdown
- **THEN** the recorded expression SHALL still be present in the stored history on the next launch

### Requirement: Loading tolerates a missing, corrupt, or oversized history file

Startup SHALL NOT fail because of history-file problems. A missing file SHALL yield an empty history. A file that cannot be parsed SHALL be treated as an empty history (and the failure logged) rather than crashing or blocking startup. A file that somehow contains more than 100 entries SHALL be clamped to the 100 most-recent on load.

#### Scenario: No history file yet

- **WHEN** the Inspector starts and no history file exists
- **THEN** it SHALL start with an empty history and SHALL NOT report an error to the user

#### Scenario: Corrupt history file

- **WHEN** the Inspector starts and the history file cannot be parsed
- **THEN** it SHALL start with an empty history, log the failure, and continue normally

#### Scenario: Oversized history file is clamped on load

- **WHEN** the Inspector loads a history file containing more than 100 entries
- **THEN** it SHALL keep only the 100 most-recent entries

### Requirement: History is browsable from the search bar

The search bar SHALL provide a control (a dropdown button beside the XPath field) that opens a list of the recent expressions, newest-first. Selecting an entry SHALL load that expression into the search field. The control SHALL also offer a Clear History action that empties the history and removes it from disk. When the history is empty, the list SHALL communicate that there is nothing to show rather than appearing broken.

#### Scenario: Selecting a history entry fills the field

- **WHEN** the user opens the history dropdown and selects a recent expression
- **THEN** the search field SHALL be populated with that expression

#### Scenario: Clearing the history

- **WHEN** the user invokes Clear History
- **THEN** the history SHALL become empty, the dropdown SHALL show no entries, and the stored history on disk SHALL be emptied so the clear survives a restart

#### Scenario: Empty history dropdown

- **WHEN** the user opens the history dropdown while the history is empty
- **THEN** the dropdown SHALL indicate there are no recent expressions rather than showing a blank or malformed list
