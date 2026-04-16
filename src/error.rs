use thiserror::Error;

#[derive(Debug, Error)]
pub enum RtangoError {
    #[error("spec file already exists (use --force to overwrite)")]
    SpecExists,

    #[error("no agents detected and none specified with --agent")]
    NoAgentsDetected,

    #[error("spec validation failed: {0}")]
    InvalidSpec(String),

    #[error("conflict on target {path}: {reason}")]
    Conflict { path: String, reason: String },
}
