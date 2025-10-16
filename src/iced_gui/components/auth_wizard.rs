use crate::error::Error;
use crate::util::{ACCESS_SCOPES, CLIENT_ID, SPOTIFY_REDIRECT_URI};
use librespot::oauth::{OAuthClientBuilder, OAuthToken};

#[derive(Debug, Clone)]
pub struct AuthWizard {}

#[derive(Debug, Clone)]
pub enum Message {
    OpenLoginInBrowser,
    OAuthSuccessful(OAuthToken),
    Ignore,
}

pub enum Action {
    /// Execute task and propagate message to AuthWizard
    Run(iced::Task<Message>),
    /// Start session with given OAuthToken
    AuthenticateWithToken(OAuthToken),
    None,
}

impl AuthWizard {
    pub fn new() -> Self {
        Self {}
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        iced::widget::column![
            iced::widget::text!("Welcome to shush"),
            iced::widget::text!("To continue, please log in with your Spotify Premium account. Authentication happens on official servers, so we don't ever see your password."),
            iced::widget::button("Log in with Spotify").on_press(Message::OpenLoginInBrowser)
        ].into()
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::OpenLoginInBrowser => Action::Run(self.setup_spotify_auth()),
            Message::OAuthSuccessful(token) => {
                log::debug!("OAuth Token: {token:#?}");
                Action::AuthenticateWithToken(token)
            }
            Message::Ignore => Action::None,
        }
    }
}

impl AuthWizard {
    fn handle_error(err: Error) -> Message {
        log::error!("{}", err);
        Message::Ignore
    }

    fn setup_spotify_auth(&self) -> iced::Task<Message> {
        iced::Task::perform(
            async {
                let scopes = ACCESS_SCOPES
                    .split(",")
                    .map(|s| s.trim())
                    .collect::<Vec<_>>();
                
                let client = OAuthClientBuilder::new(CLIENT_ID, SPOTIFY_REDIRECT_URI, scopes)
                    .open_in_browser()
                    .build()?;

                let access_token = client.get_access_token_async().await?;

                Ok(Message::OAuthSuccessful(access_token))
            },
            |result| result.unwrap_or_else(Self::handle_error),
        )
    }
}
