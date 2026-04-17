/// Engine — holds immutable dialect data from askicc's rkyv output.
///
/// Provides dialect lookup by DialectKind.

use std::collections::HashMap;
use aski_core::*;

pub struct Engine {
    data: &'static [u8],
    dialect_index: HashMap<u8, usize>, // DialectKind discriminant → index
}

impl Engine {
    pub fn from_bytes(data: &'static [u8]) -> Self {
        let tree = Self::access_tree(data);
        let mut dialect_index = HashMap::new();
        for (i, dialect) in tree.dialects.iter().enumerate() {
            let disc = dialect_kind_discriminant(&dialect.kind);
            dialect_index.insert(disc, i);
        }
        Engine { data, dialect_index }
    }

    pub fn tree(&self) -> &ArchivedDialectTree {
        Self::access_tree(self.data)
    }

    pub fn lookup(&self, kind: &ArchivedDialectKind) -> &ArchivedDialect {
        let disc = dialect_kind_discriminant(kind);
        let idx = self.dialect_index.get(&disc)
            .unwrap_or_else(|| panic!("dialect not found: {:?}", kind));
        &self.tree().dialects[*idx]
    }

    pub fn is_left_recursive(&self, kind: &ArchivedDialectKind) -> bool {
        let dialect = self.lookup(kind);
        for rule in dialect.rules.iter() {
            if let ArchivedRule::OrderedChoice { alternatives } = rule {
                for alt in alternatives.iter() {
                    if !alt.items.is_empty() {
                        if let ArchivedItemContent::DialectRef { target } = &alt.items[0].content {
                            if dialect_kind_discriminant(target) == dialect_kind_discriminant(kind) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn access_tree(data: &[u8]) -> &ArchivedDialectTree {
        unsafe { rkyv::access_unchecked::<DialectTree>(data) }
    }
}

/// Get discriminant byte for a DialectKind for comparison.
fn dialect_kind_discriminant(kind: &ArchivedDialectKind) -> u8 {
    // Safety: ArchivedDialectKind is a repr(u8) enum from rkyv
    unsafe { *(kind as *const ArchivedDialectKind as *const u8) }
}
