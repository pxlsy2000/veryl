use super::emission_frame::EmissionFrame;
use super::errors::NestedModportAnalysisInvariant;
use super::semantic_records::*;
use super::specialization_context::LoweringAvailability;
use veryl_parser::resource_table::TokenId;

#[cfg(test)]
use super::frame_record_query_mutation::{
    owner_record_lookup_mutation_enabled, required_record_scan_mutation_enabled,
};
#[cfg(test)]
use super::semantic_work::record_semantic_work;

impl<'a> EmissionFrame<'a> {
    #[cfg(test)]
    fn requires_rewrite(&self, key: &OccurrenceRewriteKey) -> bool {
        self.binding.required_rewrites.iter().any(|required| {
            record_semantic_work(1);
            required == key
        })
    }

    #[cfg(test)]
    fn requires_expanded_port(&self, key: &ExpandedPortKey) -> bool {
        self.binding.required_expanded_ports.iter().any(|required| {
            record_semantic_work(1);
            required == key
        })
    }

    pub fn rewrite(
        self,
        kind: OccurrenceKind,
        token: TokenId,
    ) -> Result<Option<&'a ResolvedPathRewrite>, NestedModportAnalysisInvariant> {
        #[cfg(test)]
        if owner_record_lookup_mutation_enabled() {
            let key = OccurrenceRewriteKey {
                owner: self.binding.specialization.as_ref().clone(),
                kind,
                token,
            };
            if !self.binding.frame_records.legacy_requires_rewrite(&key) {
                return Err(NestedModportAnalysisInvariant::RecordNotRequired);
            }
            return Ok(self.analysis.rewrite(&key));
        }
        #[cfg(test)]
        if required_record_scan_mutation_enabled() {
            let key = OccurrenceRewriteKey {
                owner: self.binding.specialization.as_ref().clone(),
                kind,
                token,
            };
            if !self.requires_rewrite(&key) {
                return Err(NestedModportAnalysisInvariant::RecordNotRequired);
            }
            return Ok(self.analysis.rewrite(&key));
        }
        self.binding
            .frame_records
            .rewrite(kind, token)
            .map(Some)
            .ok_or(NestedModportAnalysisInvariant::RecordNotRequired)
    }

    pub fn published_rewrite(
        self,
        kind: OccurrenceKind,
        token: TokenId,
    ) -> Option<&'a ResolvedPathRewrite> {
        #[cfg(test)]
        if owner_record_lookup_mutation_enabled() {
            let key = OccurrenceRewriteKey {
                owner: self.binding.specialization.as_ref().clone(),
                kind,
                token,
            };
            return self
                .binding
                .frame_records
                .legacy_requires_rewrite(&key)
                .then(|| self.analysis.rewrite(&key))
                .flatten();
        }
        #[cfg(test)]
        if required_record_scan_mutation_enabled() {
            let key = OccurrenceRewriteKey {
                owner: self.binding.specialization.as_ref().clone(),
                kind,
                token,
            };
            return self
                .requires_rewrite(&key)
                .then(|| self.analysis.rewrite(&key))
                .flatten();
        }
        self.binding.frame_records.rewrite(kind, token)
    }

    pub fn expanded_port(
        self,
        token: TokenId,
    ) -> Result<Option<&'a ExpandedPortResolution>, NestedModportAnalysisInvariant> {
        #[cfg(test)]
        if owner_record_lookup_mutation_enabled() {
            let key = ExpandedPortKey {
                owner: self.binding.specialization.as_ref().clone(),
                token,
            };
            if !self
                .binding
                .frame_records
                .legacy_requires_expanded_port(&key)
            {
                return Err(NestedModportAnalysisInvariant::RecordNotRequired);
            }
            if self.analysis.expanded_port(&key).is_none() {
                return Err(NestedModportAnalysisInvariant::MissingExpandedPort);
            }
            return Ok(self.binding.frame_records.expanded_port(token));
        }
        #[cfg(test)]
        if required_record_scan_mutation_enabled() {
            let key = ExpandedPortKey {
                owner: self.binding.specialization.as_ref().clone(),
                token,
            };
            if !self.requires_expanded_port(&key) {
                return Err(NestedModportAnalysisInvariant::RecordNotRequired);
            }
            return Ok(self.binding.frame_records.expanded_port(token));
        }
        self.binding
            .frame_records
            .expanded_port(token)
            .map(Some)
            .ok_or(NestedModportAnalysisInvariant::RecordNotRequired)
    }

    pub fn published_expanded_port(self, token: TokenId) -> Option<&'a ExpandedPortResolution> {
        #[cfg(test)]
        if owner_record_lookup_mutation_enabled() {
            let key = ExpandedPortKey {
                owner: self.binding.specialization.as_ref().clone(),
                token,
            };
            return self
                .binding
                .frame_records
                .legacy_requires_expanded_port(&key)
                .then(|| self.analysis.expanded_port(&key))
                .flatten()
                .and_then(|_| self.binding.frame_records.expanded_port(token));
        }
        #[cfg(test)]
        if required_record_scan_mutation_enabled() {
            let key = ExpandedPortKey {
                owner: self.binding.specialization.as_ref().clone(),
                token,
            };
            return self
                .requires_expanded_port(&key)
                .then(|| self.binding.frame_records.expanded_port(token))
                .flatten();
        }
        self.binding.frame_records.expanded_port(token)
    }

    pub fn expanded_port_for_emission(
        self,
        token: TokenId,
    ) -> Result<Option<&'a ExpandedPortResolution>, NestedModportAnalysisInvariant> {
        #[cfg(test)]
        {
            let key = ExpandedPortKey {
                owner: self.binding.specialization.as_ref().clone(),
                token,
            };
            if owner_record_lookup_mutation_enabled()
                && self
                    .binding
                    .frame_records
                    .legacy_requires_expanded_port(&key)
            {
                if self.analysis.expanded_port(&key).is_none() {
                    return Err(NestedModportAnalysisInvariant::MissingExpandedPort);
                }
            }
            if required_record_scan_mutation_enabled() && self.requires_expanded_port(&key) {
                if self.analysis.expanded_port(&key).is_none() {
                    return Err(NestedModportAnalysisInvariant::MissingExpandedPort);
                }
            }
        }
        if let Some(resolution) = self.binding.frame_records.expanded_port(token) {
            return Ok(Some(resolution));
        }
        match self.binding.lowering {
            LoweringAvailability::NotNested => Ok(None),
            LoweringAvailability::Found(_) => {
                Err(NestedModportAnalysisInvariant::MissingExpandedPort)
            }
        }
    }
}
