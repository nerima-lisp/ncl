use ncl_conditions::{ConditionIdentifier, condition_class_of, condition_from_lisp_error};
use ncl_object::typed::{FormatErrorCause, FormatParseError, FormatPrintError, LispError};
use ncl_object::{
    HeapRootedWord, Instance, ObjectError, Runtime, ThreadContext, Word, make_string, slot_ref,
};
use ncl_printer::{PrintError, PrintOptions, StringSink, write};

use crate::{FormatError, FormatErrorKind, ParseErrorKind};

use super::format_error_to_object_error;

#[test]
fn format_error_kinds_map_simple_error_slots_and_keep_typed_context() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_conditions::register(&runtime)?;

    let argument = Word::fixnum(23);
    let arguments = [argument];
    let stream = Word::fixnum(99);

    let cases = [
        (
            "~z",
            FormatError::new(
                0,
                Some('z'),
                FormatErrorKind::Parse(ParseErrorKind::UnknownDirective),
            ),
            FormatErrorCause::Parse(FormatParseError::UnknownDirective),
        ),
        (
            "~A",
            FormatError::new(0, Some('A'), FormatErrorKind::MissingArgument),
            FormatErrorCause::MissingArgument,
        ),
        (
            "~C",
            FormatError::new(0, Some('C'), FormatErrorKind::WrongArgumentType),
            FormatErrorCause::WrongArgumentType,
        ),
        (
            "~D",
            FormatError::new(0, Some('D'), FormatErrorKind::InvalidParameter),
            FormatErrorCause::InvalidParameter,
        ),
        (
            "~(",
            FormatError::new(0, Some('('), FormatErrorKind::InvalidDirectiveUse),
            FormatErrorCause::InvalidDirectiveUse,
        ),
        (
            "~{",
            FormatError::new(0, Some('{'), FormatErrorKind::ImproperList),
            FormatErrorCause::ImproperList,
        ),
        (
            "~S",
            FormatError::new(
                0,
                Some('S'),
                FormatErrorKind::Print(PrintError::Object(ObjectError::Unsupported)),
            ),
            FormatErrorCause::Print(FormatPrintError::Object(ObjectError::Unsupported)),
        ),
        (
            "~S",
            FormatError::new(
                0,
                Some('S'),
                FormatErrorKind::Print(PrintError::Sink("sink failure".to_owned())),
            ),
            FormatErrorCause::Print(FormatPrintError::Sink),
        ),
        (
            "~S",
            FormatError::new(
                0,
                Some('S'),
                FormatErrorKind::Print(PrintError::NotReadable),
            ),
            FormatErrorCause::Print(FormatPrintError::NotReadable),
        ),
        (
            "~S",
            FormatError::new(
                0,
                Some('S'),
                FormatErrorKind::Print(PrintError::Circularity),
            ),
            FormatErrorCause::Print(FormatPrintError::Circularity),
        ),
    ];

    for (control_text, format_error, expected_cause) in cases {
        let control_word = make_string(
            &mut ctx,
            &runtime,
            &control_text.chars().collect::<Vec<_>>(),
        )?;
        let control = HeapRootedWord::new(&ctx, control_word)?;
        let expected_object_error = match expected_cause {
            FormatErrorCause::Print(FormatPrintError::Object(error)) => error,
            _ => ObjectError::TypeError,
        };
        assert_eq!(
            format_error_to_object_error(
                &mut ctx,
                &runtime,
                &format_error,
                control.get(),
                &arguments,
                Some(stream),
            ),
            expected_object_error,
        );

        let Some(LispError::FormatError(error)) = ctx.take_pending_lisp_error() else {
            return Err(ObjectError::Layout);
        };
        assert_eq!(error.cause, expected_cause);
        assert_eq!(error.position, format_error.position);
        assert_eq!(error.directive, format_error.directive);
        assert_eq!(error.format_control, control.get());
        assert_eq!(error.stream, Some(stream));

        let format_arguments = HeapRootedWord::new(&ctx, error.format_arguments)?;
        let condition =
            condition_from_lisp_error(&mut ctx, &runtime, LispError::FormatError(error))?;
        let condition = HeapRootedWord::new(&ctx, condition)?;
        let actual_class =
            condition_class_of(&ctx, condition.get()).map_err(|_| ObjectError::Layout)?;
        let expected_identifier = if matches!(
            expected_cause,
            FormatErrorCause::Print(FormatPrintError::Object(_))
        ) {
            ConditionIdentifier::Error
        } else {
            ConditionIdentifier::SimpleError
        };
        let expected_class = expected_identifier
            .class(&mut ctx, &runtime)
            .ok_or(ObjectError::Layout)?;
        assert_eq!(actual_class.as_word(), expected_class.as_word());
        if expected_identifier == ConditionIdentifier::SimpleError {
            assert_eq!(
                slot_ref(&ctx, Instance::from_word(condition.get()), 0)?,
                control.get(),
                "SIMPLE-ERROR :format-control",
            );
            assert_eq!(
                slot_ref(&ctx, Instance::from_word(condition.get()), 1)?,
                format_arguments.get(),
                "SIMPLE-ERROR :format-arguments",
            );
            let mut sink = StringSink::new();
            write(
                &mut ctx,
                &runtime,
                format_arguments.get(),
                &mut sink,
                &PrintOptions::default(),
            )
            .map_err(|_| ObjectError::Layout)?;
            assert_eq!(sink.into_string(), "(23)");
        } else {
            assert!(slot_ref(&ctx, Instance::from_word(condition.get()), 0).is_err());
        }
    }
    Ok(())
}
