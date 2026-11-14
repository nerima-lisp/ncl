use ncl_conditions::{ConditionIdentifier, condition_class_of, condition_from_lisp_error};
use ncl_object::typed::{FormatErrorCause, FormatParseError, FormatPrintError, LispError};
use ncl_object::{
    HeapRootedWord, Instance, ObjectError, Runtime, ThreadContext, Word, make_string, slot_ref,
};
use ncl_printer::{PrintError, PrintOptions, StringSink, write};

use crate::{FormatError, FormatErrorKind, ParseErrorKind};

use super::format_error_to_object_error;

struct MappingCase {
    control: &'static str,
    error: FormatError,
    cause: FormatErrorCause,
}

fn parse_cases() -> [MappingCase; 7] {
    [
        MappingCase {
            control: "x~z",
            error: FormatError::new(
                1,
                Some('z'),
                FormatErrorKind::Parse(ParseErrorKind::UnknownDirective),
            ),
            cause: FormatErrorCause::Parse(FormatParseError::UnknownDirective),
        },
        MappingCase {
            control: "prefix ~",
            error: FormatError::new(
                7,
                None,
                FormatErrorKind::Parse(ParseErrorKind::UnterminatedDirective),
            ),
            cause: FormatErrorCause::Parse(FormatParseError::UnterminatedDirective),
        },
        MappingCase {
            control: "xx~?A",
            error: FormatError::new(
                2,
                Some('A'),
                FormatErrorKind::Parse(ParseErrorKind::InvalidParameter),
            ),
            cause: FormatErrorCause::Parse(FormatParseError::InvalidParameter),
        },
        MappingCase {
            control: "prefix ~",
            error: FormatError::new(
                7,
                None,
                FormatErrorKind::Parse(ParseErrorKind::MissingDirective),
            ),
            cause: FormatErrorCause::Parse(FormatParseError::MissingDirective),
        },
        MappingCase {
            control: "xx~}",
            error: FormatError::new(
                2,
                Some('}'),
                FormatErrorKind::Parse(ParseErrorKind::UnexpectedTerminator),
            ),
            cause: FormatErrorCause::Parse(FormatParseError::UnexpectedTerminator),
        },
        MappingCase {
            control: "xx~{~A",
            error: FormatError::new(
                2,
                Some('{'),
                FormatErrorKind::Parse(ParseErrorKind::UnterminatedConstruct { expected: '}' }),
            ),
            cause: FormatErrorCause::Parse(FormatParseError::UnterminatedConstruct {
                expected: '}',
            }),
        },
        MappingCase {
            control: "prefix ~/",
            error: FormatError::new(
                7,
                Some('/'),
                FormatErrorKind::Parse(ParseErrorKind::UnterminatedFunctionName),
            ),
            cause: FormatErrorCause::Parse(FormatParseError::UnterminatedFunctionName),
        },
    ]
}

fn argument_cases() -> [MappingCase; 5] {
    [
        MappingCase {
            control: "prefix ~A",
            error: FormatError::new(7, Some('A'), FormatErrorKind::MissingArgument),
            cause: FormatErrorCause::MissingArgument,
        },
        MappingCase {
            control: "x~C",
            error: FormatError::new(1, Some('C'), FormatErrorKind::WrongArgumentType),
            cause: FormatErrorCause::WrongArgumentType,
        },
        MappingCase {
            control: "xx~D",
            error: FormatError::new(2, Some('D'), FormatErrorKind::InvalidParameter),
            cause: FormatErrorCause::InvalidParameter,
        },
        MappingCase {
            control: "xx~(",
            error: FormatError::new(2, Some('('), FormatErrorKind::InvalidDirectiveUse),
            cause: FormatErrorCause::InvalidDirectiveUse,
        },
        MappingCase {
            control: "xx~{",
            error: FormatError::new(2, Some('{'), FormatErrorKind::ImproperList),
            cause: FormatErrorCause::ImproperList,
        },
    ]
}

