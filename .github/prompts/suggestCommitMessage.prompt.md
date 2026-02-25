---
name: suggestCommitMessage
description: Suggest a Conventional Commits message for the changes made in this session.
argument-hint: Optional scope or additional context for the commit message
---
Review the entire conversation to identify all code changes, file creations, and documentation edits made during this session. Based on those changes, suggest a well-structured commit message following the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) specification.

Follow these steps:

1. **Review the session**: Go through the conversation history and collect all files that were created, modified, or deleted during this session. Note the purpose and intent behind each change.
2. **Determine the commit type**: Choose the appropriate type (`feat`, `fix`, `refactor`, `docs`, `chore`, `test`, `perf`, `style`, `build`, `ci`) based on the nature of the changes.
3. **Determine the scope**: Identify the most appropriate scope from the affected modules, crates, or packages.
4. **Compose the message**:
   - **Subject line**: `type(scope): imperative summary` — max 72 characters, present tense, no period.
   - **Body**: Focus on the *purpose* and *business value* of the changes — what capability was added, what problem was solved, or what behavior changed. Avoid listing implementation details like file names, function names, or struct names. Use bullet points for multiple logical changes. Wrap lines at 72 characters.
   - **Footer**: Reference related issues or breaking changes if applicable.
5. **Present the result** as a fenced code block so the user can copy it directly.

Guidelines:
- Base the message on the changes made within this conversation, not on the full git diff.
- Write from a *functional* perspective: describe what users, developers, or the system can now do differently — not which files or types were touched.
- The subject line should read like a feature announcement or changelog entry, not a code diff summary.
- Keep the subject concise and action-oriented.
- If changes span multiple scopes, pick the primary one for the subject and mention others in the body.
- Group related changes logically in the body by functional area, not by file.
