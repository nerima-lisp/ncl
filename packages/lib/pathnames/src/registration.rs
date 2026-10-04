use super::operations::{
    compile_file_pathname_builtin, delete_file_builtin, directory_builtin,
    enough_namestring_builtin, ensure_directories_exist_builtin, file_author_builtin,
    file_error_pathname_builtin, file_length_builtin, file_write_date_builtin,
    host_namestring_builtin, load_logical_pathname_translations_builtin, logical_pathname_builtin,
    logical_pathname_translations_builtin, merge_pathnames_builtin, parse_namestring_builtin,
    pathname_match_builtin, probe_file_builtin, rename_file_builtin,
    translate_logical_pathname_builtin, translate_pathname_builtin, truename_builtin,
    user_homedir_pathname_builtin, wild_pathname_p_builtin,
};
use super::{
    LambdaList, OBJECT, ObjectError, PATHNAME, directory_namestring_builtin,
    file_namestring_builtin, namestring_builtin,
};

pub fn register_operations(
    direct: &mut impl FnMut(
        &'static str,
        ncl_object::RustBuiltin,
        LambdaList,
    ) -> Result<ncl_object::FunctionObject, ObjectError>,
) -> Result<(), ObjectError> {
    for (name, function, lambda_list) in [
        (
            "NAMESTRING",
            namestring_builtin as ncl_object::RustBuiltin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "PARSE-NAMESTRING",
            parse_namestring_builtin,
            LambdaList::with_rest(&[PATHNAME], OBJECT),
        ),
        (
            "FILE-NAMESTRING",
            file_namestring_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "DIRECTORY-NAMESTRING",
            directory_namestring_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "DIRECTORY",
            directory_builtin,
            LambdaList::with_rest(&[PATHNAME], OBJECT),
        ),
        (
            "TRANSLATE-PATHNAME",
            translate_pathname_builtin,
            LambdaList::with_rest(&[PATHNAME, PATHNAME, PATHNAME], OBJECT),
        ),
        (
            "LOGICAL-PATHNAME",
            logical_pathname_builtin,
            LambdaList::with_rest(&[PATHNAME], OBJECT),
        ),
        (
            "LOGICAL-PATHNAME-TRANSLATIONS",
            logical_pathname_translations_builtin,
            LambdaList::with_rest(&[PATHNAME], OBJECT),
        ),
        (
            "LOAD-LOGICAL-PATHNAME-TRANSLATIONS",
            load_logical_pathname_translations_builtin,
            LambdaList::with_rest(&[PATHNAME], OBJECT),
        ),
        (
            "TRANSLATE-LOGICAL-PATHNAME",
            translate_logical_pathname_builtin,
            LambdaList::with_rest(&[PATHNAME], OBJECT),
        ),
        (
            "COMPILE-FILE-PATHNAME",
            compile_file_pathname_builtin,
            LambdaList::with_rest(&[PATHNAME], OBJECT),
        ),
        (
            "WILD-PATHNAME-P",
            wild_pathname_p_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "PATHNAME-MATCH-P",
            pathname_match_builtin,
            LambdaList::fixed(&[PATHNAME, PATHNAME]),
        ),
        (
            "MERGE-PATHNAMES",
            merge_pathnames_builtin,
            LambdaList::with_optional(&[PATHNAME], &[PATHNAME]),
        ),
    ] {
        direct(name, function, lambda_list)?;
    }
    register_file_operations(direct)
}

fn register_file_operations(
    direct: &mut impl FnMut(
        &'static str,
        ncl_object::RustBuiltin,
        LambdaList,
    ) -> Result<ncl_object::FunctionObject, ObjectError>,
) -> Result<(), ObjectError> {
    for (name, function, lambda_list) in [
        (
            "PROBE-FILE",
            probe_file_builtin as ncl_object::RustBuiltin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        ("TRUENAME", truename_builtin, LambdaList::fixed(&[PATHNAME])),
        (
            "FILE-LENGTH",
            file_length_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "DELETE-FILE",
            delete_file_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "ENSURE-DIRECTORIES-EXIST",
            ensure_directories_exist_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "RENAME-FILE",
            rename_file_builtin,
            LambdaList::fixed(&[PATHNAME, PATHNAME]),
        ),
        (
            "FILE-WRITE-DATE",
            file_write_date_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "FILE-AUTHOR",
            file_author_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "FILE-ERROR-PATHNAME",
            file_error_pathname_builtin,
            LambdaList::fixed(&[OBJECT]),
        ),
        (
            "HOST-NAMESTRING",
            host_namestring_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "ENOUGH-NAMESTRING",
            enough_namestring_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "USER-HOMEDIR-PATHNAME",
            user_homedir_pathname_builtin,
            LambdaList::with_rest(&[], OBJECT),
        ),
    ] {
        direct(name, function, lambda_list)?;
    }
    Ok(())
}
