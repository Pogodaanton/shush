mod components;
mod util;
mod error;
mod core;
mod compat;

use iced::widget;
use librespot::oauth::OAuthToken;
use crate::components::auth_wizard;
use crate::core::session::Session;
use crate::error::Error;
use crate::util::{LOG_ENV_NAME};

#[derive(Debug, Clone)]
enum Message {
    AuthWizard(auth_wizard::Message),
    UpdateSession(Session),
    ChangeToView(View),
    /// Note: I am still trying to find out how to send errors over messages; Until then, use log_error method
    ErrorLogged,
}

#[derive(Debug, Clone)]
enum View {
    AuthWizard(auth_wizard::AuthWizard)
}

struct App {
    view: View,
    session: Session,
}

impl App {
    fn new() -> Self {
        Self {
            view: View::AuthWizard(auth_wizard::AuthWizard::new()),
            session: Session::default(),
        }
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::AuthWizard(sub_message) => {
                if let View::AuthWizard(auth_wizard) = &mut self.view {
                    match auth_wizard.update(sub_message) {
                        auth_wizard::Action::Run(task) => return task.map(Message::AuthWizard),
                        auth_wizard::Action::None => {},
                        auth_wizard::Action::AuthenticateWithToken(token) => {
                            let mut session = self.session.clone();
                            return iced::Task::perform(
                                async move { session.authenticate_with_access_token(token).await },
                                |result| { result.map(Message::UpdateSession).unwrap_or_else(Self::log_error) }
                            )
                        }
                    }
                }
            },
            Message::ChangeToView(view) => {},
            Message::UpdateSession(session) => {
                self.session = session;
            },
            Message::ErrorLogged => {},
        };

        iced::Task::none()
    }

    fn view(&self) -> iced::Element<'_, Message> {
        let frame = match &self.view {
            View::AuthWizard(auth_wizard) => auth_wizard.view().map(Message::AuthWizard),
        };

        widget::container(frame)
            .center_x(iced::Length::Fill)
            .center_y(iced::Length::Fill)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into()
    }

    fn log_error(err: Error) -> Message {
        log::error!("{:?}", err);
        Message::ErrorLogged
    }
}

fn main() -> Result<(), iced::Error> {
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .filter_or(LOG_ENV_NAME,"info")
    ).init();

    iced::application("Shush", App::update, App::view)
        .run_with(|| (App::new(), iced::Task::none()))
}
