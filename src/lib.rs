/// askic — the aski frontend.
///
/// Reads .aski source → dialect state machine → rkyv parse tree.
/// Grammar from embedded aski-core rkyv. Output as sema-core types.

pub mod lexer;
#[cfg(test)]
mod lexer_tests;
pub mod typed;
pub mod machine;
pub mod values;
pub mod engine;
#[cfg(test)]
mod engine_tests;
pub mod builder;
