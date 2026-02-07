# RoboCon Talk — PlatynUI: Cross-Platform Desktop UI Automation for Robot Framework

**Target length:** ~20 minutes (incl. live demo)

**Audience:** Robot Framework users doing desktop UI automation on Windows/Linux/macOS.

**Promise (what they’ll get):** A practical mental model + a repeatable loop (inspect → XPath → highlight/query → Robot keywords) that stays readable and reduces flakiness.

---

## Slide 1 — Title

**On-slide text**

PlatynUI: Cross-Platform Desktop UI Automation for Robot Framework

Windows • Linux • macOS

**Speaker notes (≈ 1:00)**

- I’m [NAME]. I build/test automation and I got tired of writing three versions of the same idea.
- Today is not theory: you’ll see a compact live demo.
- If you only remember one thing: *make the test describe intent; let XPath connect it to UI.*

---

## Slide 2 — Why PlatynUI? (the pain)

**On-slide text**

Desktop UI automation is usually:

- Flaky by default (timing + focus + async UI)
- Hard to debug (“it clicked” but the state didn’t change)
- Locators turn into code (titles, indexes, translations)
- Stabilized with hacks (sleeps + retries + special-cases)
- Small UI changes cause cascading failures

And then your tests become codebases.

**Speaker notes (≈ 1:30)**

- Quick room check (pick one):
  - “Who has ever added a `Sleep` to make a desktop test pass?” (hands)
  - “Who has seen: ‘it clicked’ but the app didn’t change?” (hands)
  - “Who has had a tiny UI change break a whole bunch of tests?” (hands)
- Frame this as *symptoms* your team recognizes:
  - timing/focus flakiness
  - “click happened” ≠ “goal achieved”
  - locators become logic
  - hacks to stabilize (sleeps/retries/special-cases)
  - “a tiny UI tweak breaks 20 tests”

- Optional rapid-fire (if you want one more): “it passes locally, fails in CI”.

---

## Slide 3 — What existed before (and why it doesn’t scale)

**On-slide text**

Three common approaches:

- Image/OCR automation
  - works when there’s no UI tree
  - breaks with theme/DPI/language/animations and “sleep until it looks right”
  - high maintenance: screenshots multiply (OS/theme/DPI/language)

- Native stacks per OS (direct APIs: UIA / AT-SPI / AX)
  - powerful, but fragmented
  - plus different UI technologies (Win32/WPF/WinUI, Qt/GTK, Electron, …)
  - different concepts → different tests

- Generic keyword catalogs
  - lots of tiny “Get/Count/Is…” keywords
  - instead of specifying tests/keywords, you end up *programming* in Robot (IF/ELSE, loops)
  - advanced cases spill into Python glue


**Speaker notes (≈ 2:00)**

- One-liner: “Every approach works — until you want portability, readability, and reliability at the same time.”
- Pixels are a volatile API; OS-native is a power tool with three manuals; keyword catalogs become a maintenance tax.
- Image/OCR maintenance story: screenshots multiply and small UI changes cause broad churn; debugging is often just “image not found”.
- Native stacks story: lots of power, but the UI tree/attributes differ by OS and by UI technology → you end up writing different locators and different edge-case handling per target.
- Keyword catalogs story: you either explode the keyword surface (“Get/Is/Wait for … everything”) or advanced cases spill into custom helpers — and your suite slowly turns into a Python library.
- Concrete picture: “One test team, 5 apps, multiple UI technologies, Windows + Linux → everything diverges.”
- This sets up what PlatynUI tries to do differently.
- Transition line into Slide 4:
  - “So instead of choosing one or maybe all of these trade-offs, we start trying to standardize the *behavior* and the *language*.” of user-like ui automation.

---

## Slide 4 — The behavior (and the language)

**On-slide text**

What does a keyword mean?

Example: `Click`

- What we *expect*: “the app reacts as if a user clicked it”
- What it often *does*: inject one mouse event at one coordinate
- Why it fails: focus/overlays/disabled states/async UI
- Better contract: action = effect you can verify

**Speaker notes (≈ 1:15)**

- Before I explain the project, a quick question: what does a keyword *promise*?
- In many tools, `Click` is defined as “send an input event” — not “achieve an outcome”.
- That’s why you see: “it clicked” but nothing changed.

- Comedy version: “`Click` is like pressing the elevator button.”
  - you did a thing
  - but you’re still standing there… and the doors may or may not open
