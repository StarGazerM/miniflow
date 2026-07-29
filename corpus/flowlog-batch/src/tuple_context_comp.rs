crate::fixture_program! {
    pub struct TupleContextComp;
    .decl call(c0: String, c1: String, c2: String)
    .decl out(c0: (String, String), c1: String)
    .decl same_ctx(c0: String, c1: String)
    .decl a__reaches(c0: (String, String), c1: String)
    .decl a__samectx(c0: String, c1: String)

    a__reaches((caller, site), callee) :- call(caller, callee, site).
    a__samectx(a, b) :- a__reaches(context, a), a__reaches(context, b).
    out(context, callee) :- a__reaches(context, callee).
    same_ctx(a, b) :- a__samectx(a, b).
}

crate::fixture_io! {
    TupleContextComp;
    inputs { call => "Call.csv" }
    outputs { out => "Out.csv", same_ctx => "SameCtx.csv" }
}
