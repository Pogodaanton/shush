use tokio_stream::wrappers::UnboundedReceiverStream;
use librespot::playback::player::PlayerEvent;
use tokio::sync::mpsc::UnboundedSender;
use iced::widget;
use crate::core::config::Config;
use crate::core::session::{Session, SessionRequest};
use crate::error::Error;
use crate::iced_gui::components::{auth_wizard, prot_play};

pub mod components;

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

pub struct App {
    view: View,
    session: Option<Session>,
    session_request_sender: Option<UnboundedSender<SessionRequest>>,
}

// Default implementation for iced_gui
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
    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::AuthWizard(sub_message) => {
                if let View::AuthWizard(auth_wizard) = &mut self.view {
                    match auth_wizard.update(sub_message) {
                        auth_wizard::Action::Run(task) => return task.map(Message::AuthWizard),
                        auth_wizard::Action::None => {}
                        auth_wizard::Action::AuthenticateWithToken(token) => {
                            return iced::Task::future(async move {
                                Session::authenticate_with_access_token(token)
                                    .await
                                    .map(Message::UpdateSessionListener)
                                    .unwrap_or_else(Self::log_error)
                            });
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
                        self.session.as_ref().map(|s| s.rspotify())
                    )),
                };

                // This is a lot of duplicate code and I hate it
                match &self.view {
                    View::ProtPlay(prot_play) => match prot_play.on_mount() {
                        prot_play::Action::Run(task) => return task.map(Message::ProtPlay),
                        prot_play::Action::None => {}
                    },
                    _ => {}
                }
            }
            Message::UpdateSessionListener((
                session,
                player_event_stream,
                session_request_sender,
            )) => {
                self.session_request_sender.replace(session_request_sender);
                self.session.replace(session);

                // Start player event listener
                return iced::Task::batch(vec![
                    iced::Task::done(Message::ChangeToView(ViewName::ProtPlay)),
                    iced::Task::stream(player_event_stream).map(Message::LibrespotEventReceived)
                ]);
            }
            Message::LibrespotEventReceived(player_event) => {
                log::info!("{:?}", &player_event);

                // This is a lot of duplicate code and I hate it
                match &mut self.view {
                    View::AuthWizard(_) => {}
                    View::ProtPlay(prot_play) => match prot_play.librespot_update(player_event) {
                        prot_play::Action::Run(task) => return task.map(Message::ProtPlay),
                        _ => {}
                    },
                }
            }
            Message::ErrorLogged(_) => {}
        };

        iced::Task::none()
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
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

    pub fn load_config() -> iced::Task<Message> {
        iced::Task::future(async move {
            let config_res = Config::load_from_file().await;

            match config_res {
                Ok(config) => match Session::authenticate_with_config(config).await {
                    Ok(tuple) => return Message::UpdateSessionListener(tuple),
                    // We need to somehow send this error to the UI eventually...
                    // Currently, this code throws away the error message just so we can
                    // send the ChangeToView message.
                    Err(err) => Self::log_error(err),
                },
                Err(err) => Self::log_error(err),
            };

            // If all fails, you can always log in again
            Message::ChangeToView(ViewName::AuthWizard)
        })
    }

    fn log_error(err: Error) -> Message {
        log::error!("{:?}", err);
        Message::ErrorLogged(err)
    }
}

pub fn iced_gui_main() -> iced::Result {
    iced::application("Shush", App::update, App::view)
        .run_with(|| (App::default(), App::load_config()))
}