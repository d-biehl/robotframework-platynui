# Spec Delta

## Purpose

How the Windows UI Automation provider forms the runtime id of each node, including elements for which UI Automation reports none, so that every UIA node has an id that no sibling shares and that every path to the element reproduces.

## ADDED Requirements

### Requirement: A reported runtime id is kept

When UI Automation reports a non-empty runtime id for an element, the provider SHALL form the node's runtime id from it unchanged: `uia://desktop/<hex>` in the desktop view and `uia://app/<pid>/<hex>` below an application node, where `<hex>` is the reported array in hexadecimal, joined by dots.

#### Scenario: A WinForms tab control keeps the id UI Automation reports

- **GIVEN** the WinForms test window, whose tab control UI Automation reports with the runtime id `[42, <hwnd>]`
- **WHEN** the tab control's `@RuntimeId` is read below its `app:Application` node
- **THEN** it SHALL be `uia://app/<pid>/2a.<hwnd in hex>`
- **NOTE:** Verified against the real provider in the Windows acceptance lane.

### Requirement: An element without a reported runtime id gets one from its parent

When UI Automation reports an empty runtime id for an element, or reading it fails, the provider SHALL form the id from the runtime id of the element's UI Automation parent: `<parent id>/child:<n>` when the element has an MSAA child id `n` greater than 0, otherwise `<parent id>/index:<n>`, where `n` is the element's 0-based position among its parent's children. No two nodes SHALL share an id because UI Automation reported none for them.

#### Scenario: WinForms tab headers get distinct ids from their child ids

- **GIVEN** the WinForms test window, whose five tab headers have an empty runtime id from UI Automation and the MSAA child ids 1 to 5
- **WHEN** the `@RuntimeId` of every `TabItem` below the tab control is read
- **THEN** the ids SHALL be `<tab control id>/child:1` to `<tab control id>/child:5`, in tab order
- **NOTE:** Verified against the real provider in the Windows acceptance lane.

#### Scenario: An element without a child id is numbered by its position

- **GIVEN** an element with an empty runtime id and no MSAA child id, the third child of its parent
- **WHEN** its runtime id is formed
- **THEN** it SHALL be `<parent id>/index:2`
- **NOTE:** Verified by a Windows unit test of the id composition; no fixture reports such an element.

#### Scenario: A failed runtime id read is treated like an empty one

- **GIVEN** an element whose runtime id read fails, with the MSAA child id 4
- **WHEN** its runtime id is formed
- **THEN** it SHALL be `<parent id>/child:4`, and no node SHALL carry `uia://temp`
- **NOTE:** Verified by a Windows unit test of the id composition.

#### Scenario: A parent without a reported id passes its derived id on

- **GIVEN** an element with an empty runtime id and the MSAA child id 2, whose parent also has an empty runtime id and the MSAA child id 1 below an element with the id `uia://app/<pid>/2a.10`
- **WHEN** the element's runtime id is formed
- **THEN** it SHALL be `uia://app/<pid>/2a.10/child:1/child:2`
- **NOTE:** Verified by a Windows unit test of the id composition.

### Requirement: A derived runtime id is the same on every path

A runtime id formed from the parent SHALL be the same for one element whichever way the provider reached it: listed as a child, resolved from a point, or listed again by a later query on a fresh snapshot, as long as the element and its ancestors are unchanged.

#### Scenario: A hit-tested tab header has the id of the listed one

- **GIVEN** the WinForms test window
- **WHEN** the element at the centre of the third tab header is resolved from its point, and the third `TabItem` below the tab control is listed
- **THEN** both SHALL carry the same `@RuntimeId`, `<tab control id>/child:3`
- **NOTE:** Verified against the real provider in the Windows acceptance lane.

#### Scenario: A fresh snapshot gives the tab headers the same ids

- **GIVEN** the runtime ids of the five tab headers read once
- **WHEN** the snapshot is discarded and the ids are read again
- **THEN** each header SHALL carry the same id as before
- **NOTE:** Verified against the real provider in the Windows acceptance lane.

### Requirement: Queries over elements without a reported runtime id end

A query that walks over children whose runtime ids UI Automation did not report SHALL count each of them once and SHALL end: it SHALL return every match, and a query that matches nothing SHALL find nothing rather than run on.

#### Scenario: Every tab header is found

- **GIVEN** the WinForms test window with the tab headers General, Advanced, Properties, Auto-Type and History
- **WHEN** `count(.//Tab[@Id='tabMain']/TabItem)` and `.//Tab[@Id='tabMain']/TabItem[@Name='History']` are evaluated below its `app:Application` node
- **THEN** the count SHALL be `5`, and the History header SHALL be found
- **NOTE:** Verified against the real provider in the Windows acceptance lane.

#### Scenario: A missing element is reported as not found

- **GIVEN** the WinForms test window
- **WHEN** `Wait Until Exists` waits for `.//Edit[@Id='no-such-edit']` below its `app:Application` node with a timeout of 3 seconds
- **THEN** the keyword SHALL fail with `ElementNotFoundError` once the timeout has passed, within 10 seconds of its start
- **NOTE:** Verified against the real provider in the Windows acceptance lane. The 10-second bound only catches a walk that never ends; it is not a performance target.
