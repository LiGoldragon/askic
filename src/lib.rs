/// askic — the aski frontend.
///
/// Reads .aski source, produces .sema binary.
/// Contains cc + askicc baked in.

pub mod lexer;
#[cfg(test)]
mod lexer_tests;
pub mod domain;
pub mod parse;
#[cfg(test)]
mod parse_tests;
pub mod codegen;
pub mod sema;
