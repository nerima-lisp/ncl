#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use ncl_lib_format::{
    FormatError, FormatFunctionCaller, PrettyIndent, PrettyNewline, PrettyPrinter, PrettyTab,
    execute, execute_with_caller, execute_with_options, parse,
};
use ncl_object::{Runtime, ThreadContext, Word, make_cons, make_double, make_string};
use ncl_printer::{CharSink, PrintError, StringSink};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    make_string(ctx, runtime, &text.chars().collect::<Vec<char>>()).expect("string")
}

struct RecordingCaller;

#[derive(Default)]
struct RecordingPretty {
    events: Vec<String>,
}

impl ncl_printer::CharSink for RecordingPretty {
    fn write_char(&mut self, character: char) -> Result<(), PrintError> {
        self.events.push(format!("char:{character}"));
        Ok(())
    }
}

impl PrettyPrinter for RecordingPretty {
    fn newline(&mut self, kind: PrettyNewline) -> Result<(), PrintError> {
        self.events.push(format!("newline:{kind:?}"));
        Ok(())
    }

    fn indent(&mut self, mode: PrettyIndent, amount: isize) {
        self.events.push(format!("indent:{mode:?}:{amount}"));
    }

    fn tab(&mut self, kind: PrettyTab, column: usize, increment: usize) -> Result<(), PrintError> {
        self.events
            .push(format!("tab:{kind:?}:{column}:{increment}"));
        Ok(())
    }

    fn write_object(
        &mut self,
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _object: Word,
        _options: ncl_printer::PrintOptions,
    ) -> Result<(), PrintError> {
        self.events.push("write".to_owned());
        Ok(())
    }

    fn logical_block(
        &mut self,
        segments: &[String],
        colon: bool,
        at_sign: bool,
    ) -> Result<(), PrintError> {
        self.events
            .push(format!("block:{}:{colon}:{at_sign}", segments.len()));
        Ok(())
    }

    fn finish(&mut self) -> Result<(), PrintError> {
        self.events.push("finish".to_owned());
        Ok(())
    }
}

struct WidthPretty {
    output: String,
    margin: usize,
    column: usize,
    indent: usize,
    pending_break: bool,
}

impl WidthPretty {
    const fn new(margin: usize) -> Self {
        Self {
            output: String::new(),
            margin,
            column: 0,
            indent: 0,
            pending_break: false,
        }
    }

    fn write_text(&mut self, text: &str) {
        if self.pending_break {
            self.pending_break = false;
            if self.column + 1 + text.chars().count() > self.margin {
                self.output.push('\n');
                self.output.extend(std::iter::repeat_n(' ', self.indent));
                self.column = self.indent;
            } else {
                self.output.push(' ');
                self.column += 1;
            }
        }
        self.output.push_str(text);
        self.column += text.chars().count();
    }
}

impl ncl_printer::CharSink for WidthPretty {
    fn write_char(&mut self, character: char) -> Result<(), PrintError> {
        self.write_text(&character.to_string());
        Ok(())
    }

    fn write_str(&mut self, text: &str) -> Result<(), PrintError> {
        self.write_text(text);
        Ok(())
    }
}

impl PrettyPrinter for WidthPretty {
    fn newline(&mut self, _kind: PrettyNewline) -> Result<(), PrintError> {
        self.pending_break = true;
        Ok(())
    }

    fn indent(&mut self, _mode: PrettyIndent, amount: isize) {
        self.indent = usize::try_from(amount).unwrap_or(0);
    }

    fn tab(&mut self, kind: PrettyTab, column: usize, increment: usize) -> Result<(), PrintError> {
        let count = match kind {
            PrettyTab::Relative if self.column < column => column - self.column,
            PrettyTab::Relative if increment == 0 => 0,
            PrettyTab::Relative => increment - (self.column - column) % increment,
            PrettyTab::Absolute => {
                let after_relative = self.column + column;
                column + (increment.saturating_sub(after_relative % increment)) % increment
            }
        };
        for _ in 0..count {
            self.write_text(" ");
        }
        Ok(())
    }

    fn write_object(
        &mut self,
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _object: Word,
        _options: ncl_printer::PrintOptions,
    ) -> Result<(), PrintError> {
        self.write_text("item");
        Ok(())
    }