- A click is a *suggestion*, not a guarantee.
- What we actually need in tests is boring on purpose: actions that describe outcomes.
  - “Open dialog” means: the dialog is open.
  - If it’s not open, the error should tell us what assumption failed.
- Bridge to the next slide: “So let’s unpack what ‘Click’ secretly asks the universe to line up for us.”


---

## Slide 5 — What does “click” require?

**On-slide text**

To “click a button”, you implicitly need:

- The right target (unique locator)
- Visible and not covered (no overlay)
- Enabled and hittable (not disabled/offscreen)
- Correct focus / active window
- Timing stability (UI ready enough)

And you expect an *observable effect* — not just an input event.

**Speaker notes (≈ 1:15)**

- This is why `Click` is a trap word: it hides a whole checklist.
- Most flaky UI tests are really “one of these assumptions was false”.
- What we actually want: “do the thing and prove it happened”.

- In practice, `Click` is a bundle of assumptions: visible, enabled, focused, and ready.
- `Sleep` only ever fixes *one* bullet: “ready enough” — it won’t fix focus or overlays.
- Bridge to the next slide: “So instead of saying ‘Click’, we start naming the outcome: Activate, Open, Select.”

---

## Slide 6 — Is “Click” the right word?

**On-slide text**

Prefer intent-based actions:

- **Activate** = invoke it -> *observable outcome* (dialog opens / text changes, etc.)
- **Focus** = input is ready -> *observable outcome* (caret appears, highlight appears, etc., depending on control)
- **Check** = check the box -> *observable outcome* (checkbox state changes)
- **Select** = an item is selected -> *observable outcome* (item is selected)

Outcome = an app-level state change you can verify.

Keyword = contract + verification.

**Speaker notes (≈ 1:00)**

- If you name the *outcome*, you can verify the outcome.
- This is the core idea behind semantic actions.
- What “outcome” means: an observable, app-level state change you can check.
  - not “we sent an event”
  - but “the UI is now in a different state”
- Important nuance: we’re not saying “never click”.
  - We still use mouse/keyboard under the hood.
  - We just don’t want the test to depend on the *mechanism*.
- “Click” becomes an implementation detail; the keyword names the intent.
- (and yes you also need keywords to simply click things with the mouse — but that’s a different use case, not the default for test steps)
- Bridge: “Once you think like that, you end up with a small set of semantic actions.”

---

## Slide 7 — How a semantic action should work?

**On-slide text**

Pattern: preconditions → perform → postcondition

- Preconditions: window active, element in view, element enabled
- Perform: execute the platform-specific “activate” strategy
- Postcondition: wait until the application is ready again

Result: either a verified outcome — or a clear reason why not.

**Speaker notes (≈ 1:30)**

- This is a *semantic* action: it’s not “send event”, it’s “make the state change safely”.
- Preconditions: make the assumptions explicit (active window, in view, enabled).
- Perform: delegate to a platform/control strategy (the OS/control-specific implementation).
- Postcondition: confirm the app settles / is ready again (often a soft wait).
- Result: failures are actionable (“which assumption failed?”), not random.

---

## Slide 8 — Controls are composed (example: ListBox)

**On-slide text**

Think in *structures*, not widgets.

ListBox is usually composed of:

- Container (ListBox)
- Items (ListItem)
- Selection model (single/multi)
- Scroll viewport (visible subset)
- Optional content inside items (text, icon, checkbox, …)

Semantic actions that follow:

- **Focus** list / **Scroll Into View** item
- **Select** item(s) / **Deselect**
- **Activate** item (Enter / double-click semantics)
- **Open Context Menu** (item)

**Speaker notes (≈ 1:30)**

- This is the mental model: controls are trees with roles and relationships.
- A ListBox is not “one thing” — it’s container + items + selection + scrolling.
- That’s why XPath matters: you address relationships (item inside list, item by label, selected items).
- And that’s why semantic actions matter: “Select item X” is a contract we can verify.

---

## Slide 9 — Controls are composed (example: TextField)

**On-slide text**

TextField is usually composed of:

- Container (the “field”)
- Editable value (text content)
- Caret + selection (cursor, range)
- Optional label / placeholder / hint
- Optional validation state (error, required)
- Sometimes: password/multiline + scroll

Semantic actions that follow:

- **Focus** field
- **Set Value** / **Clear** / **Append**
- **Select Range** (e.g. select all)
- **Submit/Confirm** (Enter semantics)

**Speaker notes (≈ 1:15)**

