#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ncl_runtime::{Runtime, RuntimeError, Value};
use ncl_syntax::Form;
use std::hint::black_box;

type SourceEvaluator = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;
type FormEvaluator = fn(&Runtime, &Form) -> Result<Value, RuntimeError>;

const ARITHMETIC_SOURCE: &str = "(let ((sum 0)) (dotimes (i 1000 sum) (incf sum i)))";
const FUNCTION_SOURCE: &str = "(progn (defun square (value) (* value value)) (square 1234))";

fn benchmark_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("evaluation");

    for (name, source) in [
        ("arithmetic", ARITHMETIC_SOURCE),
        ("function", FUNCTION_SOURCE),
    ] {
        let forms = match ncl_syntax::read(source) {
            Ok(forms) => forms,
            Err(error) => panic!("benchmark source must parse: {error}"),
        };
        assert_eq!(forms.len(), 1, "benchmark source must contain one form");

        benchmark_mode(
            &mut group,
            "interpreted",
            name,
            source,
            Runtime::eval_source,
        );
        benchmark_mode(
            &mut group,
            "compiled",
            name,
            source,
            Runtime::eval_compiled_source,
        );
        benchmark_form_mode(
            &mut group,
            "interpreted-form",
            name,
            &forms[0],
            Runtime::eval,
        );
        benchmark_form_mode(
            &mut group,
            "compiled-form",
            name,
            &forms[0],
            Runtime::eval_compiled,
        );
    }

    group.finish();
}

fn benchmark_mode(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    mode: &str,
    name: &str,
    source: &'static str,
    evaluate: SourceEvaluator,
) {
    group.bench_with_input(BenchmarkId::new(mode, name), source, |b, source| {
        b.iter_batched(
            Runtime::new,
            |runtime| {
                let result = evaluate(&runtime, black_box(source));
                assert!(result.is_ok(), "benchmark source failed: {result:?}");
                let _ = black_box(result);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn benchmark_form_mode(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    mode: &str,
    name: &str,
    form: &Form,
    evaluate: FormEvaluator,
) {
    group.bench_with_input(BenchmarkId::new(mode, name), form, |b, form| {
        b.iter_batched(
            Runtime::new,
            |runtime| {
                let result = evaluate(&runtime, black_box(form));
                assert!(result.is_ok(), "benchmark form failed: {result:?}");
                let _ = black_box(result);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, benchmark_evaluation);
criterion_main!(benches);