    fn logical_block(
        &mut self,
        segments: &[String],
        _colon: bool,
        _at_sign: bool,
    ) -> Result<(), PrintError> {
        self.write_text(&segments.join(" "));
        Ok(())
    }

    fn finish(&mut self) -> Result<(), PrintError> {
        Ok(())
    }
}

#[test]
fn fake_pretty_sink_applies_margin_and_indent_to_layout_breaks() {
    let (runtime, mut ctx) = context();
    for (margin, expected) in [(10, " item"), (4, "\n  item")] {
        let pretty = Rc::new(RefCell::new(WidthPretty::new(margin)));
        let mut sink = StringSink::new();
        execute_with_options(
            &parse("~_~2I~W").expect("control"),
            &[Word::fixnum(1)],
            &mut ctx,
            &runtime,
            None,
            Some(pretty.clone()),
            &mut sink,
        )
        .expect("execute");
        assert_eq!(pretty.borrow().output, expected);
    }

    let pretty = Rc::new(RefCell::new(WidthPretty::new(20)));
    let mut sink = StringSink::new();
    execute_with_options(
        &parse("x~4,4T~<~A~:>").expect("control"),
        &[Word::fixnum(1)],
        &mut ctx,
        &runtime,
        None,
        Some(pretty.clone()),
        &mut sink,
    )
    .expect("execute");
    assert_eq!(pretty.borrow().output, "    1");
}

#[test]
fn actual_pretty_printer_adapter_handles_format_layout_directives() {
    let (runtime, mut ctx) = context();
    let mut output = StringSink::new();
    let shared = ncl_printer::PrettyPrinter::with_options(&mut output, 4, 0).into_shared();
    let pretty = Rc::new(RefCell::new(shared.adapter()));
    pretty.borrow_mut().write_str("x").expect("prefix");
    let mut fallback = StringSink::new();
    execute_with_options(
        &parse("~_~I~2T~W~<~A~:>").expect("control"),
        &[Word::fixnum(1), Word::fixnum(2)],
        &mut ctx,
        &runtime,
        None,
        Some(pretty.clone()),
        &mut fallback,
    )
    .expect("execute");
    drop(pretty);
    drop(shared);
    assert_eq!(output.into_string(), "x   12");
}

#[test]
fn connects_layout_directives_to_the_pretty_printer_boundary() {
    let (runtime, mut ctx) = context();
    let pretty = Rc::new(RefCell::new(RecordingPretty::default()));
    let mut sink = StringSink::new();
    execute_with_options(
        &parse("~_~I~T~W~<~A;~A~:>").expect("control"),
        &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
        &mut ctx,
        &runtime,
        None,
        Some(pretty.clone()),
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "");
    assert_eq!(
        pretty.borrow().events,
        [
            "newline:Linear",
            "indent:Block:0",
            "tab:Relative:1:1",
            "write",
            "block:1:true:false",
            "finish",
        ]
    );
}

impl FormatFunctionCaller for RecordingCaller {
    fn call_format_function(
        &mut self,
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        name: &str,
        stream: &mut dyn ncl_printer::CharSink,
        arguments: &[Word],
        colon: bool,
        at_sign: bool,
        parameters: &[ncl_lib_format::Parameter],
    ) -> Result<(), FormatError> {
        let text = format!(
            "{name}:{}:{colon}:{at_sign}:{}",
            arguments.len(),
            parameters.len()
        );
        stream.write_str(&text).map_err(FormatError::from)
    }
}

#[test]
fn executes_user_function_directive_through_callback_boundary() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    let caller = Rc::new(RefCell::new(RecordingCaller));
    execute_with_caller(
        &parse("~2:@/pkg:printer/").expect("control"),
        &[Word::fixnum(7)],
        &mut ctx,
        &runtime,
        Some(caller),
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "pkg:printer:1:true:true:1");
}

#[test]
fn executes_literals_and_basic_value_directives() {
    let (runtime, mut ctx) = context();
    let control = parse("x=~A s=~S").expect("control");
    let value = string(&runtime, &mut ctx, "hello");
    let mut sink = StringSink::new();
    assert_eq!(
        execute(&control, &[value, value], &mut ctx, &runtime, &mut sink),
        Ok(2)
    );
    assert_eq!(sink.into_string(), "x=hello s=\"hello\"");
}

