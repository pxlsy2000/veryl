use crate::ir::ModportMemberPath;
use crate::symbol::Direction;
use veryl_parser::resource_table::StrId;
use veryl_parser::token_range::TokenRange;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum NestedModportAnalysisInvariant {
    #[error("nested modport analysis was already finalized")]
    AlreadyFinalized,
    #[error("pending nested modport analysis state is poisoned")]
    PendingStatePoisoned,
    #[error("record belongs to another analysis session")]
    CrossSession,
    #[error("conflicting semantic path rewrite")]
    ConflictingRewrite,
    #[error("conflicting expanded port resolution")]
    ConflictingExpandedPort,
    #[error("nested path rewrite could not resolve its terminal")]
    UnresolvedRewrite,
    #[error("expanded port target has no finalized interface lowering")]
    UnresolvedExpandedPort,
    #[error("emission binding references unavailable lowering")]
    MissingLowering,
    #[error("emission binding references unavailable path rewrite")]
    MissingRewrite,
    #[error("emission binding references unavailable expanded port")]
    MissingExpandedPort,
    #[error("emission binding references a record owned by another specialization")]
    CrossOwnerRecord,
    #[error("emission specialization context does not match its binding specialization")]
    MismatchedEmissionContext,
    #[error("emission declaration batch is missing, duplicated, or out of order")]
    DeclarationOrder,
    #[error("emission binding ID is duplicated")]
    DuplicateBindingId,
    #[error("emission binding ID space is exhausted")]
    EmissionBindingOverflow,
    #[error("emission cursor has unconsumed owner bindings")]
    UnconsumedBindings,
    #[error("emission package scope does not match an analyzer-owned specialization")]
    PackageScopeMismatch,
    #[error("emission frame did not publish the requested semantic record")]
    RecordNotRequired,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum NestedModportFinalizeError {
    #[error(transparent)]
    Invariant(#[from] NestedModportAnalysisInvariant),
    #[error("local reference below a removed nested interface root was not lowered")]
    UnloweredLocalReference {
        semantic_segments: Vec<StrId>,
        occurrence: TokenRange,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NestedLoweringResolveError {
    ExpansionBudget {
        requested: usize,
        limit: usize,
    },
    MissingModport {
        name: StrId,
        origin: TokenRange,
    },
    DefaultCycle {
        target: StrId,
        origin: TokenRange,
    },
    EmptyModport {
        name: StrId,
        origin: TokenRange,
    },
    UnsupportedMemberDirection {
        path: ModportMemberPath,
        direction: Direction,
        origin: TokenRange,
    },
    MissingTerminal {
        path: ModportMemberPath,
        origin: TokenRange,
    },
    NonVariableTerminal {
        path: ModportMemberPath,
        actual_kind: String,
        origin: TokenRange,
        terminal: TokenRange,
    },
    UnemittableTerminal {
        path: ModportMemberPath,
        actual_type: String,
        origin: TokenRange,
        terminal: TokenRange,
    },
    FlatNameCollision {
        flat: StrId,
        first_path: ModportMemberPath,
        second_path: ModportMemberPath,
        first_origin: TokenRange,
        second_origin: TokenRange,
    },
}