- “Type text” is not one event either: focus, selection, IME, keyboard layout, timing.
- Intent-based actions let us be explicit: “set value to X” vs “send 12 keystrokes”.
- Verification examples: value equals X, error state not present, next control enabled.

---

## Slide 10 — A baseline set of semantic actions

**On-slide text**

To cover most “standard controls”, you want keywords like:

- **Focus** / **Activate**
- **Open** / **Close**
- **Select** / **Deselect** / **Toggle**
- **Set Value** / **Clear** / **Append**
- **Scroll Into View** / **Scroll** (Up/Down/Page)
- **Expand** / **Collapse**
- **Check** / **Uncheck** (for tristate: **Set Check State**)

Plus the verification pair:

- **Get Attribute/Property** (text/value/selected/enabled/visible)

**Speaker notes (≈ 1:15)**

- This is intentionally small: fewer verbs, each with a clear contract.
- The second half is equally important: actions without attributes/properties still lead to flakiness.
- Transition into the next slide:
  - “And sometimes you don’t want to *do* something — you want to *ask* something: e.g. how many items are in this ListBox?”

---

## Slide 11 — One interface: locate + query

**On-slide text**

XPath 2.0 is one language for both **locate** and **query**:

- Locate elements:
  - `Window[@Name='My App']//control:Button[@Name='OK']`
  - `Window[@Name='My App']//Tree[@Id='mainTree']/item:TreeItem[matches(@Name,'.*Smith') and @selected='true']`
  - `Window[@Name='My App']//ListBox[@Id='members']/item:ListItem[last()]`
- Query UI state:
  - `Window[@Name='My App']//Edit[@native::AutomationId='search']/@Value`
  - `count(Window[@Name='My App']//ListBox[@Id='members']/item:ListItem[contains(@Name,'Smith')])`
  - `Window[@Name='My App']//ListBox[@Id='members']/item:ListItem[1]/@Name`

One language.
Two uses: targeting + assertions.
Less custom glue.

**Speaker notes (≈ 2:00)**

- This is the “creative shortcut”: instead of inventing 30 query keywords, XPath does the heavy lifting.
- XPath lets you express relationships, not just IDs.
- The trick: Robot keywords stay simple, XPath stays expressive.
- The “ask” example is deliberately not a keyword:
  - counting items with certain text is a query, not an action
- Give one concrete “relationship” example out loud:
  - “the OK button in the currently active dialog”
  - or “the edit field next to the ‘Username’ label”

- Bridge to the next slide: “So: this is the model — now here’s the toolkit that tries to ship it.”

---

## Slide 12 — PlatynUI: implementing the model (guardrails)

**On-slide text**

PlatynUI is an **open-source**, Robot Framework-first library + toolset (early alpha):

- Robot Framework keyword library
- CLI + Inspector for inspect/highlight/query
- Born directly out of real customer projects and already in use.
- Currently funded directly through project work.

PlatynUI tries to implement the model from the previous slides:

- Keywords = semantic actions (contract + verification)
- XPath 2.0 = one interface for *locate* + *query*
- Loop = inspect → highlight → query → Robot

**Speaker notes (≈ 1:00)**

- One-liner: “Open-source, Robot Framework-first — built around that locate/query + semantic action model.”
- Mention early alpha honestly.
- Bridge: “Now: what’s already supported today?”

---

## Slide 13 — Platforms + guardrails

**On-slide text**

Platforms + providers (preview):

- Windows (UIA)
- Linux (AT-SPI2)

Planned:

- macOS (AX)
- External providers (e.g. JSON-RPC)

Also:

- Mock providers (for tests)

In real projects (tested against):

- OS: Windows + Linux (incl. embedded Linux machines)
- UI stacks: WPF, WinForms, Avalonia, JavaFX, Qt 6, GTK

The guardrails:

1) Portability across desktop platforms
2) Readable tests that stay maintainable
3) Reliability habits baked in

**Speaker notes (≈ 1:45)**

- Supported today: Windows (UIA) + Linux (AT-SPI2).
- Planned next: macOS (AX).
- External providers (JSON-RPC) are for integrating other backends; mock providers are for fast, deterministic tests.
- Guardrails = what we optimize for; this is why the API surface stays smaller.

---

## Slide 14 — Live demo: “Let’s talk to a desktop”

**On-slide text**

Live demo (~6 minutes)

1) Inspect the UI tree
2) Craft one XPath locator + one XPath query
3) Highlight it (so we trust it)
4) Run a compact Robot flow

**Speaker notes (≈ 0:45)**

