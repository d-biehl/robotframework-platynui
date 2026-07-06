## ADDED Requirements

### Requirement: As-you-type completion suggests matching history entries

While the XPath search field has focus and contains a non-empty query, the Inspector SHALL show a completion popup below the field listing history entries that match the current input. Matching SHALL be case-insensitive. Suggestions SHALL be ordered most-recent-first (their history order), and the current query itself SHALL NOT be offered as its own suggestion. The completion source for this change is the persisted history only (see `inspector-xpath-history`); other sources (XPath vocabulary, live tree names) are out of scope.

#### Scenario: Typing shows matching suggestions

- **WHEN** the field has focus and the user has typed text that is a case-insensitive match of one or more history entries
- **THEN** a completion popup SHALL appear below the field listing the matching entries, newest-first

#### Scenario: No matches hides the popup

- **WHEN** the current input matches no history entry
- **THEN** no completion popup SHALL be shown

#### Scenario: Empty field shows no completion popup

- **WHEN** the search field is empty
- **THEN** no completion popup SHALL be shown (browsing all history is done through the history dropdown, not the completion popup)

### Requirement: Completion is keyboard-navigable and accepting fills the field

When the completion popup is open, the Up and Down arrow keys SHALL move the highlighted suggestion, Enter or Tab SHALL accept the highlighted suggestion by replacing the field contents with it and closing the popup, and Escape SHALL dismiss the popup without changing the field. Moving the highlight SHALL stay within the list bounds (or wrap) and SHALL never index outside the current suggestions, including when the suggestion list shrinks as the user keeps typing.

#### Scenario: Arrow keys move the highlight

- **WHEN** the completion popup is open and the user presses Down (or Up)
- **THEN** the highlighted suggestion SHALL move to the next (or previous) entry within the list bounds

#### Scenario: Accepting a suggestion fills the field

- **WHEN** a suggestion is highlighted and the user presses Enter or Tab
- **THEN** the field SHALL be set to the highlighted suggestion and the popup SHALL close

#### Scenario: Escape dismisses without changing the field

- **WHEN** the completion popup is open and the user presses Escape
- **THEN** the popup SHALL close and the field contents SHALL be unchanged

#### Scenario: Highlight stays valid as suggestions change

- **WHEN** the user keeps typing so the suggestion list shrinks below the current highlight index
- **THEN** the highlight SHALL be clamped to a valid entry (or cleared if the list becomes empty) and SHALL never point outside the list

### Requirement: Completion cooperates with existing search-field behavior

Adding completion SHALL NOT break the search field's existing behavior. When the completion popup is closed (or no suggestion is highlighted), Enter SHALL evaluate the current expression exactly as it does today; when the popup is open with a highlighted suggestion, Enter SHALL accept the suggestion instead of evaluating. Escape SHALL dismiss the popup when it is open; when the popup is closed, Escape SHALL retain its current meaning (cancelling an in-flight search). Selecting an entry from the history dropdown SHALL NOT immediately reopen the completion popup for the just-filled text.

#### Scenario: Enter still evaluates when no completion is active

- **WHEN** the completion popup is closed and the user presses Enter in the field
- **THEN** the current expression SHALL be evaluated as it is today

#### Scenario: Enter accepts the suggestion when completion is active

- **WHEN** the completion popup is open with a highlighted suggestion and the user presses Enter
- **THEN** the suggestion SHALL be accepted into the field and no evaluation SHALL be triggered by that keypress

#### Scenario: Escape precedence between completion and search cancel

- **WHEN** the user presses Escape while the completion popup is open
- **THEN** the popup SHALL close and any in-flight search SHALL continue; a subsequent Escape (popup already closed) SHALL cancel an in-flight search as it does today
