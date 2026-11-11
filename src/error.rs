//! Stable machine-readable operation failures.
use serde::{Deserialize, Serialize};
/// Stable public error categories; messages are diagnostic context, never a parser API.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// A new-output operation found an existing output.
    OutputExists,
    /// Saved progress does not match the requested frozen identity.
    ResumeMismatch,
    /// A writer owns the workspace, or safe locking is unavailable.
    Locked,
    /// Transport/HTTP failure other than authentication or TLS.
    Network,
    /// File, manifest, inventory or checksum integrity failure.
    Integrity,
    /// A declared byte, file or resource budget was exceeded.
    Budget,
    /// A destination path is already present or refused redeployment.
    RemoteExists,
    /// The server refused authentication/authorization.
    Auth,
    /// TLS trust/handshake configuration or verification failed.
    Tls,
    /// Cooperative cancellation was requested.
    Cancelled,
    /// Invalid arguments, configuration or unsupported state.
    Invalid,
    /// Local filesystem/stream I/O failed.
    Io,
    /// An unexpected internal failure occurred.
    Internal,
}
/// Typed failure accessible by downcasting the shared boxed [`crate::Result`] error.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Error {
    /// Stable category for UI decisions.
    pub kind: ErrorKind,
    /// Human context; may change without a protocol version change.
    pub message: String,
    /// Optional partial-operation report; never evidence of complete success.
    pub report: Option<toml::Value>,
}
impl Error {
    /// Construct a typed error without partial results.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            report: None,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}
/// Read a stable kind from typed, transport or filesystem failures.
/// Unclassified legacy/custom-source errors are `Invalid`; never match message text.
pub fn error_kind(error: &(dyn std::error::Error + 'static)) -> ErrorKind {
    fn nested(error: &(dyn std::error::Error + 'static), depth: usize) -> Option<ErrorKind> {
        if depth > 16 {
            return None;
        }
        if let Some(e) = error.downcast_ref::<Error>() {
            return Some(e.kind);
        }
        if let Some(e) = error.downcast_ref::<std::io::Error>() {
            if let Some(inner) = e.get_ref() {
                if let Some(kind) = nested(inner, depth + 1) {
                    return Some(kind);
                }
            }
        }
        error.source().and_then(|e| nested(e, depth + 1))
    }
    if let Some(kind) = nested(error, 0) {
        return kind;
    }
    if let Some(e) = error.downcast_ref::<std::io::Error>() {
        if let Some(inner) = e.get_ref() {
            if let Some(e) = inner.downcast_ref::<Error>() {
                return e.kind;
            }
        }
        return match e.kind() {
            std::io::ErrorKind::AlreadyExists => ErrorKind::OutputExists,
            std::io::ErrorKind::WouldBlock => ErrorKind::Locked,
            _ => ErrorKind::Io,
        };
    }
    if let Some(e) = error.downcast_ref::<ureq::Error>() {
        return match e {
            ureq::Error::Status(401 | 403, _) => ErrorKind::Auth,
            ureq::Error::Status(409 | 412, _) => ErrorKind::RemoteExists,
            ureq::Error::Status(_, _) => ErrorKind::Network,
            ureq::Error::Transport(t) => match t.kind() {
                ureq::ErrorKind::ConnectionFailed
                    if t.message() == Some("tls connection init failed") =>
                {
                    ErrorKind::Tls
                }
                _ => ErrorKind::Network,
            },
        };
    }
    ErrorKind::Invalid
}
pub(crate) fn normalized(
    error: Box<dyn std::error::Error + Send + Sync>,
) -> Box<dyn std::error::Error + Send + Sync> {
    if error.is::<Error>() {
        return error;
    }
    Box::new(Error::new(error_kind(error.as_ref()), error.to_string()))
}