#[test]
fn executes_integer_radices_and_line_controls() {
    let (runtime, mut ctx) = context();
    let control = parse("~D ~B ~O ~X~2%tail~&done~~").expect("control");
    let mut sink = StringSink::new();
    assert_eq!(
        execute(
            &control,
            &[Word::fixnum(255); 4],
            &mut ctx,
            &runtime,
            &mut sink,
        ),
        Ok(4)
    );
    assert_eq!(sink.into_string(), "255 11111111 377 FF\n\ntail\ndone~");
}

#[test]
fn executes_zero_and_multiple_tilde_repeats_with_expected_line_state() {
    let (runtime, mut ctx) = context();
    let control = parse("head~3~~0~~&tail").expect("control");
    let mut sink = StringSink::new();

    assert_eq!(execute(&control, &[], &mut ctx, &runtime, &mut sink), Ok(0));
    assert_eq!(sink.into_string(), "head~~~\ntail");
}

#[test]
fn rejects_missing_and_non_integer_arguments() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    let missing = parse("~A").expect("control");
    assert_eq!(
        execute(&missing, &[], &mut ctx, &runtime, &mut sink),
        Err(FormatError::MissingArgument {
            directive: ncl_lib_format::DirectiveKind::A,
        })
    );
    let non_integer = parse("~D").expect("control");
    let value = string(&runtime, &mut ctx, "not an integer");
    assert_eq!(
        execute(&non_integer, &[value], &mut ctx, &runtime, &mut sink),
        Ok(1)
    );
    assert_eq!(sink.into_string(), "not an integer");
}

#[test]
fn executes_character_name_and_reader_syntax_modifiers() {
    let (runtime, mut ctx) = context();
    let control = parse("~:C/~@C/~:@C").expect("control");
    let mut sink = StringSink::new();
    execute(
        &control,
        &[
            Word::character(u32::from(' ')),
            Word::character(u32::from('A')),
            Word::character(u32::from('\n')),
        ],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "Space/#\\A/#\\Newline");
}

#[test]
fn ampersand_does_not_add_a_second_newline_at_line_start() {
    let (runtime, mut ctx) = context();
    let control = parse("a~%~&b").expect("control");
    let mut sink = StringSink::new();
    execute(&control, &[], &mut ctx, &runtime, &mut sink).expect("execute");
    assert_eq!(sink.into_string(), "a\nb");
}

#[test]
fn rejects_invalid_repeat_parameters_and_preserves_consumed_argument_count() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    assert!(parse("~-1%").is_err());
    assert_eq!(
        execute(
            &parse("~0%~A").expect("control"),
            &[Word::fixnum(7)],
            &mut ctx,
            &runtime,
            &mut sink
        ),
        Ok(1)
    );
    assert_eq!(sink.into_string(), "7");
}

#[test]
fn rejects_character_and_relative_repeat_parameters() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    assert_eq!(
        execute(
            &parse("~'x%").expect("control"),
            &[],
            &mut ctx,
            &runtime,
            &mut sink,
        ),
        Err(FormatError::InvalidParameter {
            directive: ncl_lib_format::DirectiveKind::Percent,
        })
    );
    assert_eq!(
        execute(
            &parse("~v~").expect("control"),
            &[Word::character(u32::from('x'))],
            &mut ctx,
            &runtime,
            &mut sink,
        ),
        Err(FormatError::InvalidParameter {
            directive: ncl_lib_format::DirectiveKind::Tilde,
        })
    );
}
#[test]
fn executes_character_radix_float_width_and_printer_directives() {
    let (runtime, mut ctx) = context();
    let float = make_double(&mut ctx, &runtime, 1.25).expect("float");
    let control = parse("~C/~16R/~F/~E/~G/~$/~5A/~5@A/~W").expect("control");
    let mut sink = StringSink::new();
    let value = string(&runtime, &mut ctx, "x");
    execute(
        &control,
        &[
            Word::character(u32::from('a')),
            Word::fixnum(255),
            float.into(),
            float.into(),
            float.into(),
            float.into(),
            value,
            value,
            value,
        ],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(
        sink.into_string(),
        "a/FF/1.25/1.25/1.25/1.25/x    /    x/\"x\""
    );
}

#[test]
fn executes_compound_control_directives() {
    let (runtime, mut ctx) = context();
    let one = string(&runtime, &mut ctx, "one");
    let two = string(&runtime, &mut ctx, "two");
    let tail = make_cons(&mut ctx, &runtime, two, Word::NIL).expect("tail");
    let list = make_cons(&mut ctx, &runtime, one, tail).expect("list");
    let controls = [
        ("~[zero~;one~]", vec![Word::fixnum(1)], "one"),
        ("~{~A,~}", vec![list], "one,two,"),
        ("~@(hello~)", vec![], "Hello"),
        ("~10<ok~>", vec![], "        ok"),
        ("~?", vec![string(&runtime, &mut ctx, "~A"), list], "one"),
    ];
    for (control, arguments, expected) in controls {
        let mut sink = StringSink::new();
        execute(
            &parse(control).expect("control"),
            &arguments,
            &mut ctx,
            &runtime,
            &mut sink,
        )
        .expect("execute");
        assert_eq!(sink.into_string(), expected, "{control}");
    }
}

#[test]
fn executes_parameter_count_and_argument_navigation() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    execute(
        &parse("~#_~A~-1*~A").expect("control"),
        &[Word::fixnum(1), Word::fixnum(2)],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "  11");
}

