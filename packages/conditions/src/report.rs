//! Rendering a condition's report text (C5): `PRINC`/`FORMAT`'s `~A` of a
//! condition, and the default message an unhandled `ERROR`/`WARN`/`CERROR`
//! prints.

use ncl_object::{Instance, ThreadContext, Word, car, cdr, simple_vector_length, slot_ref};

use crate::class::class_named;
use crate::register::{string_text, symbol_text};

/// Best-effort `PRINC`-style rendering of a single value for report text.
///
/// This is a small, self-contained fallback (fixnum, string, symbol), not a
/// general printer; unrecognized values fall back to `#<OBJECT>`. The real
/// printer's dispatch (`ncl-printer`) handles the general case.
fn describe_word(ctx: &ThreadContext, value: Word) -> String {
    if let Some(number) = value.as_fixnum() {
        return number.to_string();
    }
    if let Some(text) = string_text(ctx, value) {
        return text;
    }
    if let Ok(text) = symbol_text(ctx, value) {
        return text;
    }
    "#<OBJECT>".to_owned()
}

/// Expand a `SIMPLE-CONDITION`-shaped `(format-control . format-arguments)`
/// report, honoring the `~a`/`~A` directive.
fn format_simple_report(ctx: &ThreadContext, control: &str, mut arguments: Word) -> String {
    let mut report = String::new();
    let mut chars = control.chars();
    while let Some(character) = chars.next() {
        if character == '~' {
            if let Some(directive) = chars.next() {
                if matches!(directive, 'a' | 'A') {
                    if let Ok(argument) = car(ctx, arguments) {
                        arguments = cdr(ctx, arguments).unwrap_or(Word::NIL);
                        report.push_str(&describe_word(ctx, argument));
                    } else {
                        report.push('~');
                        report.push(directive);
                    }
                } else {
                    report.push('~');
                    report.push(directive);
                }
            } else {
                report.push('~');
            }
        } else {
            report.push(character);
        }
    }
    report
}

/// Render a condition's report text.
///
/// Dispatches on its most specific known class: a `DEFINE-CONDITION`-supplied
/// `:report` string, a `SIMPLE-CONDITION`'s format-control/arguments, a
/// `TYPE-ERROR`'s datum/expected-type, or a generic `<class-name> condition`
/// fallback.
///
/// # Errors
/// This never fails outright; `None` means no report could be produced at
/// all (a malformed instance).
#[must_use]
pub fn condition_report(ctx: &ThreadContext, condition: Word) -> Option<String> {
    let instance = Instance::from_word(condition);
    let class = crate::condition_class_of(ctx, condition).ok()?.as_word();
    if simple_vector_length(ctx, class).unwrap_or(0) > crate::slots::REPORT_SLOT
        && let Ok(report) = ncl_object::simple_vector_ref(ctx, class, crate::slots::REPORT_SLOT)
        && let Some(text) = string_text(ctx, report)
    {
        return Some(text);
    }
    if class_named(ctx, class, "SIMPLE-CONDITION").unwrap_or(false) {
        let control = string_text(ctx, slot_ref(ctx, instance, 0).ok()?)?;
        let arguments = slot_ref(ctx, instance, 1).unwrap_or(Word::NIL);
        return Some(format_simple_report(ctx, &control, arguments));
    }
    if class_named(ctx, class, "TYPE-ERROR").unwrap_or(false) {
        let datum = slot_ref(ctx, instance, 0).ok()?;
        let expected = slot_ref(ctx, instance, 1).ok()?;
        return Some(format!(
            "The value {} is not of type {}.",
            describe_word(ctx, datum),
            describe_word(ctx, expected)
        ));
    }
    let name = crate::condition_class_name(ctx, crate::ConditionClass::from_word(class)).ok()?;
    Some(format!("{} condition", string_text(ctx, name)?))
}
