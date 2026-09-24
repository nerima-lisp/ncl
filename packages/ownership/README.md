# ncl-ownership

`ncl-ownership` is the test-support crate that proves a crate registered every
symbol the ownership table assigns to it. It embeds
`conformance/ownership/symbols.tsv` and exposes no Lisp values.

## Usage

Each lane adds one test to `tests/coverage.rs`:

```rust
#[test]
fn registers_every_owned_symbol() {
    let runtime = ncl_object::Runtime::new().unwrap();
    let mut ctx = ncl_object::ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_<crate>::register(&runtime).unwrap();
    ncl_ownership::assert_crate_coverage(&runtime, &mut ctx, "ncl-<crate>").unwrap();
}
```

`ncl_<crate>::register(&Runtime) -> Result<(), ObjectError>` is implemented by
each lane; `ncl-ownership` does not call it.

## Failure report

`assert_crate_coverage` prints `package::symbol (kind): reason`, one line per
unregistered symbol:

```text
COMMON-LISP::CAR (function): function not registered
COMMON-LISP::CDR (function): symbol not interned
SB-EXT::*GC-RUN-TIME* (variable): package not found
```

A crate with no Phase 1 rows returns `OwnershipError::NoRows` rather than
passing vacuously.

## Checks performed

| kind | check |
| --- | --- |
| package | `Runtime::find_package` finds it |
| symbol | `Package::find_symbol` interns it |
| `function` | `Runtime::function` has a registered function |
| `class`, `condition` | `Runtime::class` has a registered class |
| `macro`, `variable`, `constant`, `type`, `special-operator`, `other` | interned only |

## Known gaps

- `ncl-object` does not expose the symbol flags word, so the gate cannot yet
  verify the special, constant, or macro bits. `packages/object/src/layout.rs`
  defines `symbol_offset::FLAGS`, but there is no reader; those kinds are
  verified as interned only.
- Functions must be registered with `Runtime::define_function` and classes with
  `Runtime::define_class` for the gate to see them.
