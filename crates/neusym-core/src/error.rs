use miette::Diagnostic;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, NeusymError>;

#[derive(Debug, Error, Diagnostic)]
pub enum NeusymError {
    #[error("provider error: {0}")]
    #[diagnostic(code(neusym::provider))]
    Provider(String),

    #[error("mapping not found: {0}")]
    #[diagnostic(code(neusym::mapping_not_found))]
    MappingNotFound(String),

    #[error("conflict on field `{field}`: source=`{source_val}`, target=`{target_val}`")]
    #[diagnostic(
        code(neusym::conflict),
        help("resolve manually or set a sync direction preference")
    )]
    Conflict {
        field: String,
        source_val: String,
        target_val: String,
    },

    #[error("serialization error: {0}")]
    #[diagnostic(code(neusym::serde))]
    Serde(#[from] serde_json::Error),

    #[error("io error: {0}")]
    #[diagnostic(code(neusym::io))]
    Io(#[from] std::io::Error),

    #[error("http error: {0}")]
    #[diagnostic(code(neusym::http))]
    Http(String),

    #[error("missing credential: {field}")]
    #[diagnostic(
        code(neusym::missing_credential),
        help("set {field} via environment variable or pass per-call")
    )]
    MissingCredential { field: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_credential_displays_field() {
        let err = NeusymError::MissingCredential {
            field: "LINEAR_API_KEY".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("LINEAR_API_KEY"));
    }
}

/// Convert into crux's CruxErr for pipeline integration.
impl NeusymError {
    pub fn into_crux_err(self) -> crux_types::error::CruxErr {
        crux_types::error::CruxErr::step_failed("neusym", self.to_string())
    }
}
