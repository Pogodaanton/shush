use crate::core::session::SessionRequest;
use crate::error::{Error, ErrorKind};
use librespot::metadata::audio::AudioItem;
use librespot::playback::player::PlayerEvent;
use std::ops::{RangeInclusive};
use std::time::Instant;
use librespot::core::SpotifyId;
use rspotify::AuthCodeSpotify;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct ProtPlay {
    api: Option<AuthCodeSpotify>,
    play_state: PlayState,
    disable_buttons: bool,
    session_request_sender: Option<UnboundedSender<SessionRequest>>,
    audio_item: Option<Box<AudioItem>>,
    seek_range: RangeInclusive<f32>,
    last_known_seek: f32,
    last_playback_start: Option<Instant>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PlayThatOneSong,
    Previous,
    Next,
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
    pub fn new(session_request_sender: Option<UnboundedSender<SessionRequest>>, api: Option<AuthCodeSpotify>) -> Self {
        Self {
            api,
            play_state: PlayState::default(),
            session_request_sender,
            disable_buttons: false,
            last_known_seek: 0.0,
            seek_range: RangeInclusive::new(0.0,0.0),
            audio_item: None,
            last_playback_start: None,
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        iced::widget::column![
            iced::widget::scrollable(iced::widget::column![
                
            ]),
            iced::widget::progress_bar(self.seek_range.clone(), self.get_current_seek()),
            iced::widget::row![
                iced::widget::button("prev").on_press(Message::Previous),
                iced::widget::button(match self.play_state {
                    PlayState::Stopped => "Play",
                    PlayState::Playing => "Pause",
                    PlayState::Paused => "Resume",
                })
                .on_press({
                    if self.disable_buttons {
                        Message::Ignore
                    } else {
                        match self.play_state {
                            PlayState::Stopped => Message::PlayThatOneSong,
                            PlayState::Playing => Message::Pause,
                            PlayState::Paused => Message::Resume,
                        }
                    }
                }),
                iced::widget::button("next").on_press(Message::Next),
            ]
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
            Message::Previous => return self.previous().unwrap_or_else(Self::report_error),
            Message::Next => return self.next().unwrap_or_else(Self::report_error),
        }

        Action::None
    }

    pub fn librespot_update(&mut self, player_event: PlayerEvent) -> Action {
        match player_event {
            PlayerEvent::Playing { position_ms, .. } => {
                self.disable_buttons = false;
                self.play_state = PlayState::Playing;
                self.last_known_seek = position_ms as f32;
                self.last_playback_start.replace(Instant::now());
            }
            PlayerEvent::Paused { position_ms, .. } => {
                self.disable_buttons = false;
                self.play_state = PlayState::Paused;
                self.last_known_seek = position_ms as f32;
                self.last_playback_start = None;
            }
            PlayerEvent::Stopped { .. } => {
                self.disable_buttons = false;
                self.play_state = PlayState::Stopped;
                self.last_known_seek = 0.0;
                self.last_playback_start = None;
                self.seek_range = RangeInclusive::new(0.0, 0.0);
            }
            PlayerEvent::EndOfTrack { .. } => {
                self.disable_buttons = false;
                self.play_state = PlayState::Stopped;
                self.last_known_seek = 0.0;
                self.last_playback_start = None;
                self.seek_range = RangeInclusive::new(0.0, 0.0);
                self.audio_item = None;
            }
            PlayerEvent::Seeked { position_ms, .. } => {
                self.disable_buttons = false;
                self.last_known_seek = position_ms as f32;

                if let PlayState::Playing = self.play_state {
                    self.last_playback_start.replace(Instant::now());
                }
            }
            PlayerEvent::TrackChanged { audio_item } => {
                self.seek_range = RangeInclusive::new(0.0, audio_item.duration_ms as f32);
                self.audio_item.replace(audio_item);
            }
            _ => {}
        }
        
        Action::None
    }

    fn session_sender(&self) -> Result<&UnboundedSender<SessionRequest>, Error> {
        self.session_request_sender.as_ref().ok_or(Error::new(
            ErrorKind::Unexpected,
            "Cannot access session_request_sender!",
        ))
    }

    fn api(&self) -> Result<&AuthCodeSpotify, Error> {
        self.api.as_ref().ok_or(Error::new(
            ErrorKind::Unexpected,
            "Cannot access api!",
        ))
    }

    fn play_that_one_song(&mut self) -> Result<Action, Error> {
        self.disable_buttons = true;
        let sender = self.session_sender()?;
        sender.send(SessionRequest::PlayThatOneTrack)?;

        log::info!("Playing that one song from prot_play!");

        Ok(Action::None)
    }

    fn pause(&mut self) -> Result<Action, Error> {
        self.disable_buttons = true;
        let sender = self.session_sender()?;
        sender.send(SessionRequest::Pause)?;

        log::info!("Pausing current song.");

        Ok(Action::None)
    }

    fn resume(&mut self) -> Result<Action, Error> {
        self.disable_buttons = true;
        let sender = self.session_sender()?;
        sender.send(SessionRequest::Resume)?;

        log::info!("Resuming current song.");

        Ok(Action::None)
    }

    fn previous(&mut self) -> Result<Action, Error> {
        self.disable_buttons = true;
        let sender = self.session_sender()?;
        sender.send(SessionRequest::Previous)?;

        log::info!("Requesting previous song or song beginning.");

        Ok(Action::None)
    }

    fn next(&mut self) -> Result<Action, Error> {
        self.disable_buttons = true;
        let sender = self.session_sender()?;
        sender.send(SessionRequest::Next)?;

        log::info!("Requesting next song.");

        Ok(Action::None)
    }

    fn get_current_seek(&self) -> f32 {
        let progress_since_last_playback_start =
            self.last_playback_start.map_or(0.0, |last_playback_start| {
                last_playback_start.elapsed().as_millis() as f32
            });

        self.last_known_seek + progress_since_last_playback_start
    }

    fn report_error(err: Error) -> Action {
        log::error!("{}", err);
        Action::None
    }
}
