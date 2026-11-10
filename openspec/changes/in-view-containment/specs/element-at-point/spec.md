# Spec Delta

## MODIFIED Requirements

### Requirement: Hit-test does not return hidden nodes

Hit-test SHALL NOT return a node that is not actually shown to the user, even when the node's bounds geometrically contain the point. A node is excluded when it reports `Control:IsVisible` as explicitly false; a node that does not report the attribute at all SHALL be treated as visible (so a provider that does not model visibility is unaffected). This matters because the accessibility tree routinely retains laid-out but hidden nodes with stale bounds (a closed menu's items, a non-current stacked page, a collapsed panel's contents), and such a node SHALL NOT be picked in preference to the visible node the user actually sees at the point. When a hidden node contains the point, the resolver SHALL fall through to the visible node beneath or around it.

`Control:IsInView` SHALL NOT exclude a node. It is `false` for a node that is only partly in view (capability `in-view-attribute`), and such a node is shown where it is not clipped, so the user can see it at the point.

#### Scenario: A hidden node containing the point is skipped

- **WHEN** hit-test is called with a point inside a node whose bounds contain it but which reports `Control:IsVisible` as false
- **THEN** the provider SHALL NOT return that node, and SHALL instead return the visible node beneath/around the point (or nothing if none)

#### Scenario: A hidden topmost sibling falls through to the visible one beneath

- **WHEN** two siblings overlap at the point and the frontmost (topmost) one is hidden
- **THEN** the provider SHALL return the visible sibling beneath it, not the hidden frontmost one

#### Scenario: A visible node that is not fully in view is returned

- **GIVEN** a node that reports `Control:IsVisible` as true and `Control:IsInView` as false, and whose bounds contain the point
- **WHEN** hit-test is called with that point
- **THEN** the provider SHALL return that node
- **NOTE:** Verified with the mock provider, which reports `IsInView` as declared.
