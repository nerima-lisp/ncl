use crate::executor::{ExecutionState, Flow, FormatError, FormatOutput};
use crate::model::{Directive, DirectiveKind};

#[derive(Clone, Copy)]
pub enum ControlDirective {
    Star,
    Conditional,
    Iteration,
    Recursive,
    CaseConversion,
    UpArrow,
}

#[derive(Clone, Copy)]
pub enum NumericDirective {
    Radix(RadixDirective),
    Floating(FloatingDirective),
}

#[derive(Clone, Copy)]
pub enum RadixDirective {
    Decimal,
    Binary,
    Octal,
    Hexadecimal,
    Radix,
}

#[derive(Clone, Copy)]
pub enum FloatingDirective {
    Fixed,
    Exponential,
    General,
    Monetary,
}

#[derive(Clone, Copy)]
pub enum PrettyDirective {
    Underscore,
    Indent,
    Tab,
    LogicalBlock,
    Justification,
    Slash,
}

#[derive(Clone, Copy)]
pub enum TextDirective {
    Value(ValueDirective),
    Character,
    Plural,
    Line(LineDirective),
}

#[derive(Clone, Copy)]
pub enum ValueDirective {
    Aesthetic,
    Standard,
    Write,
}

#[derive(Clone, Copy)]
pub enum LineDirective {
    Percent,
    Ampersand,
    Bar,
    Tilde,
    Newline,
}

enum DirectiveRoute {
    Text(TextDirective),
    Numeric(NumericDirective),
    Control(ControlDirective),
    Pretty(PrettyDirective),
}

pub fn execute_directive(
    state: &mut ExecutionState<'_>,
    directive: &Directive,
    output: &mut FormatOutput<'_>,
) -> Result<Flow, FormatError> {
    match route_directive(directive.kind) {
        DirectiveRoute::Text(kind) => execute_text(kind, directive, state, output),
        DirectiveRoute::Numeric(kind) => execute_numeric(kind, directive, state, output),
        DirectiveRoute::Control(kind) => crate::control::execute(kind, directive, state, output),
        DirectiveRoute::Pretty(kind) => execute_pretty(kind, directive, state, output),
    }
}

const fn route_directive(kind: DirectiveKind) -> DirectiveRoute {
    match kind {
        DirectiveKind::A => DirectiveRoute::Text(TextDirective::Value(ValueDirective::Aesthetic)),
        DirectiveKind::S => DirectiveRoute::Text(TextDirective::Value(ValueDirective::Standard)),
        DirectiveKind::W => DirectiveRoute::Text(TextDirective::Value(ValueDirective::Write)),
        DirectiveKind::C => DirectiveRoute::Text(TextDirective::Character),
        DirectiveKind::Plural => DirectiveRoute::Text(TextDirective::Plural),
        DirectiveKind::Percent => DirectiveRoute::Text(TextDirective::Line(LineDirective::Percent)),
        DirectiveKind::Ampersand => {
            DirectiveRoute::Text(TextDirective::Line(LineDirective::Ampersand))
        }
        DirectiveKind::Bar => DirectiveRoute::Text(TextDirective::Line(LineDirective::Bar)),
        DirectiveKind::Tilde => DirectiveRoute::Text(TextDirective::Line(LineDirective::Tilde)),
        DirectiveKind::Newline => DirectiveRoute::Text(TextDirective::Line(LineDirective::Newline)),
        DirectiveKind::D => {
            DirectiveRoute::Numeric(NumericDirective::Radix(RadixDirective::Decimal))
        }
        DirectiveKind::B => {
            DirectiveRoute::Numeric(NumericDirective::Radix(RadixDirective::Binary))
        }
        DirectiveKind::O => DirectiveRoute::Numeric(NumericDirective::Radix(RadixDirective::Octal)),
        DirectiveKind::X => {
            DirectiveRoute::Numeric(NumericDirective::Radix(RadixDirective::Hexadecimal))
        }
        DirectiveKind::R => DirectiveRoute::Numeric(NumericDirective::Radix(RadixDirective::Radix)),
        DirectiveKind::F => {
            DirectiveRoute::Numeric(NumericDirective::Floating(FloatingDirective::Fixed))
        }
        DirectiveKind::E => {
            DirectiveRoute::Numeric(NumericDirective::Floating(FloatingDirective::Exponential))
        }
        DirectiveKind::G => {
            DirectiveRoute::Numeric(NumericDirective::Floating(FloatingDirective::General))
        }
        DirectiveKind::Dollar => {
            DirectiveRoute::Numeric(NumericDirective::Floating(FloatingDirective::Monetary))
        }
        DirectiveKind::Star => DirectiveRoute::Control(ControlDirective::Star),
        DirectiveKind::Conditional => DirectiveRoute::Control(ControlDirective::Conditional),
        DirectiveKind::Iteration => DirectiveRoute::Control(ControlDirective::Iteration),
        DirectiveKind::Recursive => DirectiveRoute::Control(ControlDirective::Recursive),
        DirectiveKind::CaseConversion => DirectiveRoute::Control(ControlDirective::CaseConversion),
        DirectiveKind::UpArrow => DirectiveRoute::Control(ControlDirective::UpArrow),
        DirectiveKind::Underscore => DirectiveRoute::Pretty(PrettyDirective::Underscore),
        DirectiveKind::LogicalBlock => DirectiveRoute::Pretty(PrettyDirective::LogicalBlock),
        DirectiveKind::Indent => DirectiveRoute::Pretty(PrettyDirective::Indent),
        DirectiveKind::Slash => DirectiveRoute::Pretty(PrettyDirective::Slash),
        DirectiveKind::Tab => DirectiveRoute::Pretty(PrettyDirective::Tab),
        DirectiveKind::Justification => DirectiveRoute::Pretty(PrettyDirective::Justification),
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
