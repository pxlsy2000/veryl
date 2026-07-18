use super::emitter_index_work::{
    emitter_index_hash_collision_forced, expanded_member_scan_mutation_enabled,
    record_emitter_index_work,
};
use super::errors::NestedModportAnalysisInvariant;
use super::lowering_records::{NestedModportLowering, ResolvedModportTerminal};
use super::semantic_type::ResolvedDeclarationType;
use crate::HashMap;
use crate::symbol::Direction;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use veryl_parser::resource_table::{self, StrId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResolvedExpandedMemberId(pub u32);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResolvedExpandedMember {
    pub id: ResolvedExpandedMemberId,
    pub source_segments: Vec<StrId>,
    pub identifier: StrId,
    pub declaration: ResolvedDeclarationType,
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug)]
struct MemberSegment(StrId);

impl MemberSegment {
    fn new(segment: StrId) -> Self {
        Self(resource_table::canonical_str_id(segment))
    }
}

impl PartialEq for MemberSegment {
    fn eq(&self, other: &Self) -> bool {
        record_emitter_index_work(1);
        self.0 == other.0
    }
}

impl Eq for MemberSegment {}

impl Hash for MemberSegment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_emitter_index_work(1);
        if !emitter_index_hash_collision_forced() {
            self.0.hash(state);
        }
    }
}

#[derive(Clone, Debug, Default)]
struct MemberPathNode {
    children: HashMap<MemberSegment, usize>,
    member: Option<ResolvedExpandedMemberId>,
}

#[derive(Clone, Debug)]
pub struct ResolvedExpandedMemberSet {
    members: Arc<[ResolvedExpandedMember]>,
    nodes: Arc<[MemberPathNode]>,
}

impl Default for ResolvedExpandedMemberSet {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl ResolvedExpandedMemberSet {
    pub(super) fn from_lowering(
        lowering: &NestedModportLowering,
        modport: StrId,
    ) -> Result<Self, NestedModportAnalysisInvariant> {
        let resolved = lowering
            .modports
            .get(&modport)
            .ok_or(NestedModportAnalysisInvariant::MissingExpandedPort)?;
        let mut members = Vec::new();
        for entry in resolved.entries.iter() {
            let value = match &entry.terminal {
                Some(ResolvedModportTerminal::FlattenedVariable { terminal }) => {
                    let terminal = lowering
                        .terminals
                        .get(terminal.0 as usize)
                        .ok_or(NestedModportAnalysisInvariant::MissingExpandedPort)?;
                    Some((
                        terminal.identifier.source_segments.clone(),
                        terminal.emitted_identifier.logical(),
                        terminal.resolved_type.declaration.clone(),
                    ))
                }
                Some(ResolvedModportTerminal::DirectVariable {
                    emitted_identifier,
                    resolved_type,
                    ..
                }) => Some((
                    entry.path.as_slice().to_vec(),
                    emitted_identifier.logical(),
                    resolved_type.declaration.clone(),
                )),
                Some(ResolvedModportTerminal::DirectFunction { .. }) | None => None,
            };
            let Some((source_segments, identifier, declaration)) = value else {
                continue;
            };
            let id = u32::try_from(members.len())
                .map(ResolvedExpandedMemberId)
                .map_err(|_| NestedModportAnalysisInvariant::MissingExpandedPort)?;
            members.push(ResolvedExpandedMember {
                id,
                source_segments,
                identifier,
                declaration,
                direction: entry.direction,
            });
        }
        Ok(Self::new(members))
    }

    fn new(members: Vec<ResolvedExpandedMember>) -> Self {
        let mut nodes = vec![MemberPathNode::default()];
        for member in &members {
            let mut node = 0;
            for segment in &member.source_segments {
                record_emitter_index_work(1);
                let key = MemberSegment::new(*segment);
                let child = if let Some(child) = nodes[node].children.get(&key) {
                    *child
                } else {
                    let child = nodes.len();
                    nodes.push(MemberPathNode::default());
                    nodes[node].children.insert(key, child);
                    child
                };
                node = child;
            }
            nodes[node].member = Some(member.id);
        }
        Self {
            members: members.into(),
            nodes: nodes.into(),
        }
    }

    pub fn members(&self) -> &[ResolvedExpandedMember] {
        &self.members
    }

    pub fn longest_prefix(&self, path: &[StrId]) -> Option<&ResolvedExpandedMember> {
        if expanded_member_scan_mutation_enabled() {
            return self
                .members
                .iter()
                .filter(|member| {
                    record_emitter_index_work(1);
                    path.starts_with(&member.source_segments)
                })
                .max_by_key(|member| member.source_segments.len());
        }
        let mut node = 0;
        let mut selected = self.nodes[node].member;
        for segment in path {
            record_emitter_index_work(1);
            let Some(next) = self.nodes[node].children.get(&MemberSegment::new(*segment)) else {
                break;
            };
            node = *next;
            selected = self.nodes[node].member.or(selected);
        }
        selected.and_then(|id| self.members.get(id.0 as usize))
    }
}

impl PartialEq for ResolvedExpandedMemberSet {
    fn eq(&self, other: &Self) -> bool {
        self.members == other.members
    }
}

impl Eq for ResolvedExpandedMemberSet {}

impl Hash for ResolvedExpandedMemberSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.members.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::super::semantic_type::ResolvedTerminalType;
    use super::*;
    use crate::ir::{Type, TypeKind};

    fn member(id: u32, path: &[StrId]) -> ResolvedExpandedMember {
        ResolvedExpandedMember {
            id: ResolvedExpandedMemberId(id),
            source_segments: path.to_vec(),
            identifier: resource_table::insert_str(&format!("member_{id}")),
            declaration: ResolvedTerminalType::try_from_ir(&Type::new(TypeKind::Logic))
                .expect("logic declaration")
                .declaration,
            direction: Direction::Input,
        }
    }

    #[test]
    fn semantic_trie_preserves_longest_prefix_missing_and_later_duplicate_precedence() {
        let child = resource_table::insert_str("child");
        let payload = resource_table::insert_str("payload");
        let field = resource_table::insert_str("field");
        let missing = resource_table::insert_str("missing");
        let members = ResolvedExpandedMemberSet::new(vec![
            member(0, &[child]),
            member(1, &[child, payload]),
            member(2, &[child, payload]),
        ]);
        assert_eq!(
            members
                .longest_prefix(&[child, payload, field])
                .map(|member| member.id),
            Some(ResolvedExpandedMemberId(2))
        );
        assert_eq!(
            members
                .longest_prefix(&[child, field])
                .map(|member| member.id),
            Some(ResolvedExpandedMemberId(0))
        );
        assert!(members.longest_prefix(&[missing]).is_none());
    }
}
