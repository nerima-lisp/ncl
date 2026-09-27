use crate::CodegenError;
use ncl_ir::{Function, OpKind, Terminator};

pub(super) fn outgoing_words(function: &Function) -> Result<u32, CodegenError> {
    let mut maximum = 0_usize;
    for block in &function.blocks {
        for op in &block.ops {
            let count = match &op.kind {
                OpKind::Call { args, .. } | OpKind::CallIndirect { args, .. } => {
                    args.len().saturating_sub(1)
                }
                OpKind::CallClosure { closure, args } => {
                    let captures = function
                        .blocks
                        .iter()
                        .flat_map(|candidate| &candidate.ops)
                        .find_map(|candidate| {
                            candidate
                                .results
                                .iter()
                                .any(|(value, _)| value == closure)
                                .then_some(match &candidate.kind {
                                    OpKind::MakeClosure { captures, .. } => captures.len(),
                                    _ => 0, // check-added-lines: allow(wildcard) non-closure ops are not captures.
                                })
                        })
                        .unwrap_or(0);
                    captures.saturating_add(args.len().saturating_sub(1))
                }
                _ => 0, // check-added-lines: allow(wildcard) non-call ops need no outgoing slots.
            };
            maximum = maximum.max(extra_words(count));
        }
        let count = match &block.terminator {
            Terminator::CallReturn { args, .. } | Terminator::TailCall { args, .. } => {
                args.len().saturating_sub(1)
            }
            _ => 0, // check-added-lines: allow(wildcard) non-call terminators need no outgoing slots.
        };
        maximum = maximum.max(extra_words(count));
    }
    u32::try_from(maximum).map_err(|_| CodegenError::FrameOverflow)
}

fn extra_words(argument_count: usize) -> usize {
    let extras = argument_count.saturating_sub(4);
    extras.saturating_add(usize::from(extras > 0))
}