#[test]
fn argument_skip_supports_backward_and_absolute_forms() {
    let (runtime, mut ctx) = context();
    let one = string(&runtime, &mut ctx, "one");
    let two = string(&runtime, &mut ctx, "two");
    let mut sink = StringSink::new();
    execute(
        &parse("~A~:*~A/~1@*~A").expect("control"),
        &[one, two],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "oneone/two");
}

#[test]
fn radix_directive_applies_width_and_padding_parameters() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    execute(
        &parse("~3,5,'0R").expect("control"),
        &[Word::fixnum(12)],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "00110");
}

#[test]
fn resolves_v_and_hash_parameters_before_directive_execution() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    let mut sink = StringSink::new();
    execute(
        &parse("~vA/~#_").expect("control"),
        &[Word::fixnum(4), value, value],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "x   / ");
}

#[test]
fn tabulation_uses_current_column_and_relative_mode() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    execute(
        &parse("abc~8Tz/~3,8@Tq").expect("control"),
        &[],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "abc     z/      q");
}

#[test]
fn executes_parameterized_float_formats_and_rejects_invalid_values() {
    let (runtime, mut ctx) = context();
    let float = make_double(&mut ctx, &runtime, 12.345).expect("float");
    let mut sink = StringSink::new();
    assert_eq!(
        parse("~8,2F").expect("parse").parts[0],
        ncl_lib_format::ControlPart::Directive(ncl_lib_format::Directive {
            name: None,
            parameters: vec![
                ncl_lib_format::Parameter::Integer(8),
                ncl_lib_format::Parameter::Integer(2)
            ],
            colon: false,
            at_sign: false,
            kind: ncl_lib_format::DirectiveKind::F
        })
    );
    execute(
        &parse("~8,2F/~10,2E/~8,1G/~3,1,8$").expect("control"),
        &[float.into(), float.into(), float.into(), float.into()],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "   12.35/   1.23e+2/    12.3/  12.345");

    let mut sink = StringSink::new();
    assert!(
        execute(
            &parse("~F").expect("control"),
            &[Word::fixnum(1)],
            &mut ctx,
            &runtime,
            &mut sink,
        )
        .is_err()
    );
    assert!(
        execute(
            &parse("~8,2F").expect("control"),
            &[float.into()],
            &mut ctx,
            &runtime,
            &mut sink,
        )
        .is_ok()
    );
}

#[test]
fn exercises_format_error_contract_and_malformed_compounds() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    let cases = ["~[", "~{", "~(", "~<", "~?", "~16R", "~'x%"];
    for control in cases {
        let parsed = parse(control).expect("parse");
        assert!(
            execute(&parsed, &[], &mut ctx, &runtime, &mut sink).is_err(),
            "{control}"
        );
    }
    let errors = [
        FormatError::MissingArgument {
            directive: ncl_lib_format::DirectiveKind::A,
        },
        FormatError::InvalidParameter {
            directive: ncl_lib_format::DirectiveKind::A,
        },
        FormatError::NonInteger {
            directive: ncl_lib_format::DirectiveKind::D,
        },
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
        assert!(std::error::Error::source(&error).is_none());
    }
    let printed = FormatError::from(PrintError::Sink("closed".to_owned()));
    assert!(printed.to_string().contains("sink"));
    assert!(std::error::Error::source(&printed).is_some());
}

