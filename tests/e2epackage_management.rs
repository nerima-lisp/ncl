#![allow(missing_docs)]

use std::process::Command;

fn eval(source: &str) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .output()
        .unwrap_or_else(|error| panic!("failed to run {source}: {error}"));
    let output_text = if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        String::from_utf8_lossy(&output.stderr)
    };
    (output.status.success(), output_text.trim().to_owned())
}

#[test]
fn rename_package_normalizes_symbol_designators_and_preserves_nickname_order() {
    let source = r#"(let* ((package (make-package "OO35-RENAME-OLD" :use nil))
                   (renamed (rename-package package 'OO35-RENAME-NEW
                                            '(OO35-RENAME-ALIAS "OO35-RENAME-ALIAS-2"))))
              (and (eq renamed package)
                   (equal (package-name package) "OO35-RENAME-NEW")
                   (equal (package-nicknames package)
                          '("OO35-RENAME-ALIAS" "OO35-RENAME-ALIAS-2"))
                   (null (find-package "OO35-RENAME-OLD"))
                   (eq (find-package 'OO35-RENAME-NEW) package)
                   (eq (find-package 'OO35-RENAME-ALIAS) package)
                   (eq (find-package "OO35-RENAME-ALIAS-2") package)))"#;
    let (success, actual) = eval(source);
    assert!(success, "{source} failed: {actual}");
    assert_eq!(actual, "T", "{source}");
}

#[test]
fn rename_package_omitted_and_nil_nicknames_clear_previous_nicknames() {
    let source = r#"(let* ((omitted (make-package "OO35-OMITTED-OLD"
                                                  :nicknames '("OO35-OMITTED-ALIAS")
                                                  :use nil))
                   (omitted-result (rename-package omitted "OO35-OMITTED-NEW"))
                   (explicit-nil (make-package "OO35-NIL-OLD"
                                                :nicknames '("OO35-NIL-ALIAS")
                                                :use nil))
                   (nil-result (rename-package explicit-nil "OO35-NIL-NEW" nil)))
              (and (eq omitted-result omitted)
                   (eq nil-result explicit-nil)
                   (null (package-nicknames omitted))
                   (null (package-nicknames explicit-nil))
                   (null (find-package "OO35-OMITTED-OLD"))
                   (null (find-package "OO35-OMITTED-ALIAS"))
                   (null (find-package "OO35-NIL-OLD"))
                   (null (find-package "OO35-NIL-ALIAS"))
                   (eq (find-package "OO35-OMITTED-NEW") omitted)
                   (eq (find-package "OO35-NIL-NEW") explicit-nil)))"#;
    let (success, actual) = eval(source);
    assert!(success, "{source} failed: {actual}");
    assert_eq!(actual, "T", "{source}");
}

#[test]
fn delete_package_clears_the_retained_objects_name_and_unregisters_nicknames() {
    let source = r#"(let* ((package (make-package "OO35-DELETE"
                                                  :nicknames '("OO35-DELETE-ALIAS")
                                                  :use nil))
                   (deleted (delete-package package)))
              (and deleted
                   (packagep package)
                   (null (package-name package))
                   (equal (package-nicknames package) '("OO35-DELETE-ALIAS"))
                   (null (find-package "OO35-DELETE"))
                   (null (find-package "OO35-DELETE-ALIAS"))
                   (null (delete-package package))))"#;
    let (success, actual) = eval(source);
    assert!(success, "{source} failed: {actual}");
    assert_eq!(actual, "T", "{source}");
}

#[test]
fn package_management_rejects_invalid_designators_and_nicknames() {
    for source in [
        "(delete-package 1)",
        "(rename-package 1 \"OO35-INVALID-NEW\")",
        "(rename-package (make-package \"OO35-MISSING-NAME\" :use nil))",
        "(rename-package (make-package \"OO35-INVALID-NAME\" :use nil) 1)",
        "(rename-package (make-package \"OO35-INVALID-LIST\" :use nil) \"OO35-NEW\" 1)",
        "(rename-package (make-package \"OO35-INVALID-NICKNAME\" :use nil) \"OO35-NEW\" '(1))",
    ] {
        let (success, actual) = eval(source);
        assert!(
            !success && actual.contains("TypeError"),
            "{source} did not reject the invalid argument (success={success}, output={actual})"
        );
    }
}
