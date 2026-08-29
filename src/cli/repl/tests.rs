use reedline::PromptHistorySearchStatus;

use super::{
    FormValidator, NclPrompt, Prompt, PromptEditMode, PromptHistorySearch, ValidationResult,
    Validator, is_incomplete,
};

#[test]
fn is_incomplete_distinguishes_unclosed_forms_from_genuine_syntax_errors() {
    assert!(is_incomplete("(+ 1 2"));
    assert!(is_incomplete("\"unterminated"));
    assert!(!is_incomplete(")"));
    assert!(!is_incomplete("(+ 1 2)"));
    assert!(!is_incomplete(""));
}

#[test]
fn form_validator_defers_submission_on_an_unclosed_form() {
    assert!(matches!(
        FormValidator.validate("(+ 1 2"),
        ValidationResult::Incomplete
    ));
}

#[test]
fn form_validator_completes_on_a_balanced_form() {
    assert!(matches!(
        FormValidator.validate("(+ 1 2)"),
        ValidationResult::Complete
    ));
}

#[test]
fn ncl_prompt_shows_its_markers_unless_quiet() {
    let verbose = NclPrompt { quiet: false };
    assert_eq!(verbose.render_prompt_left(), "ncl> ");
    assert_eq!(verbose.render_prompt_multiline_indicator(), "...  ");

    let quiet = NclPrompt { quiet: true };
    assert_eq!(quiet.render_prompt_left(), "");
    assert_eq!(quiet.render_prompt_multiline_indicator(), "");
}

#[test]
fn ncl_prompt_has_no_right_prompt_or_mode_indicator() {
    let prompt = NclPrompt { quiet: false };
    assert_eq!(prompt.render_prompt_right(), "");
    assert_eq!(prompt.render_prompt_indicator(PromptEditMode::Emacs), "");
}

#[test]
fn ncl_prompt_history_search_indicator_shows_the_search_term() {
    let prompt = NclPrompt { quiet: false };
    let search = PromptHistorySearch::new(PromptHistorySearchStatus::Passing, "foo".to_string());
    assert_eq!(
        prompt.render_prompt_history_search_indicator(search),
        "(reverse-search: foo) "
    );
}
