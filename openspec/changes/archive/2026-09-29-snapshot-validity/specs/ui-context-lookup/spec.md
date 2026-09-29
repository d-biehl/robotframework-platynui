# Spec Delta

## Purpose

How a context of the `PlatynUI.core` object model looks up its element, and when it reads the current UI again instead of asking the runtime's snapshot, so that the high-level library follows the same snapshot protocol as BareMetal.

## ADDED Requirements

### Requirement: A failed attempt is retried against the current UI

When a context retries — because its element was not found or is no longer valid, or because a condition it waits for did not hold or raised — the library SHALL discard the runtime's snapshot before the next attempt, so that the retry reads the current UI instead of asking the same snapshot again.

#### Scenario: An element that appears after the first attempt is found

- **GIVEN** a context whose element does not exist when the lookup starts
- **WHEN** the element appears before the lookup's timeout
- **THEN** `exists()` SHALL return `True`, and the snapshot SHALL have been discarded before the attempt that found the element
- **NOTE:** Exercised with stubs for the lookup and the runtime. The mock tree is static and cannot add an element during a lookup.

#### Scenario: Every retry reads the current UI

- **GIVEN** a context whose lookup fails three times before its timeout
- **WHEN** the lookup runs
- **THEN** the snapshot SHALL be discarded before each of the retries

#### Scenario: A failed condition is retried against the current UI

- **GIVEN** a context that waits for a condition on its element, such as being enabled, and the condition fails at first
- **WHEN** the context retries
- **THEN** the snapshot SHALL be discarded before the retry

### Requirement: A lookup that gives up leaves no snapshot behind

When a context's lookup or check gives up — at its timeout, or at once for a lookup without retries that finds nothing — the library SHALL discard the runtime's snapshot before it returns or raises, so that the next call reads the current UI.

#### Scenario: An existence check that gives up

- **GIVEN** a context whose element does not exist
- **WHEN** `exists()` gives up at its timeout and returns `False`
- **THEN** the snapshot SHALL have been discarded, and a later `exists()` SHALL find the element if it has appeared meanwhile

#### Scenario: A single lookup that finds nothing

- **GIVEN** a context with no matching element
- **WHEN** a lookup without retries for one or for all matches finds nothing
- **THEN** the lookup SHALL report that nothing was found, as it does today, and the snapshot SHALL have been discarded

### Requirement: A lookup that succeeds keeps the snapshot

The library SHALL NOT discard the snapshot after a lookup that found its element, so that successive successful lookups can reuse the snapshot, as the snapshot model intends.

#### Scenario: Successive successful lookups share a snapshot

- **GIVEN** two contexts whose elements both exist
- **WHEN** both are looked up one after the other
- **THEN** the snapshot SHALL NOT be discarded between or after the two lookups
