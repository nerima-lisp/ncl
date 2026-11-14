use ncl_codegen::{AbiError, ContextField, RuntimeAbi, RuntimeFunction};

pub struct X86_64FixtureAbi;

impl RuntimeAbi for X86_64FixtureAbi {
    fn builtin_address(&self, _identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        Ok(0x1000)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        let layout = ncl_sys::thread_layout();
        let offset = match field {
            ContextField::SafepointRequest => layout.safepoint_request,
            ContextField::MultipleValueArea => layout.mv,
            ContextField::Pending => layout.pending,
            ContextField::RuntimeErrorFlags => layout.runtime_error_flags,
            ContextField::MultipleValueCount => layout.mv_count,
            _ => return Err(AbiError::UnsupportedContextField(field)),
        };
        i32::try_from(offset).map_err(|_| AbiError::UnsupportedContextField(field))
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        match function {
            RuntimeFunction::SafepointSlow
            | RuntimeFunction::MakeClosure
            | RuntimeFunction::EnterCatch
            | RuntimeFunction::EnterUnwindProtect
            | RuntimeFunction::EnterProgv
            | RuntimeFunction::LeaveCatch
            | RuntimeFunction::LeaveUnwindProtect
            | RuntimeFunction::LeaveProgv
            | RuntimeFunction::SignalTypeError => Ok(0x1000),
            _ => Err(AbiError::UnsupportedRuntimeFunction(function)),
        }
    }

    fn constant_word(&self, name: &str) -> Option<i64> {
        match name {
            "function-entry:7" => Some(0x2000),
            "type-error-expected:LIST" => Some(0),
            _ => None,
        }
    }
}
