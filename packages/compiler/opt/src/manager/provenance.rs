use ncl_ir::{Function, VerifyError};

use crate::Module;

pub(super) fn address_liveness_errors(function: &Function) -> Vec<VerifyError> {
    ncl_ir::verify(function)
        .err()
        .unwrap_or_default()
        .into_iter()
        .filter(|error| {
            matches!(
                error,
                VerifyError::AddressLiveAcross { .. }
                    | VerifyError::UnprovenAddressProvenance { .. }
            )
        })
        .collect()
}

pub(super) fn added_provenance_errors(
    previous: &[VerifyError],
    function: &Function,
) -> Vec<VerifyError> {
    let mut unmatched = previous.to_vec();
    let mut added = Vec::new();
    for error in address_liveness_errors(function) {
        if let Some(index) = unmatched.iter().position(|existing| existing == &error) {
            unmatched.remove(index);
        } else {
            added.push(error);
        }
    }
    added
}

pub(super) fn new_module_provenance_errors(
    before: &Module,
    after: &Module,
) -> Vec<(String, VerifyError)> {
    let mut added = Vec::new();
    for function in &after.functions {
        let previous = before
            .functions
            .iter()
            .find(|candidate| candidate.id == function.id)
            .map_or_else(Vec::new, address_liveness_errors);
        added.extend(
            added_provenance_errors(&previous, function)
                .into_iter()
                .map(|error| (function.name.clone(), error)),
        );
    }
    added
}
