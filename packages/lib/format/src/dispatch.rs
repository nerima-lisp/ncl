use crate::executor::{ExecutionState, Flow, FormatError, FormatOutput};
use crate::model::{Directive, DirectiveKind};

#[derive(Clone, Copy)]
pub(crate) enum ControlDirective {
    Star,
    Conditional,
    Iteration,
    Recursive,
    CaseConversion,
    UpArrow,
}

#[derive(Clone, Copy)]
pub(crate) enum NumericDirective {
    Radix(RadixDirective),
    Floating(FloatingDirective),
}

#[derive(Clone, Copy)]
pub(crate) enum RadixDirective {
    Decimal,
    Binary,
    Octal,
    Hexadecimal,
    Radix,
}

#[derive(Clone, Copy)]
pub(crate) enum FloatingDirective {
    Fixed,
    Exponential,
    General,
    Monetary,
}

#[derive(Clone, Copy)]
pub(crate) enum PrettyDirective {
    Underscore,
    Indent,
    Tab,
    LogicalBlock,
    Justification,
    Slash,
}

#[derive(Clone, Copy)]
pub(crate) enum TextDirective {
    Value(ValueDirective),
    Character,
    Plural,
    Line(LineDirective),
}

#[derive(Clone, Copy)]
pub(crate) enum ValueDirective {
    Aesthetic,
    Standard,
    Write,
}

#[derive(Clone, Copy)]
pub(crate) enum LineDirective {
    Percent,
    Ampersand,
    Bar,
    Tilde,
    Newline,
}

pub(crate) fn execute_directive(
    state: &mut ExecutionState<'_>,
    directive: &Directive,
    output: &mut FormatOutput<'_>,
) -> Result<Flow, FormatError> {
    match directive.kind {
        DirectiveKind::A => execute_text(
            TextDirective::Value(ValueDirective::Aesthetic),
            directive,
            state,
            output,
        ),
        DirectiveKind::S => execute_text(
            TextDirective::Value(ValueDirective::Standard),
            directive,
            state,
            output,
        ),
        DirectiveKind::W => execute_text(
            TextDirective::Value(ValueDirective::Write),
            directive,
            state,
            output,
        ),
        DirectiveKind::C => execute_text(TextDirective::Character, directive, state, output),
        DirectiveKind::Plural => execute_text(TextDirective::Plural, directive, state, output),
        DirectiveKind::Percent => execute_text(
            TextDirective::Line(LineDirective::Percent),
            directive,
            state,
            output,
        ),
        DirectiveKind::Ampersand => execute_text(
            TextDirective::Line(LineDirective::Ampersand),
            directive,
            state,
            output,
        ),
        DirectiveKind::Bar => execute_text(
            TextDirective::Line(LineDirective::Bar),
            directive,
            state,
            output,
        ),
        DirectiveKind::Tilde => execute_text(
            TextDirective::Line(LineDirective::Tilde),
            directive,
            state,
            output,
        ),
        DirectiveKind::Newline => execute_text(
            TextDirective::Line(LineDirective::Newline),
            directive,
            state,
            output,
        ),
        DirectiveKind::D => execute_numeric(
            NumericDirective::Radix(RadixDirective::Decimal),
            directive,
            state,
            output,
        ),
        DirectiveKind::B => execute_numeric(
            NumericDirective::Radix(RadixDirective::Binary),
            directive,
            state,
            output,
        ),
        DirectiveKind::O => execute_numeric(
            NumericDirective::Radix(RadixDirective::Octal),
            directive,
            state,
            output,
        ),
        DirectiveKind::X => execute_numeric(
            NumericDirective::Radix(RadixDirective::Hexadecimal),
            directive,
            state,
            output,
        ),
        DirectiveKind::R => execute_numeric(
            NumericDirective::Radix(RadixDirective::Radix),
            directive,
            state,
            output,
        ),
        DirectiveKind::F => execute_numeric(
            NumericDirective::Floating(FloatingDirective::Fixed),
            directive,
            state,
            output,
        ),
        DirectiveKind::E => execute_numeric(
            NumericDirective::Floating(FloatingDirective::Exponential),
            directive,
            state,
            output,
        ),
        DirectiveKind::G => execute_numeric(
            NumericDirective::Floating(FloatingDirective::General),
            directive,
            state,
            output,
        ),
        DirectiveKind::Dollar => execute_numeric(
            NumericDirective::Floating(FloatingDirective::Monetary),
            directive,
            state,
            output,
        ),
        DirectiveKind::Star => {
            crate::control::execute(ControlDirective::Star, directive, state, output)
        }
        DirectiveKind::Conditional => {
            crate::control::execute(ControlDirective::Conditional, directive, state, output)
        }
        DirectiveKind::Iteration => {
            crate::control::execute(ControlDirective::Iteration, directive, state, output)
        }
        DirectiveKind::Recursive => {
            crate::control::execute(ControlDirective::Recursive, directive, state, output)
        }
        DirectiveKind::CaseConversion => {
            crate::control::execute(ControlDirective::CaseConversion, directive, state, output)
        }
        DirectiveKind::UpArrow => {
            crate::control::execute(ControlDirective::UpArrow, directive, state, output)
        }
        DirectiveKind::Underscore => {
            execute_pretty(PrettyDirective::Underscore, directive, state, output)
        }
        DirectiveKind::LogicalBlock => {
            execute_pretty(PrettyDirective::LogicalBlock, directive, state, output)
        }
        DirectiveKind::Indent => execute_pretty(PrettyDirective::Indent, directive, state, output),
        DirectiveKind::Slash => execute_pretty(PrettyDirective::Slash, directive, state, output),
        DirectiveKind::Tab => execute_pretty(PrettyDirective::Tab, directive, state, output),
        DirectiveKind::Justification => {
            execute_pretty(PrettyDirective::Justification, directive, state, output)
        }
    }
}

fn execute_text(
    kind: TextDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<Flow, FormatError> {
    crate::text::execute(kind, directive, state, output).map(|()| Flow::Continue)
}

fn execute_numeric(
    kind: NumericDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<Flow, FormatError> {
    crate::numeric::execute(kind, directive, state, output).map(|()| Flow::Continue)
}

fn execute_pretty(
    kind: PrettyDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<Flow, FormatError> {
    crate::pretty::execute(kind, directive, state, output).map(|()| Flow::Continue)
}
