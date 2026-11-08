//! Tests for `AArch64` instruction encoding and fixups.

#![expect(
    clippy::unwrap_used,
    reason = "The encoder tests construct valid instruction fixtures, so unwrap and expect failures identify a broken fixture or encoding."
)]
#![expect(
    clippy::too_many_lines,
    reason = "The function keeps one focused coverage matrix together so related fixtures and assertions remain reviewable as a unit."
)]

#[path = "encoder/common.rs"]
mod common;
#[path = "encoder/contracts.rs"]
mod contracts;
#[path = "encoder/decoder.rs"]
mod decoder;
#[path = "encoder/encoder_core.rs"]
mod encoder_core;
#[path = "encoder/encoder_edges.rs"]
mod encoder_edges;
#[path = "encoder/encoder_families.rs"]
mod encoder_families;
#[path = "encoder/encoder_fixups.rs"]
mod encoder_fixups;
