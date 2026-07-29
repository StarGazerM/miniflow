#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

mod fixture {
    macro_rules! program {
        ($($program:tt)*) => {
            miniflow_macro::miniflow! {
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

    pub(crate) use program;
}

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/doop.rs"));
