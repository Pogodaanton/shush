mod compat;
mod components;
mod core;
mod error;
mod util;

use crate::components::{auth_wizard, prot_play};
use crate::core::session::{Session, SessionRequest};
use crate::error::{Error};
use crate::util::LOG_ENV_NAME;
use iced::widget;
use librespot::playback::player::PlayerEvent;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[derive(Debug)]
enum Message {
    AuthWizard(auth_wizard::Message),
    ProtPlay(prot_play::Message),
    UpdateSessionListener(
        (
            Session,
            UnboundedReceiverStream<PlayerEvent>,
            UnboundedSender<SessionRequest>,
        ),
    ),
    LibrespotEventReceived(PlayerEvent),
    ChangeToView(ViewName),
    ErrorLogged(Error),
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
    session_request_sender: Option<UnboundedSender<SessionRequest>>,
}

// Default implementation for iced
impl Default for App {
    fn default() -> Self {
        Self {
            view: View::AuthWizard(auth_wizard::AuthWizard::new()),
            session: None,
            session_request_sender: None,
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
                                        .map(Message::UpdateSessionListener)
                                        .unwrap_or_else(Self::log_error)
                                },
                            )
                            .chain(iced::Task::done(Message::ChangeToView(ViewName::ProtPlay)));
                        }
                    }
                }
            }
            Message::ProtPlay(sub_message) => {
                if let View::ProtPlay(prot_play) = &mut self.view {
                    match prot_play.update(sub_message) {
                        prot_play::Action::Run(task) => return task.map(Message::ProtPlay),
                        prot_play::Action::None => {}
                    }
                }
            }
            Message::ChangeToView(view_name) => {
                self.view = match view_name {
                    ViewName::AuthWizard => View::AuthWizard(auth_wizard::AuthWizard::new()),
                    ViewName::ProtPlay => View::ProtPlay(prot_play::ProtPlay::new(
                        self.session_request_sender.clone(),
                    )),
                };
            }
            Message::UpdateSessionListener((
                session,
                player_event_stream,
                session_request_sender,
            )) => {
                self.session_request_sender.replace(session_request_sender);
                self.session.replace(session);

                // Start player event listener
                return iced::Task::stream(player_event_stream)
                    .map(Message::LibrespotEventReceived);
            }
            Message::LibrespotEventReceived(player_event) => {
                log::info!("{:?}", &player_event);
                match &mut self.view {
                    View::AuthWizard(_) => {}
                    View::ProtPlay(prot_play) => prot_play.librespot_update(player_event),
                }
            }
            Message::ErrorLogged(_) => {}
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
        Message::ErrorLogged(err)
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