- The demo is intentionally boring and repeatable.
- If you can repeat this workflow on your app, you can scale it.

---

## Slide 15 — Demo Runbook (speaker-notes slide)

**On-slide text**

Demo checklist

- Inspect → XPath → highlight → query → Robot
- Show one semantic action + one “ask” query

**Speaker notes (≈ 6:00)**

Pick an app that is stable on your demo OS and has 1–2 obvious controls (input + button). The goal is to show the *workflow*, not to impress with complexity.

### Tooling options

**GUI inspector (best for stage):**

- Start: `platynui-inspector`
- Show: tree, selection details, and the auto-highlight on selection.

**CLI (great backup, works headless-ish):**

- `platynui-cli list-providers`
- `platynui-cli window --list`
- `platynui-cli snapshot "//control:Window" --pretty`
- `platynui-cli query "//control:Button[@Name='OK']"`
- `platynui-cli highlight "//control:Button[@Name='OK']" --duration-ms 1200`

If you run from the repo without installing preview packages, you can also do:

- `cargo run -p platynui-cli -- window --list`

### Demo story (make it feel like a user action)

Pick one short “user story”, e.g.:

- “Search a setting, confirm it’s visible.”
- “Enter text, press a button, verify a label changes.”

### Demo beats (what to narrate)

1) **Inspect:** “This is the accessibility tree PlatynUI sees.”
2) **Craft XPath:** “Start broad, then make it stable.”
  - broad: `//control:Button`
  - stable: `//control:Button[@Name='OK']` or using IDs if available
3) **Spotlight (highlight):** “Before I automate it, I verify I’m targeting the right thing.”
4) **Ask one question (query):** existence/count/text/value.
5) **Run Robot flow:** show 4–6 steps, including a semantic action and a wait.

### If something fails live

- Switch to CLI highlight/query (fast feedback, no UI clicking).
- Narrate it as a lesson: “We verify effects and wait for conditions—this is why.”

---

## Slide 16 — Adopt without disruption

**On-slide text**

Adopt incrementally

- Keep your suite
- Add one PlatynUI flow
- Measure reliability
- Expand where it pays off

**Speaker notes (≈ 1:45)**

- The adoption model is “drop-in”, not “rewrite”.
- The best first target is the flow that currently burns the most time in re-runs.

---

## Slide 17 — Reliability habits (the boring superpower)

**On-slide text**

Flakiness reducers

- Verify effects (semantic actions)
- Prefer stable attributes in XPath
- Wait for UI conditions, not time
- Keep tests declarative (don’t build a second app in Robot)

**Speaker notes (≈ 1:30)**

- These habits work even if you never use PlatynUI.
- PlatynUI just nudges you into them by design.

---

## Slide 18 — Outlook: what will get sharper next

**On-slide text**

Where PlatynUI is heading

- Stabilize keywords + ergonomics
- Increase cross-platform parity
- Better inspection and diagnostics
- Clear “production-ready” checklist

**Speaker notes (≈ 1:30)**

- Keep it honest: early alpha / evaluation stage.
- Ask for the kind of feedback you want: “what UI patterns do you need most?”
- Optional (if someone asks “why Rust?”): shared Rust core for portability + determinism.

---

## Slide 19 — Wrap-up / Call to action

**On-slide text**

Takeaways

- One cross-platform mental model
- XPath 2.0 = locate + query
- Semantic actions + condition-based waits = less flakiness

Try it / follow along:

- github.com/d-biehl/robotframework-platynui

**Speaker notes (≈ 1:00)**

- Close with: “Make intent readable; make targeting testable.”
- Invite people to try it on one flow and report what breaks.
- Q&A.

---

# Appendix (optional)

## Timing suggestion (20 minutes)

- Slides 1–6 (why + before + click semantics): ~8:00
- Slides 7–11 (semantic actions + XPath): ~5:00
- Slides 12–13 (PlatynUI + guardrails): ~1:00
- Slides 14–15 (demo): ~6:00
- Slides 16–19 (adoption + close): ~1:00

## Pre-talk setup checklist

- Confirm demo works on the venue laptop/OS.
- Disable notifications.
- Prepare two terminals:
  - one for tooling (inspector/CLI)
  - one for running Robot
- Have a “backup recording” or pre-run output ready (even just console output).

## Stage-friendly micro-lines (optional)

- “Desktop automation fails where humans don’t notice: focus, timing, and intent.”
- “XPath is our map: not just *where* something is, but *how it relates*.”
- “Highlight is the sanity check: trust, but verify.”
