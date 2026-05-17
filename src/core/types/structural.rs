use serde::{Deserialize, Serialize};

use super::Location;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StructuralChange {
    pub kind: StructuralChangeKind,
    pub name: String,
    pub detail: String,
    pub location: Location,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum StructuralChangeKind {
    FunctionAdded,
    FunctionRemoved,
    FunctionModified,
    FunctionRenamed,
    ClassAdded,
    ClassRemoved,
    ClassModified,
    MethodAdded,
    MethodRemoved,
    MethodModified,
    InterfaceAdded,
    InterfaceRemoved,
    InterfaceModified,
    TypeAdded,
    TypeRemoved,
    TypeModified,
    ImportAdded,
    ImportRemoved,
    ExportAdded,
    ExportRemoved,
    IfStatementAdded,
    TryCatchAdded,
    NullCheckAdded,
    OptionalChainAdded,
    TestCaseAdded,
    TestCaseModified,
    DescribeBlockAdded,
}
