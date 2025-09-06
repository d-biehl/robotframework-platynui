# PlatynUI Copilot Instructions

## Project Overview
PlatynUI is a Robot Framework library for GUI automation and testing, built as a hybrid Python/Rust project. The core functionality is implemented in Rust for performance, with Python bindings for Robot Framework integration.

## Architecture

### Workspace Structure
- **Rust workspace**: Multi-crate workspace with `crates/` containing core libraries
- **Python workspace**: UV-managed Python project with Robot Framework integration
- **Hybrid build**: Rust libraries compile to Python extensions via `uv_build` backend

### Key Components
- `platynui-core`: Core UI automation primitives (Node trait, geometric types)
- `platynui-xpath`: XPath 2.0 parser/evaluator using Pest grammar (648-line `xpath2.pest`)
- `platynui-server`: Server component (currently minimal)
- `src/PlatynUI/`: Python Robot Framework library interface

## Development Workflows

### Building & Testing
```bash
# Rust components
cargo test                           # Run all Rust tests
cargo test -p platynui-xpath        # Test specific crate

# Python components  
uv sync                             # Install Python dependencies
robot tests/first.robot             # Run Robot Framework tests
```

### XPath Development
The XPath parser uses Pest PEG grammar in `crates/platynui-xpath/src/xpath2.pest`. Key patterns:
- Grammar is right-recursive (avoids PEG left-recursion issues)
- Comprehensive test suite in `tests/parser/` with rstest parameterized tests
- Debug parsing with `XPath2Parser::parse_and_debug(input)`

## Critical Patterns

### Node Abstraction
The `Node` trait in `platynui-core/src/strategies/node.rs` defines the core UI element interface:
```rust
pub trait Node: Send + Sync {
    fn parent(&self) -> Option<Weak<dyn Node>>;
    fn children(&self) -> Vec<Arc<dyn Node>>;
    fn attributes(&self) -> Vec<Arc<dyn Attribute>>;
    // ...
}
```
- Uses `Arc<dyn Node>` for shared ownership, `Weak` for parent references
- Thread-safe with `Send + Sync` bounds

### XPath Evaluator Design
The evaluator in `platynui-xpath/src/evaluator.rs` implements XDM (XQuery Data Model):
- `XdmItem` enum: Node or AtomicValue
- `XdmSequence`: Ordered collections following XPath semantics
- Designed for arbitrary tree structures, not just XML

### Test Organization
XPath tests are extensively categorized in `tests/parser/`:
- `basic_syntax.rs`: Fundamental elements (@id, ., .., *)
- `complex_expressions.rs`: Multi-operator expressions
- `xpath2_compliance.rs`: XPath 2.0 specification compliance
- Use `#[rstest]` with `#[case]` for parameterized testing

## Integration Points

### Python-Rust Bridge
- Python package built via `uv_build` backend in `pyproject.toml`
- Currently minimal (`dummy_keyword()` in `__init__.py`)
- Target: Expose Rust XPath evaluator to Robot Framework

### Robot Framework Integration
- Library interface in `src/PlatynUI/__init__.py`
- Test files in `tests/*.robot` use standard Robot Framework syntax
- Keywords should follow Robot Framework naming conventions (Title Case)

## Conventions

### Cargo Workspace
- Shared metadata in root `Cargo.toml` with `workspace.package`
- All crates inherit common fields: `version.workspace = true`
- Edition 2024 used throughout

### Error Handling
- XPath parser returns `pest::error::Error<Rule>` 
- Evaluator uses `Result` types for fallible operations
- Test assertions include descriptive messages with xpath and context

### File Organization
- Keep pest grammar in separate `.pest` files, not inline
- Separate test modules by functionality (`basic_syntax`, `operators`, etc.)
- Use `pub use` in `mod.rs` for clean public APIs
