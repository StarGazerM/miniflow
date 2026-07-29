crate::fixture_program! {
    pub struct TupleContextComp;
    relation call(String, String, String);
    relation out((String, String), String);
    relation same_ctx(String, String);
    relation a__reaches((String, String), String);
    relation a__samectx(String, String);

    a__reaches((caller, site), callee) :- call(caller, callee, site);
    a__samectx(a, b) :- a__reaches(context, a), a__reaches(context, b);
    out(context, callee) :- a__reaches(context, callee);
    same_ctx(a, b) :- a__samectx(a, b);
}

crate::fixture_io! {
    TupleContextComp;
    inputs { call => "Call.csv" }
    outputs { out => "Out.csv", same_ctx => "SameCtx.csv" }
}
