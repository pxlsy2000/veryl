use super::expaneded_modport::{ExpandedModportPort, ExpandedModportPortTableEntry};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use veryl_analyzer::nested_modport::{ResolvedExpandedMemberId, ResolvedExpandedMemberSet};
use veryl_parser::resource_table::{self, StrId};

#[cfg(test)]
fn record_work(units: usize) {
    veryl_analyzer::nested_modport::record_external_emitter_index_work(units);
}

#[cfg(not(test))]
const fn record_work(_units: usize) {}

#[cfg(test)]
fn member_scan_mutation_enabled() -> bool {
    veryl_analyzer::nested_modport::expanded_member_scan_mutation_enabled()
}

#[cfg(not(test))]
const fn member_scan_mutation_enabled() -> bool {
    false
}

#[cfg(test)]
fn record_resolved_member_lookup() {
    veryl_analyzer::nested_modport::record_resolved_member_lookup();
}

#[cfg(not(test))]
const fn record_resolved_member_lookup() {}

#[derive(Clone, Copy, Debug)]
struct PortIdentity(StrId);

impl PortIdentity {
    fn new(value: StrId) -> Self {
        Self(resource_table::canonical_str_id(value))
    }
}

impl PartialEq for PortIdentity {
    fn eq(&self, other: &Self) -> bool {
        record_work(1);
        self.0 == other.0
    }
}

impl Eq for PortIdentity {}

impl Hash for PortIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_work(1);
        self.0.hash(state);
    }
}

#[derive(Clone, Copy, Debug)]
struct SegmentIdentity(StrId);

impl SegmentIdentity {
    fn new(value: StrId) -> Self {
        Self(resource_table::canonical_str_id(value))
    }
}

impl PartialEq for SegmentIdentity {
    fn eq(&self, other: &Self) -> bool {
        record_work(1);
        self.0 == other.0
    }
}

impl Eq for SegmentIdentity {}

impl Hash for SegmentIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_work(1);
        self.0.hash(state);
    }
}

enum MemberPathIndex {
    Direct(HashMap<SegmentIdentity, ResolvedExpandedMemberId>),
    Resolved(Arc<ResolvedExpandedMemberSet>),
}

#[derive(Clone, Copy)]
struct MemberLocation {
    group: usize,
    port: usize,
}

struct EntryIndex {
    paths: MemberPathIndex,
    locations: HashMap<ResolvedExpandedMemberId, HashMap<Vec<isize>, MemberLocation>>,
}

#[derive(Default)]
pub(super) struct ExpandedModportLookupIndex {
    entries: HashMap<PortIdentity, usize>,
    members: Vec<EntryIndex>,
}

impl ExpandedModportLookupIndex {
    pub(super) fn build(entries: &[ExpandedModportPortTableEntry]) -> Self {
        let mut index = Self::default();
        for (entry_index, entry) in entries.iter().enumerate() {
            record_work(1);
            index
                .entries
                .insert(PortIdentity::new(entry.id), entry_index);
            let paths = if let Some(members) = &entry.resolved_members {
                MemberPathIndex::Resolved(Arc::clone(members))
            } else {
                let paths = entry
                    .ports
                    .first()
                    .into_iter()
                    .flat_map(|group| group.ports.iter())
                    .enumerate()
                    .filter_map(|(member, port)| {
                        let member = u32::try_from(member).ok().map(ResolvedExpandedMemberId)?;
                        let segment = port.source_segments.first().copied()?;
                        record_work(1);
                        Some((SegmentIdentity::new(segment), member))
                    })
                    .collect();
                MemberPathIndex::Direct(paths)
            };
            let mut locations: HashMap<_, HashMap<_, _>> = HashMap::new();
            for (group_index, group) in entry.ports.iter().enumerate() {
                for (port_index, port) in group.ports.iter().enumerate() {
                    record_work(1);
                    let Some(member) = entry
                        .resolved_members
                        .as_ref()
                        .and_then(|members| members.members().get(port_index))
                        .map(|member| member.id)
                        .or_else(|| u32::try_from(port_index).ok().map(ResolvedExpandedMemberId))
                    else {
                        continue;
                    };
                    locations.entry(member).or_default().insert(
                        port.array_index.clone(),
                        MemberLocation {
                            group: group_index,
                            port: port_index,
                        },
                    );
                }
            }
            index.members.push(EntryIndex { paths, locations });
        }
        index
    }

    pub(super) fn entry(&self, port: StrId) -> Option<usize> {
        record_work(1);
        self.entries.get(&PortIdentity::new(port)).copied()
    }

    pub(super) fn member<'a>(
        &self,
        entries: &'a [ExpandedModportPortTableEntry],
        port: StrId,
        path: &[StrId],
        array_index: &[isize],
    ) -> Option<&'a ExpandedModportPort> {
        let entry_index = self.entry(port)?;
        let entry = self.members.get(entry_index)?;
        let member = match &entry.paths {
            MemberPathIndex::Direct(paths) => {
                if member_scan_mutation_enabled() {
                    return entries
                        .get(entry_index)?
                        .ports
                        .iter()
                        .flat_map(|ports| ports.ports.iter())
                        .filter(|port| {
                            record_work(1);
                            path.starts_with(&port.source_segments)
                                && port.array_index == array_index
                        })
                        .max_by_key(|port| port.source_segments.len());
                }
                record_work(1);
                paths.get(&SegmentIdentity::new(*path.first()?)).copied()?
            }
            MemberPathIndex::Resolved(paths) => {
                record_resolved_member_lookup();
                paths.longest_prefix(path)?.id
            }
        };
        record_work(1);
        let location = entry.locations.get(&member)?.get(array_index)?;
        entries
            .get(entry_index)?
            .ports
            .get(location.group)?
            .ports
            .get(location.port)
    }
}
