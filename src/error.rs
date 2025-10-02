use std::fmt::Formatter;
use librespot::oauth::OAuthError;

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub situation: Option<String>,
    pub details: Box<dyn std::error::Error + Send + Sync>,
}

#[derive(Debug)]
pub enum ErrorKind {
    Unexpected
}

impl Error {
    pub fn new<E>(kind: ErrorKind, error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>
    {
        Self {
            kind,
            situation: None,
            details: error.into()
        }
    }

    /// String preferably starts with "while", "during" or "when"
    pub fn situation(mut self, situation: impl Into<String>) -> Self {
        self.situation = Some(situation.into());
        self
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { self.details.source() }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)?;

        if let Some(situation) = &self.situation {
            write!(f, " ({})", situation)?;
        }

        write!(f, ": {{ ")?;
        self.details.fmt(f)?;
        write!(f, " }}")
    }
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::Unexpected => write!(f, "Unexpected error"),
        }
    }
}

impl From<OAuthError> for Error {
    fn from(value: OAuthError) -> Self {
        Self::new(ErrorKind::Unexpected, value).situation("while handling OAuth")
    }
}