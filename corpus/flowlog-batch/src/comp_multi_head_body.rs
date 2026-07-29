crate::fixture_program! {
    #![output(fwd, rev, reach)]
    pub struct CompMultiHeadBody;
    relation edge(String, String);
    relation fwd(String, String);
    relation rev(String, String);
    relation reach(String, String);
    relation g__fwd(String, String);
    relation g__rev(String, String);
    relation g__reach(String, String);

    g__rev(y, x) :- edge(x, y);
    g__fwd(x, y) :- edge(x, y);
    g__reach(x, y) :- g__fwd(x, y);
    g__reach(x, y) :- g__reach(x, z), g__fwd(z, y);
    fwd(x, y) :- g__fwd(x, y);
    rev(x, y) :- g__rev(x, y);
    reach(x, y) :- g__reach(x, y);
}

crate::fixture_io! {
    CompMultiHeadBody;
    inputs { edge => "Edge.csv" }
    outputs { fwd => "Fwd.csv", rev => "Rev.csv", reach => "Reach.csv" }
}
