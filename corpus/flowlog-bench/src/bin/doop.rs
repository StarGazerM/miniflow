#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

mod ascent {
    macro_rules! ascent_par {
        ($($program:tt)*) => {
            miniflow::miniflow! {
                #![flowlog_batch]
                #![output(
                    mainclass,
                    method_simplename,
                    method_declaringtype,
                    method_modifier,
                    method_paramtypes,
                    method_returntype,
                    method_descriptor,
                    methodlookup,
                    methodimplemented,
                    directsubclass,
                    subclass,
                    superclass,
                    superinterface,
                    subtypeof,
                    supertypeof,
                    subtypeofdifferent,
                    mainmethoddeclaration,
                    classinitializer,
                    initializedclass,
                    assign,
                    varpointsto,
                    instancefieldpointsto,
                    staticfieldpointsto,
                    callgraphedge,
                    arrayindexpointsto,
                    reachable
                )]
                $($program)*
            }
        };
    }

    pub(crate) use ascent_par;
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../flowlog-bench/programs/oracle/ascent/doop/src/main.rs"
));
