use super::emitter_index_work::{
    connected_map_scan_mutation_enabled, emitter_index_hash_collision_forced,
    record_emitter_index_work,
};
use crate::HashMap;
use crate::symbol::GenericMap;
use std::hash::{Hash, Hasher};
use veryl_parser::resource_table::{self, StrId};

#[derive(Clone, Copy, Debug)]
struct ConnectedInterfaceIdentity(StrId);

impl ConnectedInterfaceIdentity {
    fn new(formal_port: StrId) -> Self {
        Self(resource_table::canonical_str_id(formal_port))
    }
}

impl PartialEq for ConnectedInterfaceIdentity {
    fn eq(&self, other: &Self) -> bool {
        record_emitter_index_work(1);
        self.0 == other.0
    }
}

impl Eq for ConnectedInterfaceIdentity {}

impl Hash for ConnectedInterfaceIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_emitter_index_work(1);
        if !emitter_index_hash_collision_forced() {
            self.0.hash(state);
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ConnectedGenericMapIndex(HashMap<ConnectedInterfaceIdentity, usize>);

impl ConnectedGenericMapIndex {
    pub(super) fn build(maps: &[(StrId, GenericMap)]) -> Self {
        Self(
            maps.iter()
                .enumerate()
                .map(|(index, (formal_port, _))| {
                    record_emitter_index_work(1);
                    (ConnectedInterfaceIdentity::new(*formal_port), index)
                })
                .collect(),
        )
    }

    pub(super) fn get<'a>(
        &self,
        maps: &'a [(StrId, GenericMap)],
        formal_port: StrId,
    ) -> Option<&'a GenericMap> {
        if connected_map_scan_mutation_enabled() {
            return maps.iter().find_map(|(formal, map)| {
                record_emitter_index_work(1);
                (*formal == formal_port).then_some(map)
            });
        }
        let index = self.0.get(&ConnectedInterfaceIdentity::new(formal_port))?;
        record_emitter_index_work(1);
        maps.get(*index).map(|(_, map)| map)
    }

    pub(super) fn matches(&self, maps: &[(StrId, GenericMap)]) -> bool {
        maps.len() == self.0.len()
            && maps.iter().enumerate().all(|(index, (formal_port, _))| {
                self.0.get(&ConnectedInterfaceIdentity::new(*formal_port)) == Some(&index)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolId;

    #[test]
    fn exact_lookup_rejects_missing_and_duplicate_formal_identities() {
        let first = resource_table::insert_str("first");
        let second = resource_table::insert_str("second");
        let mut map8 = GenericMap::default();
        map8.id = Some(SymbolId(8));
        let mut map16 = GenericMap::default();
        map16.id = Some(SymbolId(16));
        let maps = vec![(first, map8), (second, map16)];
        let index = ConnectedGenericMapIndex::build(&maps);
        assert_eq!(
            index.get(&maps, first).and_then(|map| map.id),
            Some(SymbolId(8))
        );
        assert_eq!(
            index.get(&maps, second).and_then(|map| map.id),
            Some(SymbolId(16))
        );
        assert!(
            index
                .get(&maps, resource_table::insert_str("missing"))
                .is_none()
        );
        assert!(index.matches(&maps));

        let duplicated = vec![
            (first, GenericMap::default()),
            (first, GenericMap::default()),
        ];
        assert!(!ConnectedGenericMapIndex::build(&duplicated).matches(&duplicated));
    }
}
