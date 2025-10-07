mod compat;
mod components;
mod core;
mod error;
mod util;

use crate::components::{auth_wizard, prot_play};
use crate::core::session::Session;
use crate::error::{Error, ErrorKind};
use crate::util::LOG_ENV_NAME;
use iced::widget;
use librespot::oauth::OAuthToken;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug)]
enum Message {
    AuthWizard(auth_wizard::Message),
    ProtPlay(prot_play::Message),
    UpdateSession(Session),
    ChangeToView(ViewName),
    /// Note: I am still trying to find out how to send errors over messages; Until then, use log_error method
    ErrorLogged,
}

#[derive(Debug)]
enum View {
    AuthWizard(auth_wizard::AuthWizard),
    ProtPlay(prot_play::ProtPlay),
}

#[derive(Debug)]
pub enum ViewName {
    AuthWizard,
    ProtPlay,
}

struct App {
    view: View,
    session: Option<Session>,
}

// Default implementation for iced
impl Default for App {
    fn default() -> Self {
        Self {
            view: View::AuthWizard(auth_wizard::AuthWizard::new()),
            session: None,
        }
    }
}

impl App {
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::AuthWizard(sub_message) => {
                if let View::AuthWizard(auth_wizard) = &mut self.view {
                    match auth_wizard.update(sub_message) {
                        auth_wizard::Action::Run(task) => return task.map(Message::AuthWizard),
                        auth_wizard::Action::None => {}
                        auth_wizard::Action::AuthenticateWithToken(token) => {
                            return iced::Task::perform(
                                async move { Session::authenticate_with_access_token(token).await },
                                |result| {
                                    result
                                        .map(Message::UpdateSession)
                                        .unwrap_or_else(Self::log_error)
                                },
                            ).chain(iced::Task::done(Message::ChangeToView(ViewName::ProtPlay)))
                        }
                    }
                }
            }
            Message::ProtPlay(sub_message) => {
                if let View::ProtPlay(prot_play) = &mut self.view {
                    match prot_play.update(sub_message) {
                        prot_play::Action::Run(task) => return task.map(Message::ProtPlay),
                        prot_play::Action::None => {}
                        prot_play::Action::PlayThatOneSong => return self.play_that_one_song(),
                        prot_play::Action::Pause => {}
                        prot_play::Action::Resume => {}
                    }
                }
            }
            Message::ChangeToView(view_name) => {
                self.view = match view_name {
                    ViewName::AuthWizard => View::AuthWizard(auth_wizard::AuthWizard::new()),
                    ViewName::ProtPlay => View::ProtPlay(prot_play::ProtPlay::new()),
                };
            }
            Message::UpdateSession(session) => {
                self.session.replace(session);
            }
            Message::ErrorLogged => {}
        };

        iced::Task::none()
    }

    fn view(&self) -> iced::Element<'_, Message> {
        let frame = match &self.view {
            View::AuthWizard(auth_wizard) => auth_wizard.view().map(Message::AuthWizard),
            View::ProtPlay(prot_play) => prot_play.view().map(Message::ProtPlay),
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

// Implementation for music playback
impl App {
    fn play_that_one_song(&mut self) -> iced::Task<Message> {
        let mut session = match self.session.take() {
            Some(session) => session,
            None => {
                return iced::Task::done(Self::log_error(Error::new(
                    ErrorKind::Unexpected,
                    "Session should exist by now",
                )));
            }
        };
        iced::Task::perform(
            async move {
                session.play_that_one_song().await?;
                Ok(session)
            },
            |r| {
                r.map(Message::UpdateSession)
                    .unwrap_or_else(Self::log_error)
            },
        )
    }
}

fn main() -> Result<(), iced::Error> {
    env_logger::Builder::from_env(env_logger::Env::default().filter_or(LOG_ENV_NAME, "info"))
        .filter_module("wgpu_core", log::LevelFilter::Info)
        .filter_module("naga", log::LevelFilter::Info)
        .filter_module("cosmic_text", log::LevelFilter::Info)
        .init();

    iced::application("Shush", App::update, App::view).run()
}
