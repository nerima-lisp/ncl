#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use super::common::*;
use ncl_asm_aarch64::*;

#[test]
fn conditional_fixups_encode_negative_four_bytes() {
    assert_negative_delta(
        |label| Inst::BCond {
            cond: Cond::Eq,
            label,
        },
        0x54ff_ffe0,
    );
    assert_negative_delta(|label| Inst::Cbz { rt: x(0), label }, 0xb4ff_ffe0);
    assert_negative_delta(|label| Inst::Cbnz { rt: x(0), label }, 0xb5ff_ffe0);
    assert_negative_delta(|label| Inst::LdrLiteral { rt: x(1), label }, 0x58ff_ffe1);
    assert_negative_delta(
        |label| Inst::Tbz {
            rt: x(0),
            bit: 0,
            label,
        },
        0x3607_ffe0,
    );
    assert_negative_delta(
        |label| Inst::Tbnz {
            rt: x(0),
            bit: 0,
            label,
        },
        0x3707_ffe0,
    );
}

#[test]
fn conditional_fixups_reject_deltas_just_past_each_signed_limit() {
    assert_boundary_deltas(
        |label| Inst::BCond {
            cond: Cond::Eq,
            label,
        },
        4 * ((1 << 18) - 1),
    );
    assert_boundary_deltas(|label| Inst::Cbz { rt: x(0), label }, 4 * ((1 << 18) - 1));
    assert_boundary_deltas(|label| Inst::Cbnz { rt: x(0), label }, 4 * ((1 << 18) - 1));
    assert_boundary_deltas(
        |label| Inst::LdrLiteral { rt: x(1), label },
        4 * ((1 << 18) - 1),
    );
    assert_boundary_deltas(
        |label| Inst::Tbz {
            rt: x(0),
            bit: 0,
            label,
        },
        4 * ((1 << 13) - 1),
    );
    assert_boundary_deltas(
        |label| Inst::Tbnz {
            rt: x(0),
            bit: 0,
            label,
        },
        4 * ((1 << 13) - 1),
    );
}
