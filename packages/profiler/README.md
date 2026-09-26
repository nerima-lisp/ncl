# ncl-profiler

`ncl-profiler` provides statistical stack profiling. The domain layer accepts
logical root-to-leaf samples and produces flat self counts, cumulative counts,
caller/callee edges, and folded stacks. Runtime adapters resolve published code
metadata through the existing `CodeRegistry` and keep Lisp `Word` values inside
the adapter boundary.

`ProfileSession` is the thread-safe Rust service. `start` resets a session,
`record` accepts samples from cooperative workers, `stop` returns an immutable
snapshot, and `report` renders a live snapshot. `LispProfileApi` exposes typed
Rust equivalents of `NCL-PROFILER:PROFILE-START`,
`NCL-PROFILER:PROFILE-STOP`, and `NCL-PROFILER:PROFILE-REPORT` for the Lisp
registration layer.

## Sampling limits

Sampling is cooperative and snapshot-based. It observes a thread only after the
runtime has published a frame snapshot at a safepoint; it is not an asynchronous
interrupt sampler. A snapshot can therefore miss short-lived work, native code
outside a published safepoint map, and threads that do not reach a safepoint.
The adapter also applies explicit bounds to snapshot words and frame depth.
Unknown PCs and missing safepoint maps are reported as errors rather than
silently represented as samples.
