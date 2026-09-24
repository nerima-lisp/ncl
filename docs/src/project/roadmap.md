# Roadmap

NCL is being rewritten. Implementation is organized into waves; the lane
table and per-lane acceptance criteria are in the [wave plan](wave-plan.md).

- Wave 0 freezes the design contracts, creates the crate skeletons, and lands the W0-1 to W0-4 gates.
- Wave 1 has 17 lanes: one per library and language crate, plus the Phase 1b and x86-64 lowering lanes.
- Wave 2 has 5 lanes.
- Wave 3 is integration: `ncl-runtime` and the `ncl` binary, then `ncl-conformance`.
- Wave 4 and later are the performance phases and the 20 contrib crates.

Phase 1a has landed on AArch64. x86-64 lowering is Wave 1's L14.
