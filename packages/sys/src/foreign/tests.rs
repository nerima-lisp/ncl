use super::{
    AccessWidth, Alignment, ByteLength, ForeignCallError, ForeignError, ForeignFunction,
    ForeignLibrary, ForeignRegion, ForeignSignature, ForeignType, ForeignValue, SapAddress, call,
    read_sap,
};
use core::ffi::c_void;
use core::ptr::NonNull;
use std::path::Path;

fn allocate_region(length: u64) -> Result<ForeignRegion, ForeignError> {
    let alignment = Alignment::new(8)?;
    ForeignRegion::allocate(ByteLength::new(length), alignment)
}

extern "C" fn pointer_identity(pointer: *mut c_void) -> *mut c_void {
    pointer
}

extern "C" fn echo_i8(value: i8) -> i8 {
    value
}

extern "C" fn echo_u8(value: u8) -> u8 {
    value
}

#[cfg(target_arch = "x86_64")]
extern "C" fn sum_six(
    first: u64,
    second: u64,
    third: u64,
    fourth: u64,
    fifth: u64,
    sixth: u64,
) -> u64 {
    first + second + third + fourth + fifth + sixth
}

#[cfg(target_arch = "aarch64")]
extern "C" fn sum_eight(
    first: u64,
    second: u64,
    third: u64,
    fourth: u64,
    fifth: u64,
    sixth: u64,
    seventh: u64,
    eighth: u64,
) -> u64 {
    first + second + third + fourth + fifth + sixth + seventh + eighth
}

extern "C" fn read_tracked_address(pointer: *mut c_void) -> u64 {
    let Ok(raw_address) = u64::try_from(pointer.expose_provenance()) else {
        return u64::MAX;
    };
    let Ok(address) = SapAddress::new(raw_address) else {
        return u64::MAX;
    };
    read_sap(address, AccessWidth::U8).unwrap_or(u64::MAX)
}

extern "C" fn apply_callback(
    callback: extern "C" fn(*mut c_void) -> u64,
    pointer: *mut c_void,
) -> u64 {
    callback(pointer)
}

#[test]
fn library_loading_requires_an_exact_absolute_path() {
    assert!(matches!(
        ForeignLibrary::load_exact(Path::new("libc.so.6")),
        Err(ForeignError::InvalidLibraryPath)
    ));
    assert!(matches!(
        ForeignLibrary::load_exact(Path::new("/definitely/missing/ncl-foreign-library.so")),
        Err(ForeignError::LibraryLoadFailed)
    ));
}

#[cfg(all(target_family = "unix", target_arch = "x86_64", target_os = "macos"))]
#[test]
fn exact_system_library_supports_a_scalar_call() -> Result<(), Box<dyn std::error::Error>> {
    scalar_tests::scalar_abs_call("/usr/lib/libSystem.B.dylib")
}

#[cfg(all(target_family = "unix", target_arch = "aarch64", target_os = "macos"))]
#[test]
fn exact_system_library_supports_a_scalar_call() -> Result<(), Box<dyn std::error::Error>> {
    scalar_tests::scalar_abs_call("/usr/lib/libSystem.B.dylib")
}

#[cfg(all(target_family = "unix", target_arch = "x86_64", target_os = "linux"))]
#[test]
fn exact_system_library_supports_a_scalar_call() -> Result<(), Box<dyn std::error::Error>> {
    scalar_tests::scalar_abs_call("/lib/x86_64-linux-gnu/libc.so.6")
}

#[cfg(all(target_family = "unix", target_arch = "aarch64", target_os = "linux"))]
#[test]
fn exact_system_library_supports_a_scalar_call() -> Result<(), Box<dyn std::error::Error>> {
    scalar_tests::scalar_abs_call("/lib/aarch64-linux-gnu/libc.so.6")
}

#[cfg(any(
    all(target_family = "unix", target_arch = "x86_64", target_os = "macos"),
    all(target_family = "unix", target_arch = "aarch64", target_os = "macos"),
    all(target_family = "unix", target_arch = "x86_64", target_os = "linux"),
    all(target_family = "unix", target_arch = "aarch64", target_os = "linux")
))]
mod scalar_tests {
    use super::*;

