# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

## [Unreleased]

### Bug Fixes

- **native:** Simplify borrowing of PyDict in FromPyObject implementations for PointerOverridesInput and KeyboardOverridesInput ([9b47049](https://github.com/imbus/robotframework-PlatynUI/commit/9b470493fb754facd7a091170d2277223bb116cd))
- **platform-windows:** Enhance keyboard input handling with AltGr and modifier key aliases ([ea07d7f](https://github.com/imbus/robotframework-PlatynUI/commit/ea07d7fa63a66eeedf107f3b9727240a342dc7a5))
- **provider-windows-uia:** Update UiaNode's availability check to use CurrentProcessId ([500b4a2](https://github.com/imbus/robotframework-PlatynUI/commit/500b4a2a8141e01af19f3b755c537bdb4ea3def0))
- **runtime:** Update FromPyObject implementations for PointerOverridesInput and KeyboardOverridesInput to match new PyO3 ([54e4bfc](https://github.com/imbus/robotframework-PlatynUI/commit/54e4bfc3057d069922ba43a1f095b5611bdcc031))
- **scripts:** Update regex pattern for robotframework-platynui version matching ([d0b0e41](https://github.com/imbus/robotframework-PlatynUI/commit/d0b0e414c56524a14abbdeb3e0dd87526a266467))
- Correct update version script ([f7b88a6](https://github.com/imbus/robotframework-PlatynUI/commit/f7b88a640a6d76132f2435b240e7c49a28806718))
- Add sync command to pre-bump hooks for all packages and extras ([e310385](https://github.com/imbus/robotframework-PlatynUI/commit/e310385a823e38851d922590b212e1363cbe5034))
- Add type hints and mypy configuration for improved type checking ([b31f326](https://github.com/imbus/robotframework-PlatynUI/commit/b31f32631908a82d79a6b5ed53a25b0769940a61))


### Documentation

- Update Windows ApplicationNode id() fallback to include optional AUMID as a stable identifier ([0ace67e](https://github.com/imbus/robotframework-PlatynUI/commit/0ace67e136b29cded3a4a7ec5fc952b0ddfa2632))
- Update architecture documentation with symbol aliases for reserved characters and modifier key support ([addc5d6](https://github.com/imbus/robotframework-PlatynUI/commit/addc5d6fa21a3e76af97d096a16a30cf39a7f55a))
- Enhance architecture documentation with linking macros and FFI details, update plan ([ebdf73f](https://github.com/imbus/robotframework-PlatynUI/commit/ebdf73f0bf4dedfc2bcbe7d17d87a3ce8bba136c))
- Enhance CONTRIBUTING.md ([d7304ed](https://github.com/imbus/robotframework-PlatynUI/commit/d7304ed9a481a2fe45bf02bed9368676d34800c8))
- Update README.md for clarity and structure, enhancing installation instructions and project description ([5a447e3](https://github.com/imbus/robotframework-PlatynUI/commit/5a447e39af2847ee6f3ba1551dc83d9ebd6f2cd5))


### Features

- **cli:** Add snapshot command for exporting UI subtrees as XML ([60bf6b0](https://github.com/imbus/robotframework-PlatynUI/commit/60bf6b06dd2e0a7f6822fa803d452dc4aedb2051))
- **core:** Introduce is_valid method to check node availability ([6a82573](https://github.com/imbus/robotframework-PlatynUI/commit/6a82573dacd55a8b701ed05c0f824f4ea6cca3b1))
- **inspector:** Implement caching and refreshing of nodes ([035b6e2](https://github.com/imbus/robotframework-PlatynUI/commit/035b6e29f9d635b41691a387b421018028dc676d))
- **inspector:** Add some custom components for split layout and treeview ([5ecb57a](https://github.com/imbus/robotframework-PlatynUI/commit/5ecb57a7a0426ad78b8a3e0225d68258ecdc4f60))
- **keyboard:** Implement symbol aliases for reserved characters and enhance keyboard device functionality ([dee5900](https://github.com/imbus/robotframework-PlatynUI/commit/dee59001dc815d0ca227502d5e6ba3db1be0f5d2))
- **keyboard:** Add known_key_names method to KeyboardDevice trait and implementations ([bba29a1](https://github.com/imbus/robotframework-PlatynUI/commit/bba29a18f427ef6924feb148abde32762e9834ab))
- **platform-windows:** Implement keyboard device ([5e60335](https://github.com/imbus/robotframework-PlatynUI/commit/5e60335c4aed54f3eed3256c404ec1708f41863f))
- **platynui:** Add take_screenshot keyword to BareMetal Library ([74c8ea8](https://github.com/imbus/robotframework-PlatynUI/commit/74c8ea80187e477987d5921931a43e366b1eab72))
- **platynui:** Add a lot of keywords for BareMetal library ([0fc1fba](https://github.com/imbus/robotframework-PlatynUI/commit/0fc1fba9c475837f77399b2025fe7aa0342e5447))
- **provider-windows-uia:** Implement scoped RuntimeId URIs for UiaNode and related attributes ([c7c4623](https://github.com/imbus/robotframework-PlatynUI/commit/c7c462327b3084199a5c21f5645cda04c3716a5c))
- **python-native:** Enhance Point, Size, and Rect classes with 'from_like' methods and support for tuple/dict inputs ([77c7161](https://github.com/imbus/robotframework-PlatynUI/commit/77c71619de54c7e466b21a74a7a1bf16ffad8dd2))
- **python-native:** Add AttributeNotFoundError and update exception hierarchy ([c2f91d1](https://github.com/imbus/robotframework-PlatynUI/commit/c2f91d13c9d6a1394505fa7aa16cb628b71deb31))
- **python-native:** Add methods for ancestor traversal and pattern retrieval ([654bd61](https://github.com/imbus/robotframework-PlatynUI/commit/654bd61b44c0f0fcf6a120cff529430d6ebcce7f))
- **python-native:** Add is_valid method to UiNode for liveness checks ([a49e26c](https://github.com/imbus/robotframework-PlatynUI/commit/a49e26cf17c0ce07392f9a5a2c7166c77910bbcb))
- **ui:** Add optional developer-provided stable identifier `Id` to UiNode and related attributes ([8ee7905](https://github.com/imbus/robotframework-PlatynUI/commit/8ee7905f1dcf12c9117f2f2dd8922d753dbf15a5))
- **window:** Add bring-to-front functionality with optional wait time for window activation ([f6422ef](https://github.com/imbus/robotframework-PlatynUI/commit/f6422efc5e331bc6bf99152ef1f9ed0209e011c9))
- **xpath:** Introduce EvaluationStream for owned XPath evaluation results ([bca283f](https://github.com/imbus/robotframework-PlatynUI/commit/bca283fd075f5b35d02ccfed12d91fb1dfb413f3))
- Add project URLs to pyproject.toml files ([61cf164](https://github.com/imbus/robotframework-PlatynUI/commit/61cf16449c8c9a933a2a60ab5db91e554d0eab24))
- Introduce platynui-cli and platynui-inspector tools as separate installable python packages ([c58da55](https://github.com/imbus/robotframework-PlatynUI/commit/c58da55d9769e9694d59fcaf6c261b61a473f8a5))
- First version of spy tool part 3 ([345c3e2](https://github.com/imbus/robotframework-PlatynUI/commit/345c3e28f471eb9b213478b363b789ab53516b99))
- First version of spy tool part 2 ([f4b38e1](https://github.com/imbus/robotframework-PlatynUI/commit/f4b38e1cdc1f5287d181c29d5de304deb5540186))
- First simple version of spy tool ([ec5d124](https://github.com/imbus/robotframework-PlatynUI/commit/ec5d124bb3807195ff31183c36063cefd40a6057))


### Refactor

- **inspector:** Simplify conditional checks for cached_bounds assignment ([f0ddb50](https://github.com/imbus/robotframework-PlatynUI/commit/f0ddb50bae04e5a6dcf7f580f756796b0f4b398e))
- **platform-windows:** Improve buffer handling in process query functions and enhance click-through overlay behavior ([6bac989](https://github.com/imbus/robotframework-PlatynUI/commit/6bac989a6d62aae5f2d0ef18d5d4b3c487680b27))
- **pointer:** Update pointer click functions to accept optional target points ([bf725d6](https://github.com/imbus/robotframework-PlatynUI/commit/bf725d6430a57ba6b17f42aa5a10fcbe93523079))
- **provider-windows-uia:** Update COM initialization to use apartment-threaded model and remove warm-up code ([87eb9ef](https://github.com/imbus/robotframework-PlatynUI/commit/87eb9efa3d8ac0a88b3b6a54e67f770f120b49fc))
- **python-native:** Simplify type annotations and improve consistency across the module ([5feda99](https://github.com/imbus/robotframework-PlatynUI/commit/5feda9936d289c1d9cc84ad2ee4a96a3213bef98))
- **runtime:** Remove unused rect_alias_attributes function and related call in DesktopNode ([650a50e](https://github.com/imbus/robotframework-PlatynUI/commit/650a50e0bae9b1c77be78b39e5a25cff5ba0d5d7))
- Rename packages and update dependencies in project files ([346b346](https://github.com/imbus/robotframework-PlatynUI/commit/346b346bf74afa7831c83db143189419abfbde26))
- Replace Rc with Arc for runtime management in main function ([eb3ac97](https://github.com/imbus/robotframework-PlatynUI/commit/eb3ac97603aee8ebd4881334fa2d452f9b67b3bf))
- Unify highlight request handling and support multiple rectangles ([03bd173](https://github.com/imbus/robotframework-PlatynUI/commit/03bd173342286eb9926399409de635a641c661ca))


## [0.1.0] - 2025-10-09

### Bug Fixes

- **evaluator:** Enhance boolean evaluation for node sequences ([3d8b85a](https://github.com/imbus/robotframework-PlatynUI/commit/3d8b85a81a9df54c7ab0fd5f43acb83fda8d6d61))
- **platform-mock:** Remove auto-registration of mock highlight and keyboard devices ([e2ee2b9](https://github.com/imbus/robotframework-PlatynUI/commit/e2ee2b933b7b386d0890fa0e51c8b5e293ba405a))
- **provider-windows-uia:** Correct getting clickable point and bounding rectangle ([1d4a42a](https://github.com/imbus/robotframework-PlatynUI/commit/1d4a42add4d3966cfd545512a2f884fd9adc1f1b))
- **python-bindings:** Restrict pointer origin to desktop|Point|Rect and update docs/README ([a5f2554](https://github.com/imbus/robotframework-PlatynUI/commit/a5f2554737e0085bf363378f5f0bfb5fdc9ae21c))

  - Remove tuple/dict acceptance for origin in platynui_native.runtime

  - Typing stays strict (runtime.pyi already limited)

  - Align docs and README example to core.Point instead of tuple

- **simple-node:** Assign document order for standalone trees ([4db05fd](https://github.com/imbus/robotframework-PlatynUI/commit/4db05fd6e5d1688f944a9896648d098ff2f3d191))
- **tests:** Update error message to use English for empty keyboard sequence ([e7a4100](https://github.com/imbus/robotframework-PlatynUI/commit/e7a41002768b80b83b139263d409d591918a09ee))
- **tests:** Add unused braces allowance for root and sc fixtures ([7170358](https://github.com/imbus/robotframework-PlatynUI/commit/717035832152f1fcd29363690e523878d6007fe3))
- **windows/highlight:** Check DC/DIB creation and clean up on failure ([d5a3413](https://github.com/imbus/robotframework-PlatynUI/commit/d5a34134b7aa70f4c652a9f33857ee1e6e3b3be3))

  - Guard  and  results; early return.
  - Replace  expect with safe match; free DCs on error paths.
  - Import /; no change to drawing logic.
  - Build and clippy stay clean (no warnings).

- **xpath:** Correct handling of wildcard test matching only elements and attributes not documents ([9c29788](https://github.com/imbus/robotframework-PlatynUI/commit/9c297884f1292997bae96073a2a73bd7f99e71b0))
- **xpath:** Correct union, except and intersect operators ([b6c36ca](https://github.com/imbus/robotframework-PlatynUI/commit/b6c36ca55ca3cf22aa1ceee1f1ba57f1986873ee))
- **xpath:** Normalize dayTimeDuration equality keys ([4bc819b](https://github.com/imbus/robotframework-PlatynUI/commit/4bc819b2a85b7e557e1e99460d453d73e625ec05))
- **xpath:** Enforce context item checks for root path compilation ([94f892f](https://github.com/imbus/robotframework-PlatynUI/commit/94f892f217f43b86fdc77bc924d8292d0fdaa7f0))
- **xpath:** Enforce spec-accurate function conversions ([1be2915](https://github.com/imbus/robotframework-PlatynUI/commit/1be29158ea3d376ac7273170878bd64d389a37c3))
- **xpath:** Correct namespace wildcards and string value concatenation ([3307676](https://github.com/imbus/robotframework-PlatynUI/commit/33076769342e625eb9867d6acfca60e6f61e3bbc))
- **xpath:** Consolidate duplicated `namespace-uri` functions ([e813823](https://github.com/imbus/robotframework-PlatynUI/commit/e8138237887fa566d28081c03aecb0c2568a0bc5))
- **xpath:** Improve error handling for logical and comparison operators, and enhance lock handling in TestNode ([8230d9f](https://github.com/imbus/robotframework-PlatynUI/commit/8230d9f76fc6ca4dd891d4605ecc70d48d7b7a16))


### Documentation

- **api:** Implement evaluate API variant for flexible context handling ([93ebb28](https://github.com/imbus/robotframework-PlatynUI/commit/93ebb2885c3b3c06793340258323166897b67f0d))
- **copilot:** Correct terminology for packaging and update repository layout details ([886a7b2](https://github.com/imbus/robotframework-PlatynUI/commit/886a7b27eb45ad39bdcf8ac5ac76bc7e40708b0e))
- **core:** Align attribute names and architecture ([cb5579a](https://github.com/imbus/robotframework-PlatynUI/commit/cb5579a2583202687bc84407e1d04c2c46ec8632))
- **keyboard:** Refine runtime contract ([e6a3d9d](https://github.com/imbus/robotframework-PlatynUI/commit/e6a3d9dd3cf1f5af4d7c91a59734d4380cf05c43))
- **plan:** Update section headers and reorganize task lists in implementation plan ([88ec022](https://github.com/imbus/robotframework-PlatynUI/commit/88ec022c144f9420728b26d4af36a7980eed3167))
- **runtime:** Describe scroll_into_view action for control and item elements ([2bf4450](https://github.com/imbus/robotframework-PlatynUI/commit/2bf44504c886163ccc781985f95e9e8c92442b4f))
- **runtime:** Rename implementation plan for PlatynUI Runtime ([4dd42dd](https://github.com/imbus/robotframework-PlatynUI/commit/4dd42dd9c3fd4100af981c1603ae74e7cbad6612))
- **runtime:** Clarify control vs app namespace usage ([5746ce1](https://github.com/imbus/robotframework-PlatynUI/commit/5746ce10fd7b5594d52347539aac4bceffc082d5))
- **runtime:** Clarify WindowSurface guidance ([17ab132](https://github.com/imbus/robotframework-PlatynUI/commit/17ab1320fcc9dc4a1973a3c396a33aa2d4ed7488))
- **windows:** Note dpi-aware pointer task ([7b1976e](https://github.com/imbus/robotframework-PlatynUI/commit/7b1976e2e854fda73e4c753cb9bc27b3b1e6fa1d))
- **xpath:** Update XPath 2.0 coverage notes for compatibility mode and namespace tracking ([b80aef7](https://github.com/imbus/robotframework-PlatynUI/commit/b80aef77981d9df44c6050ecbd37b76d1ea189d8))
- **xpath:** Add a Readme and comprehensive coverage documentation for XPath 2.0 compliance ([3a0920e](https://github.com/imbus/robotframework-PlatynUI/commit/3a0920eb99387fa8ccaa41b8042ee35840ffd1f2))
- Update AGENTS.md ([72eeae6](https://github.com/imbus/robotframework-PlatynUI/commit/72eeae6ebb88734808dcf595e79bcd270395418e))
- Refine plan ([650e34d](https://github.com/imbus/robotframework-PlatynUI/commit/650e34d9e936cb456eaad9b8e06d7853eb30fdb3))
- Enhance Windows cross-compilation instructions for GNU and MSVC toolchains ([99480cd](https://github.com/imbus/robotframework-PlatynUI/commit/99480cd162556961f34f2a1d402c822e6286b8a9))
- Capture platform init and pointer updates ([434ff27](https://github.com/imbus/robotframework-PlatynUI/commit/434ff27e2fde9e0d9a4ca42dffb1f8355c8f1b4c))
- Expand Windows platform section with detailed tasks for Pointer, Keyboard, Highlight, Screenshot, and UIAutomation integration ([17813ec](https://github.com/imbus/robotframework-PlatynUI/commit/17813eccaf92c6bfaa10a2fdb38e3b018dcd54ee))
- Update roadmap and implementation focus for cross-platform support ([0a6853f](https://github.com/imbus/robotframework-PlatynUI/commit/0a6853f625896c5692144f04c62cb61361c900d5))
- Update documentation for PlatynUI Runtime architecture and legacy analysis ([e0bbe70](https://github.com/imbus/robotframework-PlatynUI/commit/e0bbe708e78baf34851f0e1b5e1e3d83a8b78686))
- Implement comprehensive architecture and design documentation for PlatynUI Runtime ([6d6ea46](https://github.com/imbus/robotframework-PlatynUI/commit/6d6ea462c4b5c5adb2cca97b236e02684c54f998))
- Add error handling guidelines for Rust in AGENTS.md ([b591326](https://github.com/imbus/robotframework-PlatynUI/commit/b591326fc75d1cdd02ab593dfff9c2384f43ea88))
- Update comments in model.rs to use English and improve clarity in AGENTS.md ([e3f502a](https://github.com/imbus/robotframework-PlatynUI/commit/e3f502a38524900c4592a737094f0066b6d10283))
- Update xpath 2 plan ([1855790](https://github.com/imbus/robotframework-PlatynUI/commit/18557901b1fd4bc3359d48d96e1d89f1f01363c0))


### Features

- **cli:** Enhance pointer commands to return element information on actions ([fde9ed3](https://github.com/imbus/robotframework-PlatynUI/commit/fde9ed394d2beef4d28d4768689dee8a54fafe2b))
- **cli:** Enhance pointer commands to support XPath expressions and optional point arguments ([2b8a8ae](https://github.com/imbus/robotframework-PlatynUI/commit/2b8a8aedf347bd1e5a0df2464e9ba4219efefc52))
- **cli:** Add focus command leveraging runtime patterns ([504a529](https://github.com/imbus/robotframework-PlatynUI/commit/504a529baf59f475c68bc8e91a07dba044599eb4))
- **cli:** Colorize query output ([704d3c4](https://github.com/imbus/robotframework-PlatynUI/commit/704d3c433e0a5ed865a18187f0e4afb86bb4c9c4))
- **cli:** Add watch command for provider events ([273b42d](https://github.com/imbus/robotframework-PlatynUI/commit/273b42df4708f72ea721b3cae44bcd55b7931084))
- **cli:** Add mock-backed provider listing ([bbaef5f](https://github.com/imbus/robotframework-PlatynUI/commit/bbaef5f1093588ba77fdcb7731622033d5c46ebc))
- **core:** Streamline keyboard device api ([931197b](https://github.com/imbus/robotframework-PlatynUI/commit/931197b8638aee9bc71e6b1885a4586146331c67))

  Complete keyboard device contract cleanup (Plan step 15): provider now exposes key_to_code + send_key_event only; documentation and checklists updated.

- **core:** Standardize runtime id scheme ([572df01](https://github.com/imbus/robotframework-PlatynUI/commit/572df018dc818a2c13942c72786daea6a57d86ed))
- **core:** Add lazy pattern resolution ([0ed0f8b](https://github.com/imbus/robotframework-PlatynUI/commit/0ed0f8bdf8ccdbba37d6a1811aa29312ae070f22))
- **core:** Add runtime pattern helpers ([4d36c43](https://github.com/imbus/robotframework-PlatynUI/commit/4d36c43dbb9375ce3347e451737c03fbd016c2bd))
- **core:** Support structured ui values and flatten geometry ([d22c8aa](https://github.com/imbus/robotframework-PlatynUI/commit/d22c8aac4ca06c32f16a12464ca43208a1239c67))
- **docs:** Update provider checklist and implementation plan for UiNode and UiAttribute traits ([f0c4189](https://github.com/imbus/robotframework-PlatynUI/commit/f0c4189ca32ff49200efe6cc66442cef998e6f49))
- **docs:** Enhance architecture and implementation details for UiNode and XPath integration ([13dcc62](https://github.com/imbus/robotframework-PlatynUI/commit/13dcc627bc3dbc5f8d0c47d5d364ed04fa41f1f9))
- **evaluator:** Expose new streaming evaluation function in public API and add convenience function for streaming evaluation ([376e6b3](https://github.com/imbus/robotframework-PlatynUI/commit/376e6b398afda4973f8c99acf6a5d6c98313982b))
- **keyboard:** Finalize mock stack and CLI ([92d69f0](https://github.com/imbus/robotframework-PlatynUI/commit/92d69f0a678e139a784a30a78b593bbf854f904e))

  - unify keyboard commands around sequence arguments

  - log key events in mock provider

  - update docs and tests

- **link:** Centralize provider linking; mock-only tests; docs update ([c5d3f3f](https://github.com/imbus/robotframework-PlatynUI/commit/c5d3f3f088ea9e6ca13f1da88a0b566ba1553cbc))
- **mock:** Render platynui testcard screenshot ([c980306](https://github.com/imbus/robotframework-PlatynUI/commit/c980306a2c433188f9339581ea84d7b4b1b51dbd))
- **mock:** Load mock tree from xml and improve query output ([87b83df](https://github.com/imbus/robotframework-PlatynUI/commit/87b83dfaffa2c793faf60dcb5990a5f4b33e4176))
- **optimizer:** Add constant folding optimization and related benchmarks ([a2e5f43](https://github.com/imbus/robotframework-PlatynUI/commit/a2e5f43ada82ae98acaa51b83273e341760b0bc2))
- **pointer:** Add multi-click functionality and related error handling ([e7b08bf](https://github.com/imbus/robotframework-PlatynUI/commit/e7b08bf5b10490691672dc1f61557a8f3e2610ad))
- **pointer:** Activate profile timing controls ([26e67c1](https://github.com/imbus/robotframework-PlatynUI/commit/26e67c1f8f7175bddeb82a98f31f8812c436b755))
- **pointer:** Add configurable move timing ([20ca97b](https://github.com/imbus/robotframework-PlatynUI/commit/20ca97ba7ac0a1f48536c2d259574ba230a893a1))
- **pointer:** Add move duration and time per pixel options for pointer movement ([1dc3003](https://github.com/imbus/robotframework-PlatynUI/commit/1dc3003127aafe26897b36e275e7d29a0c4a6c9f))
- **pointer:** Expose pointer API and CLI command ([a9b8be9](https://github.com/imbus/robotframework-PlatynUI/commit/a9b8be901d3c039f6f59380ad423d701c8aa659d))
- **provide-windows-uia:** Enhance virtualized item handling and improve parent-child relationships ([d56105d](https://github.com/imbus/robotframework-PlatynUI/commit/d56105de2f92cba71a031aeeaadf88b7ddbc6406))
- **provider-uia:** Add support for window state attributes (IsMinimized, IsMaximized, IsTopmost) and user input acceptance ([b9a45ef](https://github.com/imbus/robotframework-PlatynUI/commit/b9a45ef43296f25792e6dc65fe04ec0528f01d3d))
- **provider-window-uia:** Implement WaitForInputIdleChecker ([6927795](https://github.com/imbus/robotframework-PlatynUI/commit/6927795b54aaee0aec2b784bd07b0bf7e3966790))
- **provider-windows-uia:** Implement Application view ([c51a3f4](https://github.com/imbus/robotframework-PlatynUI/commit/c51a3f459b12e9082ed8f11df3fc171ea71a93ed))
- **provider-windows-uia:** Implement native property support ([b262755](https://github.com/imbus/robotframework-PlatynUI/commit/b26275595166ffcc1a78e62245ba2dafea48a99d))
- **python:** Stabilize python modules ([92aaf94](https://github.com/imbus/robotframework-PlatynUI/commit/92aaf9499a1ff9dfbb307a82d9c2db4b8e234262))
- **python:** Add first Python bindings for core and runtime ([2111984](https://github.com/imbus/robotframework-PlatynUI/commit/2111984a1741ae1306bc4f7ad0275c0e53617994))
- **python-bindings:** Add highlight/clear_highlight and screenshot to runtime ([7d64e50](https://github.com/imbus/robotframework-PlatynUI/commit/7d64e50331bcb983c262627830c9d0f248815e4e))
- **python-bindings:** Expose desktop_node/desktop_info/focus in runtime module ([0441dde](https://github.com/imbus/robotframework-PlatynUI/commit/0441dde0c46609f060b621d9491f56ff7ff23506))

  - Add Runtime.desktop_node(), desktop_info() dict conversion, focus(node)
  - Update typing stubs and concept doc
  - Add Python tests for desktop info and basic focus

- **python-bindings:** Make UiAttribute.value() lazy method instead of property ([c068fbf](https://github.com/imbus/robotframework-PlatynUI/commit/c068fbfc6c7885d719aa0d0d9aa1ce59c4a92f0e))

  - UiAttribute now holds owner node and resolves value on demand
  - runtime.pyi updated: value() returns UiValue | None, no property
  - Concept doc updated to reflect lazy access

- **python-runtime-native:** Simplify python package/module structure and rewrote pyi files ([5efc4ff](https://github.com/imbus/robotframework-PlatynUI/commit/5efc4ffb3170a7dba699fe6fbfb834825b2d7f94))
- **repl:** Implement XPath REPL example with error handling and sample document ([b1caf5d](https://github.com/imbus/robotframework-PlatynUI/commit/b1caf5d029dd1b90e9a560947dce1a95984383c8))
- **runtime:** Add evaluate_single method for XPath evaluation ([2906a5d](https://github.com/imbus/robotframework-PlatynUI/commit/2906a5d077f8cdd46bda45978003c3eb5792e723))
- **runtime:** Implement shutdown lifecycle management and idempotency ([4bbf7b5](https://github.com/imbus/robotframework-PlatynUI/commit/4bbf7b5506c6003f32e7c26c8956cfbe4df63974))
- **runtime:** Finish mock fallback build gating ([b4e074f](https://github.com/imbus/robotframework-PlatynUI/commit/b4e074fc9771ccb5a912609e9f70a3f698bb5e65))
- **runtime:** Add keyboard sequence parser and API ([f63cfb8](https://github.com/imbus/robotframework-PlatynUI/commit/f63cfb827ed16e99c1dad812b53f9720f91bfb2b))
- **runtime:** Add window command and WindowSurface mocks ([c65e59f](https://github.com/imbus/robotframework-PlatynUI/commit/c65e59fb1ca9fea82568d5d865ac47d5554049bc))
- **runtime:** Add event capabilities to ProviderDescriptor and update runtime handling ([75cc15b](https://github.com/imbus/robotframework-PlatynUI/commit/75cc15b5b9aeb593808c6fd031764014be462391))
- **runtime:** Surface desktop info provider pipeline ([5bc2e1e](https://github.com/imbus/robotframework-PlatynUI/commit/5bc2e1eb484761ac9da02a2f68c13921220461a9))
- **runtime:** Add xpath evaluation bridge ([64d4abc](https://github.com/imbus/robotframework-PlatynUI/commit/64d4abcc043b38eca8099a2f0215f177e69c619c))
- **windows:** Implement Desktop Provider ([6270687](https://github.com/imbus/robotframework-PlatynUI/commit/6270687f2b97fc1331c310a377dd7e8f8d6edf69))
- **windows:** Implement screenshot provider ([0d4e43f](https://github.com/imbus/robotframework-PlatynUI/commit/0d4e43fceb21242b9411a533970bb0105e42577e))
- **windows:** Implement highlight provider ([ad85912](https://github.com/imbus/robotframework-PlatynUI/commit/ad85912d43d1ae2eedb2b860ee592e46063568b8))
- **windows:** Centralize dpi setup and register pointer device ([388e641](https://github.com/imbus/robotframework-PlatynUI/commit/388e64112079dd900bd09549e10f57a2cc243279))
- **windows-uia:** Implemented first version of UIAutomation Provider ([2d575d7](https://github.com/imbus/robotframework-PlatynUI/commit/2d575d79d6d5db4d258219d4fadb88d66a55c4e3))
- **xpath:** Implement real streaming of results ([dcc8eca](https://github.com/imbus/robotframework-PlatynUI/commit/dcc8eca4d311983d718f9b26e6b4566aa4ea78ec))
- **xpath:** Stream typed atomics for runtime evaluation ([da8b1cf](https://github.com/imbus/robotframework-PlatynUI/commit/da8b1cfefb2aad1e5af51dfe43a83886cfc0166c))
- **xpath:** Enforce static context item type checks ([cd79395](https://github.com/imbus/robotframework-PlatynUI/commit/cd79395f2776e7b973cb3c6eaae35a9b3787151b))
- **xpath:** Enforce XPath function conversion rules ([2e67ee6](https://github.com/imbus/robotframework-PlatynUI/commit/2e67ee6aaf3914565ef34b2375a4dfe2cf18d5a1))
- **xpath:** Implement parameter kind tracking and atomization for function calls (first version) ([6412209](https://github.com/imbus/robotframework-PlatynUI/commit/64122090925a44e954e54780c138c8d00b6c440e))
- **xpath:** Implement compile cache in StaticContext for performance optimization ([9c62994](https://github.com/imbus/robotframework-PlatynUI/commit/9c6299402db034dd25a068abc1c2aa56a6a433ae))
- **xpath:** Finalize extended casting support and some speed optimizations ([87a1f02](https://github.com/imbus/robotframework-PlatynUI/commit/87a1f027184a15d3dbeb433913630b5dd7c7df48))
- **xpath:** Add SingleItemCursor and RangeCursor for XdmSequenceStream, enhance chaining functionality ([330bd2c](https://github.com/imbus/robotframework-PlatynUI/commit/330bd2c979c9a669021345cfeff70afa939f2f46))
- **xpath:** Implement function signatures catalogue and statically known collations ([7d7fcc2](https://github.com/imbus/robotframework-PlatynUI/commit/7d7fcc2249adeac19fe791cf45f64b2edb44a5bb))
- **xpath:** Add support for filter expressions in path steps and implement corresponding evaluation logic ([1314585](https://github.com/imbus/robotframework-PlatynUI/commit/13145853a83dc0661449a6bf19d29af8fcb0d7a2))
- **xpath:** Implement 'let' expression ([ea60d59](https://github.com/imbus/robotframework-PlatynUI/commit/ea60d5979ef11117c9a4e33909abde2e5945dd31))
- **xpath:** Implement variable scoping and error handling for undeclared variables ([59cc134](https://github.com/imbus/robotframework-PlatynUI/commit/59cc13429333e54865693c12657409fc442393e0))
- **xpath:** Align fn implementations with W3C F&O 2.0 ([9b551ba](https://github.com/imbus/robotframework-PlatynUI/commit/9b551baccc801d9d2a23236cd85203f997910d6a))
- **xpath:** Add some remaining xpath functions and optimizations ([88b584b](https://github.com/imbus/robotframework-PlatynUI/commit/88b584b630cc87fd5416db277bec386b42c64412))
- **xpath:** First most complete XPath evaluator ([50afd00](https://github.com/imbus/robotframework-PlatynUI/commit/50afd0020b843aa368bf56a5353be90b71766f72))
- **xpath:** Rewrite of parts of the evaluator ([b900e4a](https://github.com/imbus/robotframework-PlatynUI/commit/b900e4a8937aaaaa9ee244077bf4096fd2bfa32f))
- **xpath:** Complete rewrite of compiler ([ecbfca7](https://github.com/imbus/robotframework-PlatynUI/commit/ecbfca7283381826d218cfaf07f3c3c7222768f8))
- **xpath:** Complete rewrite of xpath parser ([a1dc405](https://github.com/imbus/robotframework-PlatynUI/commit/a1dc405f8fc2577d883205564fdbe60590b28fd9))
- **xpath:** Implement XPath 2.0 Temporal Functions and Types ([04dba8f](https://github.com/imbus/robotframework-PlatynUI/commit/04dba8f92549ca4760e9f39f9ace545e64ec8f83))

  - Added support for dateTime, date, time, yearMonthDuration, and dayTimeDuration types in the XDM model.
  - Introduced functions for extracting components from dateTime and date types (year, month, day) and time types (hours, minutes, seconds).
  - Implemented implicit-timezone function to retrieve the effective timezone.
  - Enhanced string representation for date, time, and dateTime types, including timezone formatting.
  - Added comprehensive tests for datetime arithmetic, comparisons, casting, and edge cases.
  - Included error handling for invalid date and time formats.

- **xpath:** Implement collation functions and regex support in XPath evaluator ([16bb621](https://github.com/imbus/robotframework-PlatynUI/commit/16bb621da8f18e2072501810645a49589afcb216))

  - Added new collation types: SimpleCaseCollation, SimpleAccentCollation, and SimpleCaseAccentCollation with appropriate comparison and key methods.
  - Registered built-in simple collations in the CollationRegistry.
  - Enhanced the RegexProvider with methods for matching, replacing, and tokenizing strings using Rust's regex library.
  - Introduced DynamicContext fields for current date/time handling and timezone overrides.
  - Added tests for collation-aware functions including contains, starts-with, ends-with, compare, and codepoint-equal.
  - Implemented tests for current-dateTime, current-date, and current-time functions with fixed 'now' and timezone overrides.
  - Created tests for regex functions including matches, replace, and tokenize, covering various flags and error cases.

- **xpath:** Make try_compare_by_ancestry public ([a128af0](https://github.com/imbus/robotframework-PlatynUI/commit/a128af02a1751ba694c6b9e53786bbf284c4cd25))
- **xpath:** Refactor XPath Function Implementation and Enhance Document Order Comparison ([2699ada](https://github.com/imbus/robotframework-PlatynUI/commit/2699ada623b2ef382b96200f8832b5b87a8599df))

  - Introduced CallCtx struct to provide context for function implementations, including dynamic and static contexts, default collation, resource resolver, and regex provider.
  - Updated FunctionImpl type to accept CallCtx as a parameter.
  - Modified compare_document_order method in XdmNode trait to return Result<Ordering, Error> instead of Ordering, allowing for error handling in node comparisons.
  - Adjusted multiple test implementations to align with the new compare_document_order signature.
  - Added new tests for multi-root error handling and function context exposure, ensuring proper functionality of the new CallCtx structure.
  - Updated documentation to reflect changes in the API and implementation status, including the transition to context-aware function signatures.

- Add update version and changelog scripts ([6065757](https://github.com/imbus/robotframework-PlatynUI/commit/606575777482385c0a555c1c866e5fd93bbaa173))
- Add CI workflow for building, testing, and packaging Python and Rust projects ([e5c1f39](https://github.com/imbus/robotframework-PlatynUI/commit/e5c1f396acc5e013f4d6c354462b8e81b435de8f))
- Add duration-aware highlighting pipeline ([57e4aac](https://github.com/imbus/robotframework-PlatynUI/commit/57e4aac6b8d91e17b2212e40f0ef92f023de4e56))
- More steps for out own XPath2Parser ([103e779](https://github.com/imbus/robotframework-PlatynUI/commit/103e779e52f958f275596199b96352d596cd8c3b))
- First steps for our own xpath evaluator ([411105b](https://github.com/imbus/robotframework-PlatynUI/commit/411105b1530b6fefd7a18fe3ecba77a157a4e94e))
- Enhance string literal parsing to normalize doubled quotes in XPath expressions ([f105df9](https://github.com/imbus/robotframework-PlatynUI/commit/f105df9768f527b69500063ca42952aee5f03bc0))
- Enhance XPath2 grammar with structured keywords and operator rules ([fd320b5](https://github.com/imbus/robotframework-PlatynUI/commit/fd320b56976f896e776368f596fe3264adfa9cfe))
- Initialize project ([c83eab4](https://github.com/imbus/robotframework-PlatynUI/commit/c83eab4342aaa40871bfa5fc73c977df1347fbfa))


### Performance

- **cli:** Optimized cli query output ([2fb1173](https://github.com/imbus/robotframework-PlatynUI/commit/2fb11734c39587e027a1c93fc4fdaa51dbbb8726))
- **doc-order:** Use doc_order_key for distinct and sets ([9955fcb](https://github.com/imbus/robotframework-PlatynUI/commit/9955fcb6cfe905e8a96846dc9ada411d5f61d4e7))
- **evaluator:** Stream axis traversal buffer ([3da02e5](https://github.com/imbus/robotframework-PlatynUI/commit/3da02e565b5d4219762daf33caeaa65547124479))
- **evaluator:** Reuse vm frame overlays ([a322d70](https://github.com/imbus/robotframework-PlatynUI/commit/a322d70778b1df2dc64f9c8480268f948a4520f7))
- **evaluator:** Switch VM stacks to SmallVec and update plan ([4418475](https://github.com/imbus/robotframework-PlatynUI/commit/4418475c1199f4b967f05d38811e44b294d30879))
- **parser:** Reduce Pair cloning in AST builder ([f23f814](https://github.com/imbus/robotframework-PlatynUI/commit/f23f814169be629d2e270e0b8bb03497904daf7a))
- **runtime:** Implement RuntimeXdmNode caching ([2a33bd1](https://github.com/imbus/robotframework-PlatynUI/commit/2a33bd123bc6595fa0a8f29b72daae28a9f7d83f))
- **set-ops:** Hash-based union/intersect/except ([4c8d0b6](https://github.com/imbus/robotframework-PlatynUI/commit/4c8d0b6ac25200b109aa8b92f87f63ab58cd3b61))
- **set-ops:** Use doc_order_key and smallvec buffers ([87d42a7](https://github.com/imbus/robotframework-PlatynUI/commit/87d42a75a0446f7b38ec93b5cf52031739789ecd))
- **set-ops:** Smallvec buffering for axes and doc-order keys ([e5cce15](https://github.com/imbus/robotframework-PlatynUI/commit/e5cce15c4a0cf212179d8c8179babf93a5fdf72d))
- **xpath:** Remove legacy function registration and streamline to use stream-based functions ([0397883](https://github.com/imbus/robotframework-PlatynUI/commit/0397883df8812a26ba9ea00d74fa2c8bc288bf19))
- **xpath:** Convert the rest of the xpath functions to streaming versions ([44ebe2a](https://github.com/imbus/robotframework-PlatynUI/commit/44ebe2aaa2a40f2676caff04edb4e731ce8f4281))
- **xpath:** Implement more streamed xpath functions ([b0ed405](https://github.com/imbus/robotframework-PlatynUI/commit/b0ed405083fcbc3d56def0f89a225176fcab68b9))
- **xpath:** Implement Predicate-Pushdown-Optimizer and Short-circuit Evaluation ([bb641bf](https://github.com/imbus/robotframework-PlatynUI/commit/bb641bf9bd9ace4610d0d69dcc22e826f8669c08))
- **xpath:** Stream axis/path; remove unwraps ([335ecef](https://github.com/imbus/robotframework-PlatynUI/commit/335ecef035ba10887e25b122fdbf4295aa384dcf))
- **xpath:** Stream evaluator end-to-end; add optimistic doc-order hint ([564493e](https://github.com/imbus/robotframework-PlatynUI/commit/564493eabe59351d4383e44ac61fb99648c9d2b3))
- **xpath:** Streamline doc order and predicate lowering ([754de09](https://github.com/imbus/robotframework-PlatynUI/commit/754de090d1855700c279e75de1e64954cf342fa3))
- **xpath:** Shortcut doc-order distinct when already sorted ([cbd2e36](https://github.com/imbus/robotframework-PlatynUI/commit/cbd2e360b735a2f04587c5ef962aa973ef014c50))
- **xpath:** Reuse doc-order keys in distinct ([746b253](https://github.com/imbus/robotframework-PlatynUI/commit/746b2538a30998f36584f5178422094b0ee841d0))
- **xpath:** Cache fancy regex compilations ([8d4771a](https://github.com/imbus/robotframework-PlatynUI/commit/8d4771adf2e44ef84fa1b2a2debe412b97ad9e0b))
- **xpath:** Optimize set operators with doc-order merges ([bb2fd7e](https://github.com/imbus/robotframework-PlatynUI/commit/bb2fd7ef857076eb81e108eb107a4ca6f3502f8c))
- **xpath:** Precompute document order metadata ([eb7065c](https://github.com/imbus/robotframework-PlatynUI/commit/eb7065cb62662f1011b8da1619ec408472ad46e4))
- Improve compiler scope stack and set-union merge ([8dc3a48](https://github.com/imbus/robotframework-PlatynUI/commit/8dc3a48a9f400f7fcdb54fd83a05cdb74ea54e51))


### Refactor

- **cli:** Simplify attribute handling in pointer commands and improve iterator type definitions in XPath ([58f7814](https://github.com/imbus/robotframework-PlatynUI/commit/58f781416754fbfe2c294d9389ab57d06f844660))
- **cli:** Remove unused filters ([1abd0e8](https://github.com/imbus/robotframework-PlatynUI/commit/1abd0e8fb6dd6bca4f73c87058dbccafb1f22e9c))
- **cli:** Split subcommands into modules ([0927dc9](https://github.com/imbus/robotframework-PlatynUI/commit/0927dc942585fcc3883c209174ebc65fee9c27e4))
- **cli,platform-mock:** Unify CLI output; move logs to mocks ([9fb5817](https://github.com/imbus/robotframework-PlatynUI/commit/9fb5817971200b0ad6e795d898632c11342b8901))
- **core:** Remove strategies module and associated node traits ([530cdc4](https://github.com/imbus/robotframework-PlatynUI/commit/530cdc4ee56ff826404e8bab94eb3d2f1cee900a))
- **docs:** Improve clarity in platform and provider descriptions ([08ad47d](https://github.com/imbus/robotframework-PlatynUI/commit/08ad47d288d57e07fdaec9f4957c47187b93e3bb))
- **evaluator:** Optimize set operations for streaming performance ([9676e14](https://github.com/imbus/robotframework-PlatynUI/commit/9676e144dbbe975f111a6506328cc8a898fb6b1f))
- **evaluator:** Replace attribute buffering with true streaming for improved performance ([29ad067](https://github.com/imbus/robotframework-PlatynUI/commit/29ad0675282a4d591bc16dd0930160eb9d3e6ed8))
- **focus:** Simplify focus command to handle single node evaluation ([2cacebe](https://github.com/imbus/robotframework-PlatynUI/commit/2cacebe554d76fa8259acf4acf2627d03845bb06))
- **mock:** Make mock providers explicit-only, remove auto-registration ([65fbd53](https://github.com/imbus/robotframework-PlatynUI/commit/65fbd5369c2b7d2e07a1460bc236602eacf41fd5))
- **mock:** Modularize platform and provider scaffolds ([9d30cab](https://github.com/imbus/robotframework-PlatynUI/commit/9d30cab3ef57ddf06ee4b9d45ac96914a1171dd6))
- **mock-provider:** Drop provider-side geometry aliases ([26408e1](https://github.com/imbus/robotframework-PlatynUI/commit/26408e15b42ba1d787293ba8032f34d3eee66381))
- **parser:** Move grammar file path for XPathParser ([ed987e9](https://github.com/imbus/robotframework-PlatynUI/commit/ed987e99060ddda978c7ed8a9ae4ad52c74f9534))
- **pattern:** Enhance WindowSurfaceActions with user input handling and remove ApplicationStatus ([f4d3979](https://github.com/imbus/robotframework-PlatynUI/commit/f4d3979ae4d012c1872d253e9f2032531123dc5e))
- **patterns:** Drop window manager terminology ([766cf18](https://github.com/imbus/robotframework-PlatynUI/commit/766cf181c5ac2152796428777a645cdd609937f0))
- **pointer:** Reuse cached engine and clean overrides ([04dc88f](https://github.com/imbus/robotframework-PlatynUI/commit/04dc88fabbd1a694a2ade06ef3b3e5ac55ca825b))
- **pointer:** Simplify PointerSettings and PointerProfile initialization ([82ae325](https://github.com/imbus/robotframework-PlatynUI/commit/82ae325e5f7b9762bde85e83788ae2e4666ed3d3))
- **provide-windows-uia:** Streamline error handling with uia_api helper in COM interactions ([41fe022](https://github.com/imbus/robotframework-PlatynUI/commit/41fe0223c980fb4fe7c446df37e0fc076138e7fa))
- **provider:** Simplify lifecycle and add contract checks ([ad3f69c](https://github.com/imbus/robotframework-PlatynUI/commit/ad3f69c83209aefcca90aba0170edf031c237f99))
- **runtime:** Fold accepts user input into window surface ([78b2f7c](https://github.com/imbus/robotframework-PlatynUI/commit/78b2f7cd1888c067c125769083ea7bb95a2bdebf))
- **runtime:** Use ui node adapters for xpath ([c152ed8](https://github.com/imbus/robotframework-PlatynUI/commit/c152ed8f219600818754805a8c7647039a0afd92))
- **xpath:** Drop recursion lint allowance ([59188b8](https://github.com/imbus/robotframework-PlatynUI/commit/59188b8dc0aa542d9628371d40abe2060384a508))
- **xpath:** Enhance benchmarking with sampling mode and adjusted parameters ([09d06c7](https://github.com/imbus/robotframework-PlatynUI/commit/09d06c726e5bd37cbd4a47b36d441dab2f11d13d))
- **xpath:** Clean up code formatting and improve readability in various files ([adf60af](https://github.com/imbus/robotframework-PlatynUI/commit/adf60af566a33c5c9b60a57583a5d50b26fb527a))
- **xpath:** Replace `compile_xpath` with `compile` across various test files for consistency ([5f149e2](https://github.com/imbus/robotframework-PlatynUI/commit/5f149e2c51f003351246ad03d45c3278546a8d29))
- **xpath:** Replace children_vec with children iterator and remove redundant vector methods ([781b693](https://github.com/imbus/robotframework-PlatynUI/commit/781b69398044c4bf0f70d3e0e45da5dfc729c88c))
- **xpath:** Implement streaming evaluator with cursor-based architecture ([410627d](https://github.com/imbus/robotframework-PlatynUI/commit/410627d0586e788b72a25d0ce914ee894bf89395))
- **xpath:** Fix clippy warnings ([c38a9e8](https://github.com/imbus/robotframework-PlatynUI/commit/c38a9e85ddb9588cc67f96c53c89a91dcadfcb71))
- **xpath:** Reworked evaluator to be a streamed evaluator ([c9fa0fa](https://github.com/imbus/robotframework-PlatynUI/commit/c9fa0fa6fe6cf5e8055fa5629ada14b37be89f0d))
- **xpath:** Reduce context cloning and cache function registry ([6deb4f6](https://github.com/imbus/robotframework-PlatynUI/commit/6deb4f6f856f8cdccf590ffe72f87b9e845cb77b))
- **xpath:** Enhance error handling for context item requirements in id functions ([b6b7e1e](https://github.com/imbus/robotframework-PlatynUI/commit/b6b7e1e0b0ef4f662c5e82436dab4f07cbd2d7d3))
- **xpath:** Split default function registry into topic modules ([df5de62](https://github.com/imbus/robotframework-PlatynUI/commit/df5de62a39cebb46e645d627b993be6492a33b4a))
- **xpath:** Expand function registry and centralize namespace constants ([009e0e9](https://github.com/imbus/robotframework-PlatynUI/commit/009e0e922f0ff540489516a06ed00fddf4a9da47))
- **xpath:** Consolidation of error handling ([58915ff](https://github.com/imbus/robotframework-PlatynUI/commit/58915ff231ac4a9062ecf362e8dc94e2af75f589))
- **xpath:** Reorganize crate structure ([967fba6](https://github.com/imbus/robotframework-PlatynUI/commit/967fba64fccf832b6eccf82646a224c2eec74fa8))
- **xpath:** Simplify function implementations and improve readability in evaluator, functions, parser, and runtime modules ([a65678d](https://github.com/imbus/robotframework-PlatynUI/commit/a65678dcc2bf73bfff57b5d9ccf13a78acee22ea))
- Remove unnecessary target OS configuration for Windows ([eea7aa6](https://github.com/imbus/robotframework-PlatynUI/commit/eea7aa60f3f2ddc5b705f778929f7dedbc1b0b8e))
- Unify error handling (thiserror in libs, anyhow in CLI ([daec632](https://github.com/imbus/robotframework-PlatynUI/commit/daec63283be3d6b20f7b91a4df31936a2384523b))
- Improved readability and consistency ([357bd6a](https://github.com/imbus/robotframework-PlatynUI/commit/357bd6a001ec19df695f3fd78787aac757130158))
- Remove ProviderPriority from ProviderDescriptor and related code ([b0aa5b1](https://github.com/imbus/robotframework-PlatynUI/commit/b0aa5b19d2327c00f189b47461d6a62f67f65888))
- Reorder crates structure ([90a0d09](https://github.com/imbus/robotframework-PlatynUI/commit/90a0d09ab328880b107b52a5f89789f3382f4740))
- Rename XPath2Parser to XPathParser and update references in tests ([695bee1](https://github.com/imbus/robotframework-PlatynUI/commit/695bee11da9d029426cb21f31a048e1908bad3e5))


<!-- generated by git-cliff -->
