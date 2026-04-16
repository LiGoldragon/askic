/// Sema serialization — domain tree → .sema binary.
///
/// Bootstrap uses simple bincode-style serialization.
/// The self-hosted version will use rkyv for zero-copy.
///
/// TODO: rkyv recursive type support (needs manual impl
/// or structural changes to break cycles).

use crate::domain::AskiProgram;

impl AskiProgram {
    pub fn to_sema_bytes(&self) -> Vec<u8> {
        // bootstrap: use debug format as placeholder
        // real serialization needs rkyv with manual impls
        // for recursive types
        format!("{:#?}", self).into_bytes()
    }
}
