# Spec Delta

## Purpose

The user documentation website teaches Robot Framework users how to automate desktop applications with PlatynUI. This capability fixes where the site lives and how it is built, how it is organized, how its examples stay correct, and how it is published, so that content can grow page by page on a stable frame.

## ADDED Requirements

### Requirement: Site project and toolchain
The user documentation SHALL be an Astro project with the Starlight theme in `docs/`, with its pages as Markdown or MDX files in the project's content directory. The Node.js toolchain SHALL be confined to `docs/`: the repository-wide gates (`just check`, `just test`, the native build) SHALL NOT need Node.js. A `just` recipe SHALL build the static site, and another SHALL serve it locally with live reload. Built output SHALL NOT be committed.

#### Scenario: Build from a clean checkout
- **GIVEN** a clean checkout and Node.js installed
- **WHEN** the docs build recipe runs
- **THEN** it installs the locked dependencies, builds the static site into the docs project's output directory, and exits with code 0

#### Scenario: Repository gates stay Node-free
- **GIVEN** a machine without Node.js
- **WHEN** `just check` and `just test` run
- **THEN** both behave exactly as before this change

### Requirement: Section structure
The site SHALL organize its pages into these sections, in this order: Getting Started, Concepts, Guides, Tools, Reference, Troubleshooting. The order SHALL be fixed in the site configuration rather than derived from file names. A section without pages SHALL NOT appear. The site SHALL have a landing page, and Getting Started SHALL begin with the pages "What is PlatynUI?" and "Installation".

#### Scenario: Sections in their fixed order
- **GIVEN** the built site, with pages in Getting Started and Concepts
- **WHEN** its sidebar is read
- **THEN** Getting Started precedes Concepts, regardless of the file and directory names of their pages

#### Scenario: Empty sections are hidden
- **GIVEN** the Reference section has no page yet
- **WHEN** the site is built
- **THEN** the sidebar shows no Reference entry, and the build does not fail because of it

#### Scenario: First pages exist
- **WHEN** the site is built
- **THEN** it contains the landing page and, in Getting Started, "What is PlatynUI?" followed by "Installation"

### Requirement: Writing rules
The rules for writing the site SHALL be documented in one developer document, `dev-docs/user-docs.md`. They SHALL cover:

- the audience: Robot Framework users, not PlatynUI developers;
- English as the language;
- the four page types: tutorial, guide, concept, reference;
- how examples are written and included;
- platform- and toolkit-neutral wording;
- no status or roadmap text;
- one home per fact;
- the keyword docstrings as the contract the site links to rather than copies.

CONTRIBUTING.md and `docs/README.md` SHALL point to it.

#### Scenario: A contributor finds the rules
- **WHEN** a contributor opens CONTRIBUTING.md's documentation section or `docs/README.md`
- **THEN** each links to `dev-docs/user-docs.md`, and that document states the rules listed above

### Requirement: Robot Framework syntax highlighting
Code blocks marked as Robot Framework SHALL be highlighted with RobotCode's TextMate grammar, vendored into the docs project together with its license notice. The language names `robotframework` and `robot` SHALL both select it.

#### Scenario: A Robot Framework block is highlighted
- **GIVEN** a page with a fenced code block marked `robotframework` that contains a test case calling a keyword with arguments
- **WHEN** the site is built
- **THEN** the build reports no unknown language, and the rendered block marks the section header, the test name, the keyword and its arguments as distinct tokens

### Requirement: Internal links are checked
Every build SHALL check the links between the site's pages, including anchors. A link to a page or anchor that does not exist SHALL fail the build with a message naming the page and the link.

#### Scenario: A broken link fails the build
- **GIVEN** a page that links to a page that does not exist
- **WHEN** the docs build recipe runs
- **THEN** it exits with a non-zero code and names the page and the broken link

#### Scenario: A broken anchor fails the build
- **GIVEN** a page that links to an existing page with an anchor that page does not have
- **WHEN** the docs build recipe runs
- **THEN** it exits with a non-zero code and names the anchor

### Requirement: Site address derived at build time
The site's public URL and base path SHALL be supplied to the build from outside the site configuration, so that the configuration contains no repository owner, repository name or host. Without them, a local build SHALL serve the site at the root path. Internal links SHALL work under any base path.

#### Scenario: Built under a repository base path
- **GIVEN** the build is given the URL of a project site and the base path `/robotframework-platynui/`
- **WHEN** the site is built and its internal links are followed
- **THEN** every link resolves under `/robotframework-platynui/`, and no page links to the root path

#### Scenario: The repository moves
- **GIVEN** the repository is moved to another owner or renamed
- **WHEN** the deployment workflow runs on the moved repository
- **THEN** the site is published under the new address without any change to the site configuration

### Requirement: Examples are tested files
Every Robot Framework example on the site that is longer than a single keyword call SHALL live as a `.robot` file under `docs/examples/`. Pages SHALL include an example from its file, never as a copy. Each example file SHALL be a runnable suite tagged for the real-provider acceptance lanes. An example that drives an application SHALL drive the demo application from `add-demo-app` and start it the way the documentation teaches. A failing example SHALL fail the lane it runs in.

#### Scenario: A page shows the file's content
- **GIVEN** a page that includes `docs/examples/first-test.robot`
- **WHEN** the example file is changed and the site is rebuilt
- **THEN** the page shows the changed content, and no copy of the example exists in the page source

#### Scenario: Examples run in the acceptance lanes
- **GIVEN** the non-mock native build and the demo application are available
- **WHEN** the docs examples run on the Windows lane and on a Linux lane
- **THEN** every example suite passes (real provider only)

#### Scenario: A broken example fails the lane
- **GIVEN** an example whose locator no longer matches the demo application
- **WHEN** the docs examples run
- **THEN** the run fails and names the example suite and test

### Requirement: Built on pull requests, deployed from main
CI SHALL build the site, including the link check, for every pull request and every push. A failing build SHALL fail CI. Every push to `main` SHALL deploy the built site to the repository's GitHub Pages. Only the deployment step SHALL hold the permissions to publish.

#### Scenario: A pull request with a broken link
- **GIVEN** a pull request that adds a broken internal link
- **WHEN** CI runs
- **THEN** the docs build fails and the pull request shows a failed check

#### Scenario: A push to main publishes the site
- **GIVEN** a push to `main` whose docs build succeeds
- **WHEN** CI finishes
- **THEN** the repository's GitHub Pages serves the new build at the address derived for the repository

#### Scenario: Other branches do not deploy
- **WHEN** CI runs for a pull request or a branch other than `main`
- **THEN** the site is built but not deployed

### Requirement: Keyword reference is generated, not written
The Reference section SHALL hold the keyword reference of the PlatynUI libraries as pages generated from the libraries' docstrings, which are the contract for every keyword. The site SHALL NOT contain hand-written copies of keyword documentation. Guides and concept pages that need a keyword's exact behaviour SHALL name the keyword and, once the reference is generated, link to its entry. Generating the pages is out of scope here: it will use RobotCode's Markdown generator for Robot Framework library documentation, not Robot Framework's `libdoc` tool.

#### Scenario: No hand-written keyword reference
- **WHEN** the site's pages are reviewed against the writing rules
- **THEN** no page restates a keyword's arguments or its tables of options; a page that depends on them names the keyword instead
