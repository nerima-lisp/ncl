use std::rc::Rc;

use ncl_object::{HeapRootedWord, LispError, MultipleValues, ObjectError, ThreadContext, Word};
use ncl_sys::{CodeError, SafepointMapError};

use super::{FileLoadError, finish_file_load};
use crate::{Runtime, RuntimeError};

#[test]
fn code_and_safepoint_errors_become_load_conditions() -> Result<(), String> {
    let runtime = Runtime::new().map_err(|error| format!("runtime: {error:?}"))?;
    let object = Rc::clone(&runtime.object);
    let mut context = ThreadContext::new();
    context
        .register(&object)
        .map_err(|error| format!("register context: {error:?}"))?;
    let pathname = HeapRootedWord::new(&context, Word::NIL)
        .map_err(|error| format!("root pathname: {error:?}"))?;
    let mut values = MultipleValues::new();

    assert_eq!(
        finish_file_load(
            &mut context,
            &object,
            &runtime,
            &pathname,
            false,
            &mut values,
            Err(FileLoadError::Runtime(RuntimeError::Code(
                CodeError::OutOfBounds,
            ))),
        ),
        Err(ObjectError::TypeError)
    );
    assert!(matches!(
        context.take_pending_lisp_error(),
        Some(LispError::SimpleError { .. })
    ));

    assert_eq!(
        finish_file_load(
            &mut context,
            &object,
            &runtime,
            &pathname,
            false,
            &mut values,
            Err(FileLoadError::Runtime(RuntimeError::SafepointMap(
                SafepointMapError::InvalidPc,
            ))),
        ),
        Err(ObjectError::TypeError)
    );
    assert!(matches!(
        context.take_pending_lisp_error(),
        Some(LispError::SimpleError { .. })
    ));
    Ok(())
}
