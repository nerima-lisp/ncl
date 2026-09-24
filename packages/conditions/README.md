# ncl-conditions

`ncl-conditions` is the runtime side of the Common Lisp condition system for
NCL. It depends only on `ncl-object`.

## Public API

```rust
pub fn register(runtime: &Runtime) -> Result<(), ObjectError>

pub enum ConditionError { Unhandled, NotACondition, RestartNotFound, ChainCorrupt, Object(ObjectError) }

pub struct ConditionClass(Word);          // as_word, from_word
pub fn condition_class(ctx, runtime, name: &str) -> Option<ConditionClass>
pub fn condition_class_name(ctx, class) -> Result<Word, ConditionError>
pub fn condition_class_of(ctx, condition) -> Result<ConditionClass, ConditionError>
pub fn make_condition(ctx, runtime, class, slots: &[Word]) -> Result<Word, ConditionError>

pub fn signal(ctx, condition) -> Result<(), ConditionError>
pub fn error(ctx, condition) -> Result<(), ConditionError>
pub fn warn(ctx, condition) -> Result<(), ConditionError>
pub fn cerror(ctx, runtime, continue_control, continue_args, condition) -> Result<(), ConditionError>

pub struct HandlerChain(Word);
pub fn push_handler(ctx, runtime, class, handler) -> Result<HandlerChain, ConditionError>
pub fn pop_handler(ctx, chain)

pub struct RestartRecord(Word);
pub fn push_restart(ctx, runtime, name, function, report, interactive, test) -> Result<RestartRecord, ConditionError>
pub fn pop_restart(ctx, record)
pub fn find_restart(ctx, name) -> Result<Option<Word>, ConditionError>
pub fn compute_restarts(ctx, runtime) -> Result<Word, ConditionError>
pub fn invoke_restart(ctx, restart) -> Result<Word, ConditionError>
pub fn invoke_restart_by_name(ctx, name) -> Result<Word, ConditionError>

pub struct CleanupRecord(Word);
pub fn push_cleanup(ctx, runtime, entry) -> Result<CleanupRecord, ConditionError>
pub fn pop_cleanup(ctx, record)
pub fn unwind(ctx)
```

## Representation

A condition class is a minimal descriptor: a simple vector
`[name, superclass, slots]`, where `superclass` is the descriptor of the
direct superclass (NIL at the root `condition`) and `slots` is a deferred slot
specification. A condition instance is a CLOS-style `make_instance` whose class
word is that descriptor. L18 (`ncl-clos`) can formalize this into a real class
later.

Handler, restart, cleanup, and catch records are heap-allocated simple vectors
chained by a `previous` slot. The chain heads live in `ThreadContext`'s three
current pointers (handler, cleanup, catch), matching the native backend
contract. The handler pointer carries both handler and restart records,
distinguished by a leading kind tag, because restarts share the dynamic extent
of the surrounding handlers.

## Standard hierarchy

The standard hierarchy is installed with single-superclass links. Multiple
inheritance (for example, `simple-error` being both a `simple-condition` and an
`error`) is a Wave-2 concern.

```
condition
├── warning
│   ├── style-warning
│   └── (simple-warning, via simple-condition)
├── serious-condition
│   ├── error
│   │   ├── arithmetic-error ├─ division-by-zero, floating-point-*
│   │   ├── cell-error ├─ unbound-variable, undefined-function, unbound-slot
│   │   ├── type-error └─ simple-type-error
│   │   ├── file-error ├─ file-does-not-exist, file-exists
│   │   ├── stream-error └─ end-of-file
│   │   ├── package-error ├─ package-locked-error └─ symbol-package-locked-error
│   │   ├── parse-error └─ reader-error
│   │   ├── program-error, control-error, print-not-readable, ...
│   └── storage-condition └─ memory-fault-error
└── simple-condition ├─ simple-error, simple-warning, simple-type-error
```

## Macro expanders

`handler-bind`, `handler-case`, and `restart-case` are macros owned by
`ncl-lib-macros` (L19). This crate provides the record-chain primitives
(`push_handler`, `push_restart`, `push_cleanup`, `unwind`) and the signalling
entry points that those expanders lower to. No macro or special-operator flags
are set here.

## Phase-1 limitations

- Records are heap vectors, not machine stack frames. A garbage collection
  while a record chain is active is out of scope until L13/L14 move records to
  stack frames with raw pointers.
- `invoke_restart` and `unwind` model the control transfer as a pending
  non-local exit (`set_non_local_exit`); the machine-side transfer to the
  selected `target_pc` is the L13/L14 unwinder work.
- `signal` returns without invoking a handler function: there is no generated
  code to call in Phase 1, so a matching handler only marks the condition as
  handled.
