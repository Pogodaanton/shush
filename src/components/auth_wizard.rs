use librespot::oauth::OAuthClientBuilder;
use crate::error::Error;
use crate::util::{ACCESS_SCOPES, CLIENT_ID, SPOTIFY_REDIRECT_URI};

pub struct AuthWizard {
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenLoginInBrowser,
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
            Message::OpenLoginInBrowser => Action::Run(self.handle_errors(self.setup_spotify_auth()))
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

    fn setup_spotify_auth(&self, ) -> Result<iced::Task<Message>, Error> {
        let scopes = ACCESS_SCOPES.split(",").map(|s| s.trim()).collect::<Vec<_>>();
        let client = OAuthClientBuilder::new(CLIENT_ID, SPOTIFY_REDIRECT_URI, scopes)
            .open_in_browser()
            .build()?;

        let refresh_token = match client.get_access_token() {
            Ok(token) => {
                println!("OAuth Token: {token:#?}");
                token.refresh_token
            }
            Err(err) => {
                return Err(Error::from(err).situation("Unable to get OAuth Token"));
            }
        };

        match client.refresh_token(&refresh_token) {
            Ok(token) => println!("New refreshed OAuth Token: {token:#?}"),
            Err(err) => return Err(Error::from(err).situation("Unable to get refreshed OAuth Token")),
        }

        Ok(iced::Task::none())
    }
}