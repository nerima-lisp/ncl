//! Integration tests for the front-end HIR builder and MIR bridge.

#[path = "support/forms.rs"]
mod forms;

mod hir_builder {
    mod basic;
    mod binding_edges;
    mod bindings;
    mod bridge_errors;
    mod closures;
    mod common;
    mod control;
    mod coverage_edges;
    mod declarations;
    mod dynamic_control;
    mod functions;
    mod identities;
    mod literal_lowering;
    mod literals;
    mod lowering_paths;
    mod multiple_values;
    mod types;
    mod unsupported;
}
