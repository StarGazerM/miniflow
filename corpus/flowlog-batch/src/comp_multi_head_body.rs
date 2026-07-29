crate::fixture_program! {
    #![output(fwd, rev, reach)]
    pub struct CompMultiHeadBody;
    .decl edge(c0: String, c1: String)
    .decl fwd(c0: String, c1: String)
    .decl rev(c0: String, c1: String)
    .decl reach(c0: String, c1: String)
    .decl g__fwd(c0: String, c1: String)
    .decl g__rev(c0: String, c1: String)
    .decl g__reach(c0: String, c1: String)

    g__rev(y, x) :- edge(x, y).
    g__fwd(x, y) :- edge(x, y).
    g__reach(x, y) :- g__fwd(x, y).
    g__reach(x, y) :- g__reach(x, z), g__fwd(z, y).
    fwd(x, y) :- g__fwd(x, y).
    rev(x, y) :- g__rev(x, y).
    reach(x, y) :- g__reach(x, y).
}

crate::fixture_io! {
    CompMultiHeadBody;
    inputs { edge => "Edge.csv" }
    outputs { fwd => "Fwd.csv", rev => "Rev.csv", reach => "Reach.csv" }
}
