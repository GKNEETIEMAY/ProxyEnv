use serde::Serialize;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TunObservationState {
    NotDetected,
    Possible,
    Detected,
    Unknown,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TunEvidenceKind {
    TunnelInterfaceType,
    VirtualAdapterName,
    InterfaceOperational,
    EnumerationUnavailable,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunEvidence {
    pub kind: TunEvidenceKind,
    pub interface_name: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunObservation {
    pub state: TunObservationState,
    pub interface_name: Option<String>,
    pub description: Option<String>,
    pub evidence: Vec<TunEvidence>,
}
