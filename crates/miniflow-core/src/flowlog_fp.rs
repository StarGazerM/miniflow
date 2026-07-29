//! FlowLog-compatible names for logical-plan transformations.
//!
//! `FlowLog` names generated intermediates with `DefaultHasher` fingerprints of
//! its transformation-flow algebra. `MiniFlow` reproduces only the hash shape
//! needed by the strict overlap fixtures, including literal/type/comparison
//! discriminants. These private compatibility types name generated operators;
//! they are not a parser, evaluator, or public expression language.

use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

#[derive(Clone, Copy, Hash)]
pub(crate) enum TransformationArgument {
    KV((bool, usize)),
    Jn((bool, bool, usize)),
}

#[allow(dead_code)]
#[derive(Clone, Hash)]
pub(crate) enum FactorArgument {
    Var(TransformationArgument),
    Const(Constant),
    FnCall {
        name: String,
        args: Vec<ArithmeticArgument>,
    },
    Builtin {
        op: BuiltinOperator,
        args: Vec<ArithmeticArgument>,
    },
    Group(Box<ArithmeticArgument>),
    Tuple {
        fields: Vec<ArithmeticArgument>,
    },
    TupleProj {
        tuple: Box<ArithmeticArgument>,
        index: usize,
    },
}

#[derive(Clone, Hash)]
pub(crate) struct ArithmeticArgument {
    pub(crate) init: FactorArgument,
    pub(crate) rest: Vec<(ArithmeticOperator, FactorArgument)>,
}

#[derive(Clone, Hash)]
pub(crate) enum ArithmeticOperator {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Hash)]
pub(crate) enum BuiltinOperator {
    Strlen,
    Substr,
    Ord,
    ToString,
    ToNumber,
    Cat,
}

#[allow(dead_code)]
#[derive(Clone, Hash)]
pub(crate) enum DataType {
    IntLit,
    FloatLit,
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Float32,
    Float64,
    String,
    Bool,
    FixedTuple(Vec<DataType>),
}

#[derive(Clone, Hash)]
pub(crate) struct Constant {
    pub(crate) text: String,
    pub(crate) ty: DataType,
}

#[allow(dead_code)]
#[derive(Clone, Hash)]
pub(crate) enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterEqualThan,
    LessThan,
    LessEqualThan,
    Match { negated: bool },
    Contains { negated: bool },
}

#[derive(Hash)]
pub(crate) struct ComparisonExprArgument {
    pub(crate) left: ArithmeticArgument,
    pub(crate) operator: ComparisonOperator,
    pub(crate) right: ArithmeticArgument,
}

#[derive(Hash)]
struct Constraints {
    constant_equalities: Arc<Vec<(TransformationArgument, Constant)>>,
    variable_equalities: Arc<Vec<(TransformationArgument, TransformationArgument)>>,
}

#[derive(Hash)]
enum TransformationFlow {
    Unary {
        key: Arc<Vec<ArithmeticArgument>>,
        value: Arc<Vec<ArithmeticArgument>>,
        constraints: Constraints,
        comparisons: Vec<ComparisonExprArgument>,
    },
    Join {
        key: Arc<Vec<ArithmeticArgument>>,
        value: Arc<Vec<ArithmeticArgument>>,
        comparisons: Vec<ComparisonExprArgument>,
    },
}

pub(crate) fn relation(name: &str) -> u64 {
    compute(name)
}

pub(crate) fn unary(
    tag: &'static str,
    input: u64,
    key: impl IntoIterator<Item = TransformationArgument>,
    value: impl IntoIterator<Item = TransformationArgument>,
) -> u64 {
    let flow = TransformationFlow::Unary {
        key: Arc::new(arguments(key)),
        value: Arc::new(arguments(value)),
        constraints: Constraints {
            constant_equalities: Arc::new(Vec::new()),
            variable_equalities: Arc::new(Vec::new()),
        },
        comparisons: Vec::new(),
    };
    compute((tag, input, &flow))
}

pub(crate) fn unary_expressions(
    tag: &'static str,
    input: u64,
    key: Vec<ArithmeticArgument>,
    value: Vec<ArithmeticArgument>,
    constant_equalities: Vec<(TransformationArgument, Constant)>,
    variable_equalities: Vec<(TransformationArgument, TransformationArgument)>,
    comparisons: Vec<ComparisonExprArgument>,
) -> u64 {
    let flow = TransformationFlow::Unary {
        key: Arc::new(key),
        value: Arc::new(value),
        constraints: Constraints {
            constant_equalities: Arc::new(constant_equalities),
            variable_equalities: Arc::new(variable_equalities),
        },
        comparisons,
    };
    compute((tag, input, &flow))
}

pub(crate) fn join(
    tag: &'static str,
    left: u64,
    right: u64,
    key: impl IntoIterator<Item = TransformationArgument>,
    value: impl IntoIterator<Item = TransformationArgument>,
) -> u64 {
    let flow = TransformationFlow::Join {
        key: Arc::new(arguments(key)),
        value: Arc::new(arguments(value)),
        comparisons: Vec::new(),
    };
    compute((tag, left, right, &flow))
}

pub(crate) fn join_expressions(
    tag: &'static str,
    left: u64,
    right: u64,
    key: Vec<ArithmeticArgument>,
    value: Vec<ArithmeticArgument>,
    comparisons: Vec<ComparisonExprArgument>,
) -> u64 {
    let flow = TransformationFlow::Join {
        key: Arc::new(key),
        value: Arc::new(value),
        comparisons,
    };
    compute((tag, left, right, &flow))
}

fn arguments(
    arguments: impl IntoIterator<Item = TransformationArgument>,
) -> Vec<ArithmeticArgument> {
    arguments
        .into_iter()
        .map(|argument| ArithmeticArgument {
            init: FactorArgument::Var(argument),
            rest: Vec::new(),
        })
        .collect()
}

fn compute<T: Hash>(value: T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
