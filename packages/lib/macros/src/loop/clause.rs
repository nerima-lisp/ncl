use super::{
    AccumulatorKind, ConditionalKind, HashIterationKind, LimitDirection, PackageIterationKind,
    StepDirection,
};

#[derive(Clone, Debug)]
pub(super) enum HeldLoopClause {
    With {
        variable: usize,
        type_spec: Option<usize>,
        has_init: bool,
        init: usize,
    },
    For {
        variable: usize,
        type_spec: Option<usize>,
        has_init: bool,
        init: usize,
        step: Option<usize>,
        direction: Option<StepDirection>,
        limit: Option<(LimitDirection, usize)>,
    },
    Hash {
        variable: usize,
        type_spec: Option<usize>,
        kind: HashIterationKind,
        table: usize,
        using: Option<(HashIterationKind, usize)>,
    },
    Package {
        variable: usize,
        type_spec: Option<usize>,
        kind: PackageIterationKind,
        package: Option<usize>,
    },
    EqualsThen {
        variable: usize,
        type_spec: Option<usize>,
        init: usize,
        then: Option<usize>,
    },
    In {
        variable: usize,
        type_spec: Option<usize>,
        sequence: usize,
        on: bool,
        by: Option<usize>,
    },
    Across {
        variable: usize,
        type_spec: Option<usize>,
        vector: usize,
    },
    Repeat(usize),
    While(usize),
    Until(usize),
    Initially(Vec<usize>),
    Finally(Vec<usize>),
    Do(Vec<usize>),
    Accumulate {
        kind: AccumulatorKind,
        form: usize,
        variable: Option<usize>,
    },
    Always(usize),
    Never(usize),
    Thereis(usize),
    Return(usize),
    Conditional {
        kind: ConditionalKind,
        test: usize,
        then: Vec<Self>,
        otherwise: Vec<Self>,
    },
}
