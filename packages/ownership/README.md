# ncl-ownership

`ncl-ownership` is the test-support crate that proves a crate registered every
symbol in the table supplied by its coverage test. It exposes no Lisp values.

## Usage

Each lane adds one test to `tests/coverage.rs`:

```rust
#[test]
fn registers_every_owned_symbol() {
    let runtime = ncl_object::Runtime::new().unwrap();
    let mut ctx = ncl_object::ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_<crate>::register(&runtime).unwrap();
    const TABLE: &str = include_str!("../ownership.tsv");
    ncl_ownership::assert_crate_coverage_from_table(
        &runtime, &mut ctx, TABLE, "ncl-<crate>",
    ).unwrap();
}
```

`ncl_<crate>::register(&Runtime) -> Result<(), ObjectError>` is implemented by
each lane; `ncl-ownership` does not call it.

## Failure report

`assert_crate_coverage_from_table` prints `package::symbol (kind): reason`, one line per
unregistered symbol:

```text
COMMON-LISP::CAR (function): function not registered
COMMON-LISP::CDR (function): symbol not interned
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
| `macro` | `symbol_is_macro` is set |
| `variable` | `symbol_is_special` is set |
| `constant` | `symbol_is_constant` is set |
| `type`, `special-operator`, `other` | interned only |

## Known gaps

- Functions must be registered with `Runtime::define_function` and classes with
  `Runtime::define_class` for the gate to see them.
