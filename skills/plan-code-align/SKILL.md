---
name: plan-code-align
description: "Repo-specific workflow for PlatynUI to read documentation/plans, compare against code, surface gaps, provide code review and optimization findings, and propose a prioritized next-task list. Use when asked to align docs/plans with implementation or decide what to build next."
---

# Plan-Code Alignment (PlatynUI)

## Scope defaults
- Use this skill for the PlatynUI repo only.
- Default plan docs to compare:
  - `docs/architekturkonzept_runtime_umsetzungsplan.md`
  - `docs/linux_x11_implementation_plan.md`
- Load supporting context only as needed:
  - `docs/architekturkonzept_runtime.md`
  - `docs/patterns.md`
  - `docs/provider_checklist.md`
  - `docs/cli_snapshot_spec.md`

## Workflow
1) Confirm scope briefly
- Ask only if the user wants additional docs or a narrower code area.

2) Read plans and extract tasks
- Parse checkboxes `[x]`/`[ ]` and any dated status notes.
- Build a simple checklist: Task, Expected behavior, Evidence, Status (Implemented/Partial/Missing/Unclear).

3) Map plan items to code
- Identify relevant crates and entry points.
- Use fast scans first:
  - `rg --files docs` for doc inventory.
  - `rg --files crates` and `rg -n 'TODO|FIXME|NotReady|unimplemented!|todo!' crates` for signals.
- Verify claimed implementations with concrete symbols, APIs, or tests; cite file paths in findings.

4) Review code for risks and optimization potential
- Focus on the areas tied to the plans.
- Prioritize bugs, behavioral gaps, error handling, test coverage, performance, and API consistency.

5) Synthesize next tasks
- Propose a prioritized list with: target crate/module, expected outcome, tests/validation.
- Keep tasks actionable and scoped (1–3 day chunks where possible).

## PlatynUI-specific map (use selectively)
- Linux/X11 devices: `crates/platform-linux-x11/*`
- AT-SPI provider: `crates/provider-atspi/*`
- Runtime glue: `crates/runtime/*`
- CLI commands: `crates/cli/src/commands/*`
- Linking macros: `crates/link/src/lib.rs`

## Output structure (default)
- Plan alignment summary (what matches vs. missing)
- Gap list (plan items not implemented, with evidence)
- Code review findings (ordered by severity, file refs)
- Optimization potential (performance, UX, DX, test gaps)
- Recommended next tasks (prioritized, with test notes)
- Questions/assumptions (only if needed)

## Notes
- Do not claim “fully reviewed” if only a subset was examined; state coverage.
- If the plan or docs include dates, repeat the concrete date when clarifying status.
- Prefer `rg` for search and `sed -n` or `nl -ba` for precise references.
