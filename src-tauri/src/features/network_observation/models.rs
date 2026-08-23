use serde::Serialize;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum TunObservationState {
    NotDetected,
    Possible,
    Detected,
    Unknown,
}
