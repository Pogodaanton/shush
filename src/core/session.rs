use crate::core::config::Config;
use crate::error::Error;
use librespot::core::SpotifyId;
use librespot::core::session::Session as LibrespotSession;
use librespot::discovery::Credentials;
use librespot::oauth::OAuthToken;
use librespot::playback::audio_backend;
use librespot::playback::config::{AudioFormat, PlayerConfig};
use librespot::playback::mixer::NoOpVolume;
use librespot::playback::player::{Player, PlayerEvent, PlayerEventChannel};
use librespot::protocol::authentication::AuthenticationType;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;

pub enum SessionRequest {
    PlayThatOneTrack,
    Pause,
    Resume,
    Stop,
    Previous,
    Next,
}

#[derive(Debug)]
pub struct Session {
    config: Option<Config>,
    session_request_handler: JoinHandle<()>,
    librespot_event_handler: JoinHandle<()>,
}

impl Session {
    pub async fn authenticate_with_access_token(
        token: OAuthToken,
    ) -> Result<
        (
            Self,
            UnboundedReceiverStream<PlayerEvent>,
            UnboundedSender<SessionRequest>,
        ),
        Error,
    > {
        let librespot_session_config = librespot::core::SessionConfig::default();
        let librespot_session =
            librespot::core::session::Session::new(librespot_session_config, None);
        let oauth_credentials = Credentials::with_access_token(token.access_token);

        log::info!("Logging in with oauth access token...");
        librespot_session.connect(oauth_credentials, false).await?;

        let credentials = Self::get_stored_credentials(&librespot_session);
        let player = Self::get_new_player(librespot_session);
        let player_event_receiver = player.get_player_event_channel();
        let player_event_receiver_stream = UnboundedReceiverStream::from(player_event_receiver);

        let playback_data: Arc<Mutex<PlaybackData>> = Arc::new(Mutex::new(PlaybackData::default()));
        let (session_request_s, session_request_r) = mpsc::unbounded_channel::<SessionRequest>();

        let librespot_event_handler =
            tokio::spawn(Self::handle_librespot_events(player.get_player_event_channel(), playback_data.clone()));
        let session_request_handler =
            tokio::spawn(Self::handle_session_requests(session_request_r, player, playback_data));

        Ok((
            Self {
                config: Config {
                    credentials: Some(credentials),
                }.into(),
                session_request_handler,
                librespot_event_handler,
            },
            player_event_receiver_stream,
            session_request_s,
        ))
    }

    fn get_stored_credentials(session: &LibrespotSession) -> Credentials {
        Credentials {
            username: session.username().into(),
            auth_type: AuthenticationType::AUTHENTICATION_STORED_SPOTIFY_CREDENTIALS,
            auth_data: session.auth_data(),
        }
    }

    pub fn get_new_player(session: LibrespotSession) -> Arc<Player> {
        let player_config = PlayerConfig::default();
        let audio_format = AudioFormat::default();
        let backend = audio_backend::find(None).unwrap();

        Player::new(player_config, session, Box::new(NoOpVolume), move || {
            backend(None, audio_format)
        })
    }

    async fn handle_session_requests(
        mut rx: UnboundedReceiver<SessionRequest>,
        librespot_player: Arc<Player>,
        playback_data: Arc<Mutex<PlaybackData>>,
    ) {
        let mut player_man = PlayerMan::new(librespot_player, playback_data.clone());

        while let Some(req) = rx.recv().await {
            match req {
                SessionRequest::PlayThatOneTrack => player_man.play_that_one_track().await,
                SessionRequest::Pause => player_man.pause().await,
                SessionRequest::Resume => player_man.play().await,
                SessionRequest::Stop => player_man.stop().await,
                SessionRequest::Previous => player_man.previous().await,
                SessionRequest::Next => {}
            };
        }
    }

    /// Handler for events stemming from librespot.
    /// Most of the data received here is used to re-align internal data shared with the
    /// session_request handler so that playback is not fully approximated.
    /// That said, if it gets little to no use, this will be stripped out.
    async fn handle_librespot_events(
        mut rx: PlayerEventChannel,
        playback_data: Arc<Mutex<PlaybackData>>,
    ) {
        while let Some(event) = rx.recv().await {
            match event {
                PlayerEvent::Playing { play_request_id: _, position_ms, track_id: _ } => {
                    let mut playback_data = playback_data.lock().await;
                    playback_data.set_seek(position_ms as u64);
                    playback_data.set_playing();
                }
                PlayerEvent::Paused { position_ms, .. } => {
                    let mut playback_data = playback_data.lock().await;
                    playback_data.set_paused();
                    playback_data.set_seek(position_ms as u64);
                }
                PlayerEvent::Stopped { .. } => {
                    let mut playback_data = playback_data.lock().await;
                    playback_data.set_paused();
                    playback_data.set_seek(0);
                }
                PlayerEvent::Seeked { position_ms, .. } => {
                    playback_data.lock().await.set_seek(position_ms as u64);
                }
                PlayerEvent::EndOfTrack { .. } => {
                    // not sure if it always means paused tbh, needs further testing
                    let mut playback_data = playback_data.lock().await;
                    playback_data.set_paused();
                    playback_data.set_seek(0);
                }
                _ => {}
            }
        }
    }
}

struct PlayerMan {
    player: Arc<Player>,
    playback_data: Arc<Mutex<PlaybackData>>,
}

impl PlayerMan {
    pub fn new(player: Arc<Player>, playback_data: Arc<Mutex<PlaybackData>>) -> Self {
        Self {
            player,
            playback_data,
        }
    }

    pub async fn load_and_play(&self, track: SpotifyId) {
        self.player.load(track, true, 0);
    }

    pub async fn play(&self) {
        self.player.play();
    }

    async fn play_that_one_track(&self) {
        let track = SpotifyId::from_uri("spotify:track:2dQNBDqHumZS3tfzjrfUHi").unwrap();

        self.load_and_play(track).await;

        log::info!("Playing...");
    }

    pub async fn pause(&self) {
        self.player.pause();
    }

    pub async fn stop(&self) {
        self.player.stop();
    }

    pub async fn previous(&mut self) {
        let playback_data = self.playback_data.lock().await;
        if playback_data.current_seek_position() > 2000 {
            self.player.seek(0);
        }
    }
}

#[derive(Debug, Default)]
struct PlaybackData {
    is_playing: bool,
    last_known_seek: u64,
    last_playback_start: Option<Instant>,
}

impl PlaybackData {
    pub fn current_seek_position(&self) -> u64 {
        let progress_since_last_playback_start = self
            .last_playback_start
            .map(|last_playback_start| last_playback_start.elapsed().as_millis() as u64)
            .unwrap_or(0);

        self.last_known_seek + progress_since_last_playback_start
    }

    pub fn set_seek(&mut self, seek_position: u64) {
        self.last_known_seek = seek_position;
        self.last_playback_start = self.is_playing.then(Instant::now);
    }

    pub fn set_paused(&mut self) {
        self.is_playing = false;
        self.last_playback_start = None;
    }

    pub fn set_playing(&mut self) {
        self.is_playing = true;
        self.last_playback_start = Some(Instant::now());
    }
}
