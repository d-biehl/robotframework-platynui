---
name: suggestNextWorkItems
description: Audit project documentation against actual code to find gaps and identify next work items.
argument-hint: Look if there is a `docs` path in the workspace, if yes use this or specify the documentation folder or specific doc files to audit against the codebase.
---
Perform a comprehensive audit of the project by comparing its documentation, requirements, and plans against the actual codebase implementation.

Follow these steps:

1. **Read all documentation**: Read every file in the specified documentation folder. Pay close attention to architecture descriptions, planning/roadmap documents, requirement specifications, and design decisions.

2. **Extract expected state**: From the documentation, extract:
   - What components/modules should exist
   - What features are described as implemented vs. planned vs. stubbed
   - What design decisions have been made but not yet implemented
   - Task checklists and their stated completion status

3. **Analyze the code**: For each documented component, verify in the actual codebase:
   - Does the code exist and match the documented structure?
   - Are documented features actually implemented or just stubs/scaffolds?
   - Do interfaces/traits/APIs match what the documentation specifies?
   - Are checklist items marked as done actually present in code?

4. **Identify discrepancies**: Report any mismatches between documentation and code:
   - Features documented as complete but missing or incomplete in code
   - Code that exists but isn't reflected in documentation
   - Decided designs with no implementation started
   - Stubs documented as full implementations (or vice versa)

5. **Prioritize next steps**: Based on the gaps found, recommend where to continue work:
   - Group items by priority (high/medium/low) based on architectural impact and dependency chains
   - Identify which items are blockers for other planned work
   - Suggest the most impactful next work item with rationale
   - Note items that can be worked on in parallel vs. sequentially

Present the results as a structured summary with tables for clarity, clearly separating what's done, what's partially done, and what's missing.
