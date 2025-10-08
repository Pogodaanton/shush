use crate::core::session::SessionRequest;
use crate::error::{Error, ErrorKind};
use librespot::playback::player::PlayerEvent;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct ProtPlay {
    play_state: PlayState,
    disable_play_button: bool,
    session_request_sender: Option<UnboundedSender<SessionRequest>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PlayThatOneSong,
    Pause,
    Resume,
    Ignore,
}

#[derive(Default, Debug, Clone)]
enum PlayState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

pub enum Action {
    /// Execute task and propagate message to ProtPlay
    Run(iced::Task<Message>),
    None,
}

impl ProtPlay {
    pub fn new(session_request_sender: Option<UnboundedSender<SessionRequest>>) -> Self {
        Self {
            play_state: PlayState::default(),
            session_request_sender,
            disable_play_button: false,
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        iced::widget::row![
            iced::widget::button(match self.play_state {
                PlayState::Stopped => "Play",
                PlayState::Playing => "Pause",
                PlayState::Paused => "Resume",
            })
            .on_press({
                if self.disable_play_button {
                    Message::Ignore
                } else {
                    match self.play_state {
                        PlayState::Stopped => Message::PlayThatOneSong,
                        PlayState::Playing => Message::Pause,
                        PlayState::Paused => Message::Resume,
                    }
                }
            })
        ]
        .into()
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::PlayThatOneSong => {
                return self.play_that_one_song().unwrap_or_else(Self::report_error);
            }
            Message::Pause => return self.pause().unwrap_or_else(Self::report_error),
            Message::Resume => return self.resume().unwrap_or_else(Self::report_error),
            Message::Ignore => {}
        }

        Action::None
    }

    pub fn librespot_update(&mut self, player_event: PlayerEvent) {
        match player_event {
            PlayerEvent::Playing { .. } => {
                self.disable_play_button = false;
                self.play_state = PlayState::Playing;
            }
            PlayerEvent::Paused { .. } => {
                self.disable_play_button = false;
                self.play_state = PlayState::Paused;
            }
            PlayerEvent::Stopped { .. } => {
                self.disable_play_button = false;
                self.play_state = PlayState::Stopped;
            }
            PlayerEvent::EndOfTrack { .. } => {
                self.disable_play_button = false;
                self.play_state = PlayState::Stopped;
            }
            _ => {}
        }
    }

    fn session_sender(&self) -> Result<&UnboundedSender<SessionRequest>, Error> {
        self.session_request_sender.as_ref().ok_or(Error::new(
            ErrorKind::Unexpected,
            "Cannot access session_request_sender!",
        ))
    }

    fn play_that_one_song(&mut self) -> Result<Action, Error> {
        self.disable_play_button = true;
        let sender = self.session_sender()?;
        sender.send(SessionRequest::PlayThatOneTrack)?;

        log::info!("Playing that one song from prot_play!");

        Ok(Action::None)
    }

    fn pause(&mut self) -> Result<Action, Error> {
        self.disable_play_button = true;
        let sender = self.session_sender()?;
        sender.send(SessionRequest::Pause)?;

        log::info!("Pausing current song.");

        Ok(Action::None)
    }

    fn resume(&mut self) -> Result<Action, Error> {
        self.disable_play_button = true;
        let sender = self.session_sender()?;
        sender.send(SessionRequest::Resume)?;

        log::info!("Resuming current song.");

        Ok(Action::None)
    }

    fn report_error(err: Error) -> Action {
        log::error!("{}", err);
        Action::None
    }
}
