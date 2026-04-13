//! askic-ffi — FFI implementations for the aski compiler.
//!
//! Each module matches an FFI library declaration in aski source.
//! Generated code calls askic_ffi::module::function(...).

pub mod file_system;
pub mod std_string;