    pub(super) fn scalar_abs_call(path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let library = ForeignLibrary::load_exact(Path::new(path))?;
        let signature = ForeignSignature::new(ForeignType::I32, vec![ForeignType::I32]);
        let function = library.symbol(c"abs")?;
        assert!(matches!(
            library.symbol(c"ncl_symbol_that_does_not_exist"),
            Err(ForeignError::SymbolNotFound)
        ));
        assert_signature_errors(&library, &function, &signature)?;
        assert_eq!(
            call(&function, &signature, &[ForeignValue::I32(-9)]),
            Ok(ForeignValue::I32(9))
        );
        assert_integer_width_calls(&library)?;
        assert_register_argument_calls(&library)?;
        assert_pointer_calls(&library)?;
        Ok(())
    }

    fn assert_signature_errors(
        library: &ForeignLibrary,
        function: &ForeignFunction<'_>,
        signature: &ForeignSignature,
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            call(function, signature, &[]),
            Err(ForeignCallError::ArgumentCountMismatch)
        );
        assert_eq!(
            call(function, signature, &[ForeignValue::U32(9)]),
            Err(ForeignCallError::ArgumentTypeMismatch)
        );
        let single_argument = ForeignSignature::new(ForeignType::I32, vec![ForeignType::F32]);
        assert_eq!(
            call(function, &single_argument, &[ForeignValue::F32(1.5)]),
            Err(ForeignCallError::UnsupportedAbi)
        );
        let double_argument = ForeignSignature::new(ForeignType::I32, vec![ForeignType::F64]);
        assert_eq!(
            call(function, &double_argument, &[ForeignValue::F64(1.5)]),
            Err(ForeignCallError::UnsupportedAbi)
        );
        let single_result = ForeignSignature::new(ForeignType::F32, vec![ForeignType::I32]);
        assert_eq!(
            call(function, &single_result, &[ForeignValue::I32(-9)]),
            Err(ForeignCallError::UnsupportedAbi)
        );
        let double_result = ForeignSignature::new(ForeignType::F64, vec![ForeignType::I32]);
        assert_eq!(
            call(function, &double_result, &[ForeignValue::I32(-9)]),
            Err(ForeignCallError::UnsupportedAbi)
        );
        let void_argument = ForeignSignature::new(ForeignType::I32, vec![ForeignType::Void]);
        assert_eq!(
            call(function, &void_argument, &[ForeignValue::Void]),
            Err(ForeignCallError::InvalidSignature)
        );
        let variadic_signature =
            ForeignSignature::variadic(ForeignType::I32, vec![ForeignType::I32]);
        let variadic = library.symbol(c"abs")?;
        assert_eq!(
            call(&variadic, &variadic_signature, &[ForeignValue::I32(-9)]),
            Err(ForeignCallError::UnsupportedAbi)
        );
        #[cfg(target_arch = "x86_64")]
        assert_stack_argument_rejected(library, 7)?;
        #[cfg(target_arch = "aarch64")]
        assert_stack_argument_rejected(library, 9)?;
        Ok(())
    }

    fn assert_stack_argument_rejected(
        library: &ForeignLibrary,
        count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let signature = ForeignSignature::new(ForeignType::U64, vec![ForeignType::U64; count]);
        let function = library.symbol(c"abs")?;
        let values = vec![ForeignValue::U64(1); count];
        assert_eq!(
            call(&function, &signature, &values),
            Err(ForeignCallError::UnsupportedAbi)
        );
        Ok(())
    }

    fn assert_integer_width_calls(
        library: &ForeignLibrary,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let signed_address =
            NonNull::new(echo_i8 as *const () as *mut c_void).ok_or(ForeignError::NullAddress)?;
        let signed = ForeignFunction::from_address(signed_address, library);
        assert_eq!(
            call(
                &signed,
                &ForeignSignature::new(ForeignType::I8, vec![ForeignType::I8]),
                &[ForeignValue::I8(-128)],
            ),
            Ok(ForeignValue::I8(-128))
        );

        let unsigned_address =
            NonNull::new(echo_u8 as *const () as *mut c_void).ok_or(ForeignError::NullAddress)?;
        let unsigned = ForeignFunction::from_address(unsigned_address, library);
        assert_eq!(
            call(
                &unsigned,
                &ForeignSignature::new(ForeignType::U8, vec![ForeignType::U8]),
                &[ForeignValue::U8(255)],
            ),
            Ok(ForeignValue::U8(255))
        );
        Ok(())
    }

    fn assert_register_argument_calls(
        library: &ForeignLibrary,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(target_arch = "x86_64")]
        {
            let address = NonNull::new(sum_six as *const () as *mut c_void)
                .ok_or(ForeignError::NullAddress)?;
            let function = ForeignFunction::from_address(address, library);
            assert_eq!(
                call(
                    &function,
                    &ForeignSignature::new(ForeignType::U64, vec![ForeignType::U64; 6]),
                    &[ForeignValue::U64(1); 6],
                ),
                Ok(ForeignValue::U64(6))
            );
        }
        #[cfg(target_arch = "aarch64")]
        {
            let address = NonNull::new(sum_eight as *const () as *mut c_void)
                .ok_or(ForeignError::NullAddress)?;
            let function = ForeignFunction::from_address(address, library);
            assert_eq!(
                call(
                    &function,
                    &ForeignSignature::new(ForeignType::U64, vec![ForeignType::U64; 8]),
                    &[ForeignValue::U64(1); 8],
                ),
                Ok(ForeignValue::U64(8))
            );
        }
        Ok(())
    }

    fn assert_pointer_calls(library: &ForeignLibrary) -> Result<(), Box<dyn std::error::Error>> {
        assert_memset_call(library)?;
        assert_pointer_identity_calls(library)?;
        assert_tracked_callback_call(library)?;
        Ok(())
    }

    fn assert_memset_call(library: &ForeignLibrary) -> Result<(), Box<dyn std::error::Error>> {
        let signature = ForeignSignature::new(
            ForeignType::Pointer,
            vec![ForeignType::Pointer, ForeignType::I32, ForeignType::U64],
        );
        let memset = library.symbol(c"memset")?;
        let region = allocate_region(8)?;
        assert_eq!(
            call(
                &memset,
                &signature,
                &[
                    ForeignValue::Pointer(region.address()),
                    ForeignValue::I32(0x5a),
                    ForeignValue::U64(8),
                ],
            ),
            Ok(ForeignValue::Pointer(region.address()))
        );
        assert_eq!(
            region.read(region.address(), AccessWidth::U64),
            Ok(0x5a5a_5a5a_5a5a_5a5a)
        );
        Ok(())
    }

    fn assert_pointer_identity_calls(
        library: &ForeignLibrary,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let identity_address = NonNull::new(pointer_identity as *const () as *mut c_void)
            .ok_or(ForeignError::NullAddress)?;
        let identity = ForeignFunction::from_address(identity_address, library);
        let signature = ForeignSignature::new(ForeignType::Pointer, vec![ForeignType::Pointer]);
        let null_pointer = SapAddress::new(0)?;
        assert_eq!(
            call(
                &identity,
                &signature,
                &[ForeignValue::Pointer(null_pointer)],
            ),
            Ok(ForeignValue::Pointer(null_pointer))
        );
        let mut opaque_storage = [0_u8; 1];
        let opaque_raw_address = u64::try_from(opaque_storage.as_mut_ptr().expose_provenance())
            .map_err(|_| ForeignError::AddressOutOfRange)?;
        let opaque_address = SapAddress::new(opaque_raw_address)?;
        assert_eq!(
            call(
                &identity,
                &signature,
                &[ForeignValue::Pointer(opaque_address)],
            ),
            Ok(ForeignValue::Pointer(opaque_address))
        );
        assert_eq!(
            read_sap(opaque_address, AccessWidth::U8),
            Err(ForeignError::UnknownRegion)
        );

        let tracked = allocate_region(8)?;
        assert_eq!(
            call(
                &identity,
                &signature,
                &[ForeignValue::Pointer(tracked.address())],
            ),
            Ok(ForeignValue::Pointer(tracked.address()))
        );
        assert_eq!(read_sap(tracked.address(), AccessWidth::U8), Ok(0));
        Ok(())
    }

    fn assert_tracked_callback_call(
        library: &ForeignLibrary,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tracked = allocate_region(8)?;
        let callback_address = NonNull::new(read_tracked_address as *const () as *mut c_void)
            .ok_or(ForeignError::NullAddress)?;
        let apply_address = NonNull::new(apply_callback as *const () as *mut c_void)
            .ok_or(ForeignError::NullAddress)?;
        let apply = ForeignFunction::from_address(apply_address, library);
        let callback_raw_address = u64::try_from(callback_address.as_ptr().expose_provenance())
            .map_err(|_| ForeignError::AddressOutOfRange)?;
        let callback_sap = SapAddress::new(callback_raw_address)?;
        assert_eq!(
            call(
                &apply,
                &ForeignSignature::new(ForeignType::U64, vec![ForeignType::Pointer; 2]),
                &[
                    ForeignValue::Pointer(callback_sap),
                    ForeignValue::Pointer(tracked.address()),
                ],
            ),
            Ok(ForeignValue::U64(0))
        );
        Ok(())
    }
}
