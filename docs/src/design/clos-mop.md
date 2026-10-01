# CLOS dispatch and MOP design

NCL stores generic-function methods in the CLOS method table and computes an
applicable method order from the runtime class precedence list. Method
qualifiers are combined in the standard order: around methods wrap the
before/primary/after combination, before methods run most-specific first, and
after methods run least-specific first. The method continuation carries the
remaining primary methods for `call-next-method` and `next-method-p`.

The dispatch hot path is designed for a discriminating-function cache. A cache
entry is keyed by generic-function identity, call site, and the class-layout
generation of every required argument. A hit reuses the effective-method
continuation and its fixed slot locations. A miss recomputes applicability,
stores the effective method in a bounded per-site cache, and records the
current generation. Class redefinition or effective-slot finalization increments
the generation, invalidating entries without scanning call sites.

The current implementation preserves this invalidation boundary in the class
descriptor version field. Full inline-cache storage remains a follow-up after
the ANSI behavior and MOP protocol are complete.
