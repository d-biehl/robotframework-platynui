# Provider Plugins — Constraints and Option Space

**Status: notes, not a decision.** Captured while designing the Java agent provider so the constraints do not have to be re-derived. The actual proposal is future work; nothing here is committed to.

## Why this comes up

Two wishes point at the same mechanism:

1. **Optional providers.** Java agent support already ships as its own Python package (`platynui-provider-java`, see the `provider-java-swing` change) — install it and Java works, omit it and it does not exist. Extending that to other technologies (a WPF provider instead of UIA for WPF apps, third-party providers) means providers become distributable units rather than code linked into one binary.
2. **Selecting providers per session.** That part needs *no* plugin mechanism — see the `runtime-provider-selection` change. Selection gates what is already present; plugins change what is present at all. Keeping the two apart matters: selection is cheap and near-term, plugin loading is neither.

Discovery is already solved in a forward-compatible way: the `platynui.providers` entry-point group (`importlib.metadata`), introduced by the Java agent change. Whatever the loading mechanism becomes, *finding* installed providers can stay Python-package-based.

## The hard part: Rust has no stable ABI

Loading a provider as a Rust `cdylib` and passing Rust types across the boundary is only sound when both sides were built identically. "Same compiler" understates the requirement — all of the following must match:

- **rustc down to the patch version.** `repr(Rust)` layout, niche optimizations and symbol mangling may change between releases.
- **Profile and codegen flags** (`opt-level`, `panic=abort` vs `unwind`, LTO).
- **The exact dependency graph of every interface crate.** Any type from `platynui-core` crossing the boundary must come from the identically compiled crate version — **including feature unification**, since an extra feature can change a struct's layout.
- **Allocator and `std` coupling.** A `cdylib` links its own `std` by default: a `String` allocated in the plugin and dropped in the host crosses two allocator instances; unwinding a panic across the boundary is UB; and every global becomes duplicated — `inventory`-style registration (how providers register today), loggers, `OnceCell`s.

A violation is not a clean error. It is silent memory corruption at some later call.

### What the lockstep argument does and does not buy

PlatynUI builds and publishes all packages together with `==` pins, so first-party plugins built in one CI run genuinely satisfy the list above. That makes a plain `cdylib` defensible **for first-party artifacts**. It does not make the failure mode safe: even then a load-time handshake is warranted (plugin exports its rustc version + an interface hash, host refuses on mismatch before the first call). For **third-party** plugins, "document the required toolchain" is not a safeguard — a mistake produces UB, not a message.

### What `abi_stable` actually provides

It mechanizes exactly the discipline above:

- `repr(C)` replacements for std types (`RString`, `RVec`, `RBox`, `RArc`, `ROption`, `RResult`, `RHashMap`) that carry their deallocation function as a pointer, so memory is always freed by the module that allocated it.
- `#[sabi_trait]` — FFI-safe trait objects with an explicit `repr(C)` vtable, which is what a `UiTreeProvider` plugin interface needs.
- `StableAbi` derive plus a **load-time layout check**: each interface type's layout is described at compile time and compared when the library loads, so incompatibility is a clean load error instead of UB. Plus semver checking of the root module (one versioned struct of function pointers as the single C-ABI export).
- Panics caught at the boundary and returned as `RResult`.

The cost is a mirrored interface surface: today's trait signatures (trait objects, iterators, XPath value types) need `abi_stable` equivalents on both sides — a real, ongoing maintenance layer.

## Option space

| Option | Fits | Cost |
|---|---|---|
| **A. Rust `cdylib`, plain** | first-party only, strict lockstep | UB on any mismatch ⇒ wants a load-time handshake anyway; Rust-only |
| **B. Rust `cdylib` via `abi_stable`** | third-party Rust plugins | mirrored interface layer to maintain; still Rust-only |
| **C. C-ABI by hand** | any language | narrowest interface, most manual marshalling |
| **D. Out-of-process + RPC** | any language, incl. .NET/WPF | latency, process lifecycle; **already prototyped** — the Java agent is exactly this shape (own process, versioned NDJSON-RPC wire, handshake-file rendezvous, entry-point distribution) |

Two observations worth keeping:

- **A WPF provider can never be option A or B.** .NET code cannot be a Rust-ABI plugin, so any cross-language ambition lands in C or D regardless of what is chosen for Rust plugins. A hybrid (D for foreign languages, A/B for Rust) is therefore likely, not a compromise.
- **Option D exists in the codebase before the proposal does.** If the generalized plugin protocol is distilled from the *hardened* agent wire rather than designed speculatively alongside it, it starts from something that survived contact with reality. That is the main argument for writing this proposal *after* the agent lane, not before.

## Related

- `provider-java-swing` change — decision 9 (delivery as its own wheel, `platynui.providers` entry point) and decisions 1a/7 (the RPC wire that option D would generalize).
- `runtime-provider-selection` change — selection without dynamic loading; the near-term half of the wish.
- [`architecture.md`](architecture.md) — how providers are registered today (`inventory`, linked in).
