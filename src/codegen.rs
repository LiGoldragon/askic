/// Codegen — domain tree → scoped Rust enums/structs.
///
/// Reuses the enum-as-index pattern from askicc.
/// Walks the AskiProgram and emits Rust source.

pub struct CodegenOutput {
    pub source: String,
}

// TODO: implement — same pattern as askicc/src/codegen.rs
