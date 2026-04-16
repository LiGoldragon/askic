/// Sema serialization — AskiProgram → .sema binary via rkyv.
///
/// The arena IS the .sema file. rkyv zero-copy mmap.
/// No recursive types — all refs are arena indices.

use crate::domain::AskiProgram;

impl AskiProgram {
    pub fn to_sema_bytes(&self) -> Vec<u8> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .expect("sema serialization failed")
            .to_vec()
    }
}