fn print_cases() -> [MappingCase; 4] {
    [
        MappingCase {
            control: "prefix ~S",
            error: FormatError::new(
                7,
                Some('S'),
                FormatErrorKind::Print(PrintError::Object(ObjectError::Unsupported)),
            ),
            cause: FormatErrorCause::Print(FormatPrintError::Object(ObjectError::Unsupported)),
        },
        MappingCase {
            control: "prefix ~S",
            error: FormatError::new(
                7,
                Some('S'),
                FormatErrorKind::Print(PrintError::Sink("sink failure".to_owned())),
            ),
            cause: FormatErrorCause::Print(FormatPrintError::Sink),
        },
        MappingCase {
            control: "prefix ~S",
            error: FormatError::new(
                7,
                Some('S'),
                FormatErrorKind::Print(PrintError::NotReadable),
            ),
            cause: FormatErrorCause::Print(FormatPrintError::NotReadable),
        },
        MappingCase {
            control: "prefix ~S",
            error: FormatError::new(
                7,
                Some('S'),
                FormatErrorKind::Print(PrintError::Circularity),
            ),
            cause: FormatErrorCause::Print(FormatPrintError::Circularity),
        },
    ]
}

#[test]
fn format_error_variants_map_condition_class_and_slots() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_conditions::register(&runtime)?;

    let arguments = [Word::fixnum(23)];
    let stream = Word::fixnum(99);
    for case in parse_cases()
        .into_iter()
        .chain(argument_cases())
        .chain(print_cases())
    {
        assert_mapping_case(&mut ctx, &runtime, &arguments, stream, &case)?;
    }
    Ok(())
}

fn assert_mapping_case(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    arguments: &[Word],
    stream: Word,
    case: &MappingCase,
) -> Result<(), ObjectError> {
    let control_word = make_string(ctx, runtime, &case.control.chars().collect::<Vec<_>>())?;
    let control = HeapRootedWord::new(ctx, control_word)?;
    let expected_object_error = match case.cause {
        FormatErrorCause::Print(FormatPrintError::Object(error)) => error,
        _ => ObjectError::TypeError,
    };
    assert_eq!(
        format_error_to_object_error(
            ctx,
            runtime,
            &case.error,
            control.get(),
            arguments,
            Some(stream),
        ),
        expected_object_error,
    );

    let Some(LispError::FormatError(error)) = ctx.take_pending_lisp_error() else {
        return Err(ObjectError::Layout);
    };
    assert_eq!(error.cause, case.cause);
    assert_eq!(error.position, case.error.position);
    assert_eq!(error.directive, case.error.directive);
    assert_eq!(error.format_control, control.get());
    assert_eq!(error.stream, Some(stream));

    let format_arguments = HeapRootedWord::new(ctx, error.format_arguments)?;
    let condition = condition_from_lisp_error(ctx, runtime, LispError::FormatError(error))?;
    let condition = HeapRootedWord::new(ctx, condition)?;
    let actual_class = condition_class_of(ctx, condition.get()).map_err(|_| ObjectError::Layout)?;
    let expected_identifier = if matches!(
        case.cause,
        FormatErrorCause::Print(FormatPrintError::Object(_))
    ) {
        ConditionIdentifier::Error
    } else {
        ConditionIdentifier::SimpleError
    };
    let expected_class = expected_identifier
        .class(ctx, runtime)
        .ok_or(ObjectError::Layout)?;
    assert_eq!(actual_class.as_word(), expected_class.as_word());
    let instance = Instance::from_word(condition.get());
    if expected_identifier == ConditionIdentifier::SimpleError {
        assert_eq!(slot_ref(ctx, instance, 0)?, control.get());
        assert_eq!(slot_ref(ctx, instance, 1)?, format_arguments.get());
        let mut sink = StringSink::new();
        write(
            ctx,
            runtime,
            format_arguments.get(),
            &mut sink,
            &PrintOptions::default(),
        )
        .map_err(|_| ObjectError::Layout)?;
        assert_eq!(sink.into_string(), "(23)");
    } else {
        assert!(slot_ref(ctx, instance, 0).is_err());
        assert!(slot_ref(ctx, instance, 1).is_err());
    }
    Ok(())
}
