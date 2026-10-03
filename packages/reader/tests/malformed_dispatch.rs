#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "tests assert on reader behavior"
)]

use ncl_object::{ObjectRef, Runtime, ThreadContext, classify};
use ncl_reader::{ReadError, ReadOptions, StringSource, read};

#[test]
fn malformed_dispatch_reports_the_error_and_allows_following_form() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = ReadOptions::standard(&mut ctx, &runtime).unwrap();
    let mut source = StringSource::new("#2r2 7");

    assert_eq!(
        read(&mut ctx, &runtime, &mut source, &opts).unwrap_err(),
        ReadError::InvalidNumber(String::new())
    );
    let next = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(next), ObjectRef::Fixnum(2));
    let following = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(following), ObjectRef::Fixnum(7));
    assert_eq!(read(&mut ctx, &runtime, &mut source, &opts).unwrap(), None);
}