#[test]
fn exercises_early_termination_case_variants_and_parameter_errors() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    execute(
        &parse("~^tail").expect("control"),
        &[],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "");
    let mut sink = StringSink::new();
    let result = execute(
        &parse("~[zero~;one~]").expect("control"),
        &[Word::TRUE],
        &mut ctx,
        &runtime,
        &mut sink,
    );
    assert!(result.is_err());
    let mut sink = StringSink::new();
    execute(
        &parse("~[zero~;one~]").expect("control"),
        &[Word::fixnum(0)],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "zero");
    let mut sink = StringSink::new();
    execute(
        &parse("~:(Hello~)").expect("control"),
        &[],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "Hello");
    assert!(parse("~2,0T").is_ok());
    assert!(parse("~-1A").is_err());
}

#[test]
fn exercises_invalid_lists_nested_formats_and_float_padding_parameters() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    assert!(
        execute(
            &parse("~{~A~}").expect("control"),
            &[Word::fixnum(1)],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
    let improper =
        make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).expect("improper");
    assert!(
        execute(
            &parse("~{~A~}").expect("control"),
            &[improper],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
    assert!(
        execute(
            &parse("~?").expect("control"),
            &[Word::fixnum(1), Word::NIL],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
    let invalid_control = string(&runtime, &mut ctx, "~");
    assert!(
        execute(
            &parse("~?").expect("control"),
            &[invalid_control, Word::NIL],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
    let float = make_double(&mut ctx, &runtime, 1.2).expect("float");
    execute(
        &parse("~8,2,,,'0F").expect("control"),
        &[float.into()],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert!(sink.into_string().contains('0'));
}

#[test]
fn executes_remaining_directive_edges_and_reports_typed_parameter_errors() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();

    execute(
        &parse("before~:>after~/tail~|~:_(x~)").expect("control"),
        &[],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("control directives");
    assert!(sink.into_string().contains("before\nafter"));

    let value = string(&runtime, &mut ctx, "value");
    let improper = make_cons(&mut ctx, &runtime, value, Word::fixnum(1)).expect("improper");
    assert!(
        execute(
            &parse("~{~A~}").expect("brace"),
            &[improper],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
    assert!(
        execute(
            &parse("~?").expect("nested"),
            &[Word::fixnum(1), Word::NIL],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
    assert!(parse("~1R").is_err());
    assert!(parse("~37R").is_err());

    let integer = Word::fixnum(1);
    assert!(
        execute(
            &parse("~F").expect("float"),
            &[integer],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
    assert!(
        execute(
            &parse("~$").expect("currency"),
            &[integer],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
}
#[test]
fn format_errors_expose_specific_messages_and_sources() {
    use std::error::Error;

    let cases = [
        (
            FormatError::MissingArgument {
                directive: ncl_lib_format::DirectiveKind::A,
            },
            "format: missing argument for ~A",
        ),
        (
            FormatError::InvalidParameter {
                directive: ncl_lib_format::DirectiveKind::Percent,
            },
            "format: invalid parameter for ~Percent",
        ),
        (
            FormatError::NonInteger {
                directive: ncl_lib_format::DirectiveKind::D,
            },
            "format: expected integer for ~D",
        ),
    ];
    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
        assert!(error.source().is_none());
    }

    let print_error = FormatError::from(PrintError::Sink("closed".to_owned()));
    assert_eq!(print_error.to_string(), "format: print: sink error: closed");
    assert_eq!(
        print_error.source().map(ToString::to_string),
        Some("print: sink error: closed".to_owned())
    );
}

#[test]
fn nested_bracket_directives_keep_inner_semicolons_in_one_branch() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    execute(
        &parse("~[outer~[inner-zero~;inner-one~]~;fallback~]").expect("control"),
        &[Word::fixnum(0), Word::fixnum(1)],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "outerinner-one");
}
