//! Integration tests for the front-end HIR builder and MIR bridge.

#[path = "support/forms.rs"]
mod forms;

mod hir_builder {
    mod basic;
    mod bindings;
    mod closures;
    mod common;
    mod control;
    mod declarations;
    mod functions;
    mod identities;
    mod literals;
    mod types;
    mod unsupported;
}
