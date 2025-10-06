use std::fmt::Debug;
use librespot::core::Session;
use librespot::discovery::Credentials;
use librespot::protocol::authentication::AuthenticationType;

/// Wrapper for Librespot's session to handle Debug calls
#[derive(Clone)]
pub struct LibrespotSession(Session);

impl Debug for LibrespotSession {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("LibrespotSession")
            .field("username", &self.0.username())
            .field("is_invalid", &self.0.is_invalid())
            .finish()
    }
}

impl From<Session> for LibrespotSession {
    fn from(session: Session) -> Self {
        Self(session)
    }
}

impl LibrespotSession {
    pub fn get_stored_credentials(&self) -> Credentials {
        Credentials {
            username: self.0.username().into(),
            auth_type: AuthenticationType::AUTHENTICATION_STORED_SPOTIFY_CREDENTIALS,
            auth_data: self.0.auth_data(),
        }
    }
}