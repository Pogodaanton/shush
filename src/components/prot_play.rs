use crate::core::session::SessionRequest;
use crate::error::{Error, ErrorKind};
use iced::futures::StreamExt;
use iced::widget::text;
use librespot::core::SpotifyId;
use librespot::metadata::audio::AudioItem;
use librespot::playback::player::PlayerEvent;
use rspotify::AuthCodeSpotify;
use rspotify::clients::OAuthClient;
use rspotify::model::{FullTrack, Id, SavedTrack};
use std::ops::RangeInclusive;
use std::time::Instant;
use iced::Theme;
use iced::widget::button::Status;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;

#[derive(Debug)]
pub struct ProtPlay {
    api: Option<AuthCodeSpotify>,
    play_state: PlayState,
    disable_buttons: bool,
    session_request_sender: Option<UnboundedSender<SessionRequest>>,
    seek_ticker_handles: Option<(iced::task::Handle, iced::task::Handle)>,
    audio_item: Option<Box<AudioItem>>,
    seek_range: RangeInclusive<f32>,
    seek_state: f32,
    last_known_seek: f32,
    last_playback_start: Option<Instant>,
    my_tracks: Option<Vec<SavedTrack>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PlayThatOneSong,
    Previous,
    Next,
    Pause,
    Resume,
    Ignore,
    SeekTick(f32),
    MyTracksListLoaded(Vec<SavedTrack>),
    ChooseTrack(FullTrack),
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
    pub fn new(
        session_request_sender: Option<UnboundedSender<SessionRequest>>,
        api: Option<AuthCodeSpotify>,
    ) -> Self {
        Self {
            api,
            play_state: PlayState::default(),
            session_request_sender,
            disable_buttons: false,
            seek_state: 0.0,
            seek_ticker_handles: None,
            last_known_seek: 0.0,
            seek_range: RangeInclusive::new(0.0, 0.0),
            audio_item: None,
            last_playback_start: None,
            my_tracks: None,
        }
    }

    pub fn on_mount(&self) -> Action {
        // TODO: error handling
        Action::Run(iced::Task::future(Self::load_my_songs_list(
            self.api().unwrap().clone(),
        )))
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        let el: iced::Element<_> = match &self.my_tracks {
            None => text!("Empty List").into(),
            Some(vec) => iced::widget::scrollable(iced::widget::column(
                vec.into_iter()
                    .map(|track| {
                        iced::widget::button(iced::widget::text(track.track.name.clone()))
                            .on_press(Message::ChooseTrack(track.track.clone()))
                            .style(iced::widget::button::text)
                            .into()
                    }),
            )).height(iced::Length::Fill).width(iced::Length::Fill).into(),
        };

        iced::widget::column![
            el,
            iced::widget::progress_bar(self.seek_range.clone(), self.seek_state),
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
            Message::SeekTick(seek) => {
                self.seek_state = seek;
            }
            Message::MyTracksListLoaded(vec) => {
                self.my_tracks.replace(vec);
            },
            Message::ChooseTrack(track) => return self.play(track).unwrap_or_else(Self::report_error),
        }

        Action::None
    }

    pub fn librespot_update(&mut self, player_event: PlayerEvent) -> Action {
        match player_event {
            PlayerEvent::Playing { position_ms, .. } => {
                self.disable_buttons = false;
                self.play_state = PlayState::Playing;
                self.last_known_seek = position_ms.clone() as f32;
                self.last_playback_start.replace(Instant::now());

                return self.reset_seek_ticker(position_ms as f32);
            }
            PlayerEvent::Paused { position_ms, .. } => {
                self.disable_buttons = false;
                self.play_state = PlayState::Paused;
                self.last_known_seek = position_ms.clone() as f32;
                self.last_playback_start = None;

                self.stop_seek_ticker();
                self.seek_state = position_ms as f32;
            }
            PlayerEvent::Stopped { .. } => {
                self.disable_buttons = false;
                self.play_state = PlayState::Stopped;
                self.last_known_seek = 0.0;
                self.last_playback_start = None;
                self.seek_range = RangeInclusive::new(0.0, 0.0);

                self.stop_seek_ticker();
                self.seek_state = 0.0;
            }
            PlayerEvent::EndOfTrack { .. } => {
                self.disable_buttons = false;
                self.play_state = PlayState::Stopped;
                self.last_known_seek = 0.0;
                self.last_playback_start = None;
                self.seek_range = RangeInclusive::new(0.0, 0.0);
                self.audio_item = None;

                self.stop_seek_ticker();
                self.seek_state = 0.0;
            }
            PlayerEvent::Seeked { position_ms, .. } => {
                self.disable_buttons = false;
                self.last_known_seek = position_ms as f32;

                if let PlayState::Playing = self.play_state {
                    self.last_playback_start.replace(Instant::now());
                    return self.reset_seek_ticker(position_ms as f32);
                }

                self.seek_state = position_ms as f32;
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
        self.api
            .as_ref()
            .ok_or(Error::new(ErrorKind::Unexpected, "Cannot access api!"))
    }

    fn play_that_one_song(&mut self) -> Result<Action, Error> {
        self.disable_buttons = true;
        let sender = self.session_sender()?;
        sender.send(SessionRequest::PlayThatOneTrack)?;

        log::info!("Playing that one song from prot_play!");

        Ok(Action::None)
    }

    fn play(&mut self, track: FullTrack) -> Result<Action, Error> {
        self.disable_buttons = true;
        let sender = self.session_sender()?;
        let spotify_uri = track.id
            .ok_or_else(|| Error::invalid_data(format!("Track \"{:?}\" does not have an ID. Is it a local file?", track.name)))?
            .uri();
        let spotify_id = SpotifyId::from_uri(&spotify_uri)?;
        sender.send(SessionRequest::Play(spotify_id));

        log::info!("Playing the song \"{:?}\"...", track.name);

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

    /// Brute-forced solution to make the progress bar update dynamically.
    /// I don't know whether this is the desired way to do it... Looks very convoluted and also performance-nagging.
    fn reset_seek_ticker(&mut self, latest_seek_state: f32) -> Action {
        self.stop_seek_ticker();

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let (receiver_task, receiver_handle) =
            iced::Task::stream(UnboundedReceiverStream::from(rx)).abortable();
        let (sender_task, sender_handle) = iced::Task::future(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
            let mut seek = latest_seek_state;

            interval.tick().await;
            loop {
                if tx.is_closed() {
                    return Message::Ignore;
                }

                interval.tick().await;

                seek = seek + 100.0;
                tx.send(Message::SeekTick(seek)).unwrap();
            }
        })
        .abortable();

        self.seek_ticker_handles
            .replace((receiver_handle, sender_handle));

        Action::Run(iced::Task::batch(vec![receiver_task, sender_task]))
    }

    fn stop_seek_ticker(&mut self) {
        if let Some((handle_1, handle_2)) = self.seek_ticker_handles.take() {
            handle_1.abort();
            handle_2.abort();
        }
    }

    async fn load_my_songs_list(api: AuthCodeSpotify) -> Message {
        let saved_tracks = api
            .current_user_saved_tracks(None)
            .filter_map(|res| async { res.ok() })
            .collect::<Vec<_>>()
            .await;

        log::info!("Successfully loaded my tracks list.");
        Message::MyTracksListLoaded(saved_tracks)
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
