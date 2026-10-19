# CLOS/MOP dispatch: runtime, domain model, and proposed caching

This note separates current CLOS (Common Lisp Object System) runtime dispatch,
the separate Rust domain model, and a possible future inline cache. MOP means
Metaobject Protocol.

The runtime stores each generic function's method entries under a private key
in the property list of its name symbol, as implemented in
`packages/clos/src/lib_helpers.rs`. `packages/clos/src/lib_dispatch.rs` matches
applicable methods and approximates specificity using superclass depth. For
classes with multiple direct superclasses, it uses the first parent as a
stand-in for the class precedence list; complete multiple-inheritance CPL
ordering remains an implementation gap.

Method qualifiers follow the standard order: around methods wrap the
before/primary/after combination, before methods run most-specific first, and
after methods run least-specific first. Primary methods receive the remaining
primary-method list as the `call-next-method` continuation, and
`next-method-p` reports whether that list is nonempty. Around methods receive a
separate continuation containing the remaining around methods and the core
combination.

The runtime dispatcher recomputes method matches on every call.
`packages/clos/src/domain_behavior.rs` contains a separate
`GenericFunction` model that caches applicable method IDs. Its key contains
typed arguments (`ClassId` or `EqlValueId`) and that model's own generic-function
generation. Adding a method, removing an existing method, or explicitly
invalidating for a class redefinition advances the generation and clears the
cache. The class-redefinition operation currently ignores the supplied class
ID and clears the whole cache.

Separately, `packages/clos/src/domain_core.rs` defines `Class::version()`, which
increments when inheritance is finalized or the class is redefined. This class
version and the generic-function cache generation are independent: the cache
key does not contain `Class::version()`, and runtime dispatch uses neither
domain-model value.

Runtime inline caching is not implemented. A per-call-site cache of ordered
method lists is only a sketch, not an implementation-ready design. Runtime key
details, including generic-function identity and how to distinguish arguments
with EQL specializers, remain undecided. Whether and how runtime classes map to
domain `ClassId` values or use domain `Class::version()` is also undecided, as
are invalidation for method changes or class redefinitions, cache capacity, and
eviction policy. This sketch covers method dispatch only; it makes no proposal
about slot-index caching, which the runtime dispatcher does not resolve.
