use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub description: String,
    pub location: Location,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum EvidenceKind {
    NullGuardAdded,
    OptionalChainingAdded,
    ErrorHandlerAdded,
    BoundaryCheckAdded,
    TypeNarrowed,
    FunctionExtracted,
    FunctionInlined,
    SymbolRenamed,
    DelegationPattern,
    CodeMoved,
    ExportAdded,
    NewPublicApi,
    NewFileAdded,
    AuthCheckAdded,
    InputValidationAdded,
    SanitizationAdded,
    TestCaseAdded,
    TestCaseModified,
    TestFileAdded,
    PackageFileChanged,
    LockFileChanged,
    VersionBumped,
    MarkdownFileChanged,
    CommentAdded,
    DocStringAdded,
    LinkUpdated,
    ConfigFileChanged,
    EnvFileChanged,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Location {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}
