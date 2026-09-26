#![allow(missing_docs)]

use crate::{ContextField, RuntimeAbi, RuntimeFunction};

pub struct X86_64FixtureAbi;

impl RuntimeAbi for X86_64FixtureAbi {
    fn builtin_address(
        &self,
        _identifier: ncl_object::BuiltinIdentifier,
    ) -> Result<u64, crate::AbiError> {
        Ok(0x1000)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, crate::AbiError> {
        let layout = ncl_sys::thread_layout();
        let offset = match field {
            ContextField::SafepointRequest => layout.safepoint_request,
            _ => return Err(crate::AbiError::UnsupportedContextField(field)),
        };
        i32::try_from(offset).map_err(|_| crate::AbiError::UnsupportedContextField(field))
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, crate::AbiError> {
        match function {
            RuntimeFunction::SafepointSlow
            | RuntimeFunction::MakeClosure
            | RuntimeFunction::EnterCatch
            | RuntimeFunction::EnterUnwindProtect
            | RuntimeFunction::EnterProgv
            | RuntimeFunction::LeaveCatch
            | RuntimeFunction::LeaveUnwindProtect
            | RuntimeFunction::LeaveProgv => Ok(0x1000),
            _ => Err(crate::AbiError::UnsupportedRuntimeFunction(function)),
        }
    }

    fn constant_word(&self, name: &str) -> Option<i64> {
        (name == "function-entry:7").then_some(0x2000)
    }
}
