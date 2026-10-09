//! Interned names for settlements and (from M2) villagers (SIM-10).

use bevy_ecs::prelude::*;

use crate::hash::{StateHash, StateHasher};

/// Index into [`NameTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NameId(pub u32);

/// Every generated name, so components store a 4-byte [`NameId`].
#[derive(Resource, Debug, Default, Clone)]
pub struct NameTable {
    names: Vec<String>,
}

impl NameTable {
    /// Stores a name and returns its id.
    pub fn push(&mut self, name: impl Into<String>) -> NameId {
        self.names.push(name.into());
        NameId(self.names.len() as u32 - 1)
    }

    /// The name with id `id`.
    pub fn get(&self, id: NameId) -> &str {
        self.names
            .get(id.0 as usize)
            .map_or("<unnamed>", String::as_str)
    }

    /// Number of names stored.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether no names are stored.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

impl StateHash for NameTable {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.names.len() as u64);
        for name in &self.names {
            h.u32(name.len() as u32);
            h.bytes(name.as_bytes());
        }
    }
}

crate::save_newtype!(NameId);
crate::save_fields!(NameTable { names });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_get() {
        let mut t = NameTable::default();
        let a = t.push("Ashford");
        let b = t.push("Brambleby");
        assert_eq!(t.get(a), "Ashford");
        assert_eq!(t.get(b), "Brambleby");
        assert_eq!(t.get(NameId(99)), "<unnamed>");
    }
}
