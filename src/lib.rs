/// askic — the aski frontend.
///
/// Reads .aski source → dialect state machine → rkyv parse tree.
/// Grammar from embedded aski-core rkyv. Output as sema-core types.

pub mod lexer;
#[cfg(test)]
mod lexer_tests;
