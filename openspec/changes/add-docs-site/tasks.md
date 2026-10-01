# Tasks

## 1. Probe the link form

- [ ] 1.1 In a scratch directory outside the repository, scaffold a minimal Astro + Starlight site with `starlight-links-validator`. Add two pages that link to each other, once in relative form and once in the validator's base-aware absolute form, plus one link to an anchor. Build it with base `/` and with base `/robotframework-platynui/`. Settle the form that works in both builds and that the validator checks. Record it in design.md Decision 4. Verify that the chosen form passes both builds and that a deliberately broken link and a broken anchor fail both.

## 2. Site project

- [ ] 2.1 Create the Astro project in `docs/`:
  - `package.json` with the `dev`, `build` and `preview` scripts, and `package-lock.json`;
  - `.nvmrc` with the current Node.js LTS line;
  - `astro.config.mjs` with the Starlight integration and the title "PlatynUI";
  - `src/content.config.ts` with `docsLoader` and `docsSchema`;
  - `.gitignore` gains `node_modules/`, `dist/` and `.astro/`.

  `docs/talks/` stays untouched. Verify that `npm ci && npm run build` in `docs/` exits 0 and `git status` lists no build output.
- [ ] 2.2 Configure the six sections in `astro.config.mjs` (Getting Started, Concepts, Guides, Tools, Reference, Troubleshooting) as directory-generated groups in this fixed order. Skip a group whose directory holds no page. Verify on the built HTML that a group with pages appears in its configured position whatever its directory name, and that the empty `reference/` directory produces no sidebar entry and no build error.
- [ ] 2.3 Vendor RobotCode's `robotframework.tmLanguage.json` as `docs/grammars/`, with RobotCode's license notice and the source commit in a short `docs/grammars/README.md`. Register it through `expressiveCode.shiki.langs` with the aliases `robotframework` and `robot`. Verify that a page with a `robotframework` block builds without an unknown-language warning, and that the rendered block marks the section header, the test name, the keyword and its arguments as distinct tokens.
- [ ] 2.4 Enable `starlight-links-validator` for every build. Verify that a temporary page with a link to a missing page, and one with a missing anchor, make `npm run build` fail and name the page and the link. Remove the temporary pages afterwards.
- [ ] 2.5 Read `site` and `base` from `PLATYNUI_DOCS_SITE` and `PLATYNUI_DOCS_BASE`, with no site and base `/` when they are absent. Verify that a build with `PLATYNUI_DOCS_SITE=https://example.github.io` and `PLATYNUI_DOCS_BASE=/robotframework-platynui/` yields internal links that all start with `/robotframework-platynui/`, checked by grepping `dist/` for root links without the base. Verify also that `grep -ri 'd-biehl\|imbus' docs/astro.config.mjs` finds nothing.

## 3. Recipes and CI

- [ ] 3.1 Add the recipes to the Documentation section of the `justfile`:
  - `docs-dev` (install the locked dependencies and serve with live reload);
  - `docs-build` (install the locked dependencies and build).

  Verify that both work from a clean `docs/` and that `just check` and `just test` reference neither the recipes nor Node.js.
- [ ] 3.2 Add `.github/workflows/docs.yml` with two jobs.
  - **`build`:** runs on pull requests and pushes. It sets up Node from `docs/.nvmrc` with the npm cache, runs `actions/configure-pages`, then `npm ci` and `npm run build` in `docs/` with the derived origin and base path in `PLATYNUI_DOCS_SITE` and `PLATYNUI_DOCS_BASE`, and uploads `docs/dist` with `actions/upload-pages-artifact`.
  - **`deploy`:** runs only for pushes to `main`. It needs `build`, uses the `github-pages` environment with `pages: write` and `id-token: write`, and runs `actions/deploy-pages`.

  Workflow-level permissions are `contents: read`. Verify the workflow with `actionlint` if it is available, otherwise by review against design Decision 7. The deploy run itself is verified in 7.3.

## 4. Writing rules and pointers

- [ ] 4.1 Write `dev-docs/user-docs.md` per design Decision 8:
  - audience and voice;
  - the four page types and their outline;
  - how examples are written and included;
  - platform- and toolkit-neutral wording, including roles that depend on the toolkit;
  - no status or roadmap text beyond the landing page's notice;
  - one home per fact;
  - docstrings as the contract;
  - no mention of the mock provider;
  - the link form from 1.1;
  - screenshots from the demo application.

  Verify by reading it against the spec requirement "Writing rules".
- [ ] 4.2 Update the pointers:
  - `docs/README.md`: what the folder holds, how to build and serve the site, and a link to the writing rules.
  - CONTRIBUTING.md §11: the site, the rules, the recipes, and the one-time "GitHub Pages source = GitHub Actions" setting.
  - `dev-docs/README.md`: an index entry for `user-docs.md`.
  - Root `README.md`: a link to the site. It is an absolute URL under the repository's original home, like the README's other absolute links (CONTRIBUTING.md §11).

  Verify by reading each diff.

## 5. First content

- [ ] 5.1 Write the landing page (`src/content/docs/index.mdx`): what PlatynUI is for, the single preview notice, and a link to Getting Started. Verify that it builds and passes the link check.
- [ ] 5.2 Write `getting-started/what-is-platynui.md`: the desktop as a tree of elements, finding elements with XPath, and acting like a user, all in Robot Framework terms with no internals. Set `sidebar.order` so it comes first. Verify that it builds, that the sidebar lists it first, and that it reads against the writing rules.
- [ ] 5.3 Write `getting-started/installation.mdx`: the packages and commands as they are at the time of writing (checked against `pyproject.toml`, the package READMEs and the root README), and per-platform prerequisites in Windows and Linux tabs. Verify that it builds, that it follows "What is PlatynUI?" in the sidebar, and that every command on it was run once.

## 6. Tested examples

- [ ] 6.1 Write the first example suite, `docs/examples/getting-started/launch-and-close.robot`:
  - it starts the demo with `Start Process    platynui-demo`;
  - it finds it with the launch recipe from the `add-demo-app` spec and pins it with `Set Root`;
  - it closes it through its main window;
  - it is tagged `acceptance` and `real` and complete on its own.

  Verify with `robotcode analyze` and a dry run that the suite resolves every keyword.
- [ ] 6.2 Include the example on the installation page under "Check your setup", through `?raw` and `<Code>`. Verify that changing a line of the `.robot` file and rebuilding shows the change on the page, and that the page source holds no copy of the example.
- [ ] 6.3 Add `just test-docs-examples`. It runs `docs/examples` under the current OS's lane profiles (`real-x11` and `real-wayland` on Linux, `real-windows` on Windows), so the profile's session wrapper applies. Verify with a dry run that it selects the example suites and nothing under `tests/`.
- [ ] 6.4 Once `add-demo-app` provides `PLATYNUI_DEMO_COMMAND` to the lane sessions, add a step to the `acceptance-linux` job that runs the docs examples. Run them on the real Windows desktop as well. Verify that they pass on both, and that a deliberately broken locator fails the run and names the suite and the test. This stays open until the demo application exists.

## 7. Verification

- [ ] 7.1 Run `just docs-build` from a clean `docs/`, the broken-link probe (2.4) and the base-path probe (2.5) once more on the finished site. All behave as specified.
- [ ] 7.2 Run `just check`. It passes without Node.js on the path.
- [ ] 7.3 After the maintainer has merged to `main` and set the Pages source to GitHub Actions, the `deploy` job publishes the site and the pages and their links work at the derived address. This is checked by the maintainer after the push.
- [ ] 7.4 Run `openspec validate add-docs-site --strict`, and tick every task above.
