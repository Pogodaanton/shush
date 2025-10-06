use crate::error::Error;
use librespot::discovery::Credentials;
use librespot::oauth::OAuthToken;
use librespot::protocol::authentication::AuthenticationType;
use crate::compat::LibrespotSession;
use crate::core::config::Config;

#[derive(Default, Clone, Debug)]
pub struct Session {
    config: Option<Config>,
    librespot_session: Option<LibrespotSession>
}

impl Session {
    pub async fn authenticate_with_access_token(mut self, token: OAuthToken) -> Result<Self, Error> {
        let librespot_session_config = librespot::core::SessionConfig::default();
        let librespot_session = librespot::core::session::Session::new(librespot_session_config, None);
        let oauth_credentials = Credentials::with_access_token(token.access_token);

        log::info!("Logging in with oauth access token...");
        librespot_session.connect(oauth_credentials, false).await?;
        
        self.librespot_session = Some(librespot_session.into());
        let credentials = self.librespot_session.as_ref().unwrap().get_stored_credentials();
        
        self.config = Config {
            credentials: Some(credentials)
        }.into();
        
        Ok(self)
    }
}