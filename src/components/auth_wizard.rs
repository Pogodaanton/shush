use librespot::core::token::Token;
use librespot::oauth::{OAuthClientBuilder, OAuthToken};
use crate::error::Error;
use crate::util::{ACCESS_SCOPES, CLIENT_ID, SPOTIFY_REDIRECT_URI};

pub struct AuthWizard {
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenLoginInBrowser,
    OAuthSuccessful(OAuthToken),
    OAuthFailure,
}

pub enum Action {
    Run(iced::Task<Message>),
    None
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
                println!("OAuth Token: {token:#?}");
                Action::None
            },
            Message::OAuthFailure => Action::None,
        }
    }
}

impl AuthWizard {
    fn handle_errors(&self, f: Result<iced::Task<Message>, Error>) -> iced::Task<Message> {
        f.unwrap_or_else(|err| {
            // TODO: report error in UI
            log::error!("{}", err);
            iced::Task::none()
        })
    }

    fn setup_spotify_auth(&self) -> iced::Task<Message> {
        iced::Task::future(async {
            let scopes = ACCESS_SCOPES.split(",").map(|s| s.trim()).collect::<Vec<_>>();
            let client = match OAuthClientBuilder::new(CLIENT_ID, SPOTIFY_REDIRECT_URI, scopes)
                .open_in_browser()
                .build()
            {
                Ok(client) => client,
                Err(err) => {
                    log::error!("Error establishing OAuthClient: {}", err);
                    return Message::OAuthFailure;
                }
            };

            let access_token = match client.get_access_token_async().await {
                Ok(token) => {
                    token
                }
                Err(err) => {
                    log::error!("Unable to get OAuthToken: {}", err);
                    return Message::OAuthFailure;
                }
            };

            Message::OAuthSuccessful(access_token)
        })
    }
}