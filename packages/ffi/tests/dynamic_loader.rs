#![allow(
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests assert on dynamic-loader boundary errors"
)]

//! Dynamic loader and foreign-symbol wrapper behavior.

use ncl_ffi::{
    FfiError, ForeignSymbolName, LoaderMode, SharedObjectPath, SysPrimitive, dlerror_message,
    dlopen_or_lose, extern_alien_name, find_foreign_symbol_address, foreign_symbol_address,
    foreign_symbol_dataref_sap, foreign_symbol_sap, load_shared_object,
};

fn missing<T>(result: Result<T, FfiError>, expected: SysPrimitive) {
    match result {
        Err(error) => assert_eq!(error, FfiError::MissingSysPrimitive(expected)),
        Ok(_) => panic!("expected missing primitive error"),
    }
}

#[test]
fn loader_and_symbol_wrappers_report_required_primitives() {
    missing(
        load_shared_object(SharedObjectPath::new("libexample.so"), LoaderMode::Lazy),
        SysPrimitive::DlopenSharedObject,
    );
    missing(
        load_shared_object("libexample.so", false),
        SysPrimitive::DlopenSharedObject,
    );
    missing(
        dlopen_or_lose(SharedObjectPath::from("libexample.so")),
        SysPrimitive::DlopenSharedObject,
    );
    missing(dlerror_message(), SysPrimitive::DlerrorMessage);
    missing(
        find_foreign_symbol_address(ForeignSymbolName::from("entry")),
        SysPrimitive::DlsymForeignSymbol,
    );
    missing(
        foreign_symbol_address("entry"),
        SysPrimitive::DlsymForeignSymbol,
    );
    missing(
        foreign_symbol_sap("entry"),
        SysPrimitive::DlsymForeignSymbol,
    );
    missing(
        foreign_symbol_dataref_sap("entry"),
        SysPrimitive::DlsymForeignSymbol,
    );
}

#[test]
fn loader_value_objects_preserve_owned_names() {
    assert_eq!(LoaderMode::from(true), LoaderMode::Lazy);
    assert_eq!(LoaderMode::from(false), LoaderMode::Now);

    let path = SharedObjectPath::new(String::from("libexample.so"));
    assert_eq!(path.as_str(), "libexample.so");
    assert_eq!(SharedObjectPath::from("libc.so").as_str(), "libc.so");

    let name = ForeignSymbolName::new(String::from("entry"));
    assert_eq!(name.as_str(), "entry");
    assert_eq!(ForeignSymbolName::from("main").as_str(), "main");
    assert_eq!(extern_alien_name("package::name"), "package::name");
}
