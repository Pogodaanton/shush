use librespot::oauth::OAuthError;
use std::fmt::Formatter;
use std::io;
use tokio::sync::mpsc::error::SendError;

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub situation: Option<String>,
    pub details: Box<dyn std::error::Error + Send + Sync>,
}

#[derive(Debug)]
pub enum ErrorKind {
    Unexpected,
    IOError,
    InvalidData,
    LibrespotError,
    ChannelClosed,
}

impl Error {
    pub fn new<E>(kind: ErrorKind, error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind,
            situation: None,
            details: error.into(),
        }
    }

    /// String preferably starts with "while", "during" or "when"
    pub fn situation(mut self, situation: impl Into<String>) -> Self {
        self.situation = Some(situation.into());
        self
    }

    #[allow(dead_code)]
    pub fn io_error<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self::new(ErrorKind::IOError, error)
    }

    #[allow(dead_code)]
    pub fn invalid_data<S>(message: S) -> Self
    where
        S: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self::new(ErrorKind::InvalidData, message)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.details.source()
    }
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
            ErrorKind::IOError => write!(f, "I/O error"),
            ErrorKind::InvalidData => write!(f, "Invalid data"),
            ErrorKind::LibrespotError => write!(f, "Librespot internal error"),
            ErrorKind::ChannelClosed => write!(f, "Channel closed"),
        }
    }
}

impl From<OAuthError> for Error {
    fn from(value: OAuthError) -> Self {
        Self::new(ErrorKind::Unexpected, value).situation("while handling OAuth")
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::new(ErrorKind::IOError, value)
    }
}

impl From<librespot::core::Error> for Error {
    fn from(value: librespot::core::Error) -> Self {
        Self::new(ErrorKind::LibrespotError, value)
    }
}

impl<T> From<SendError<T>> for Error {
    fn from(_: SendError<T>) -> Self {
        // We cannot propagate SendError due to template T.
        // As for now, the error only means one thing anyway, so we can just communicate that manually.
        Self::new(
            ErrorKind::ChannelClosed,
            "Currently read MPSC channel has already been closed",
        )
    }
}
