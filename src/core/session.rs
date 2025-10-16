use std::ops::Add;
use crate::compat::LibrespotSession;
use crate::core::config::Config;
use crate::error::{Error, ErrorKind};
use librespot::core::SpotifyId;
use librespot::discovery::Credentials;
use librespot::oauth::OAuthToken;
use librespot::playback::audio_backend;
use librespot::playback::config::{AudioFormat, PlayerConfig};
use librespot::playback::mixer::NoOpVolume;
use librespot::playback::player::{Player, PlayerEvent, PlayerEventChannel};
use librespot::protocol::authentication::AuthenticationType;
use rspotify::AuthCodeSpotify;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;

pub enum SessionRequest {
    PlayThatOneTrack,
    Play(SpotifyId),
    Pause,
    Resume,
    Stop,
    Previous,
    Next,
}

#[derive(Debug)]
pub struct Session {
    config: Config,
    librespot_session: LibrespotSession,
    rspotify: AuthCodeSpotify,
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
        let oauth_credentials = Credentials::with_access_token(token.access_token);
        Self::authenticate_with_credentials(oauth_credentials, Config::default()).await
    }

    pub async fn authenticate_with_config(
        config: Config,
    ) -> Result<
        (
            Self,
            UnboundedReceiverStream<PlayerEvent>,
            UnboundedSender<SessionRequest>,
        ),
        Error,
    > {
        let credentials = config.credentials.clone().ok_or(Error::auth_failed(
            "Credentials missing in config, please log in again.",
        ))?;
        Self::authenticate_with_credentials(credentials, config).await
    }

    async fn authenticate_with_credentials(
        credentials: Credentials,
        mut config: Config,
    ) -> Result<
        (
            Self,
            UnboundedReceiverStream<PlayerEvent>,
            UnboundedSender<SessionRequest>,
        ),
        Error,
    > {
        let librespot_session_config = librespot::core::SessionConfig::default();
        let librespot_session = LibrespotSession::new(librespot_session_config, None);

        log::info!(
            "Logging in with credential type {:?}...",
            credentials.auth_type
        );

        librespot_session.get_ref().connect(credentials, false).await?;

        let stored_credentials = librespot_session.get_stored_credentials();
        config.set_stored_credentials(stored_credentials).await?;

        let rspotify = Self::get_new_rspotify(&librespot_session).await?;

        Ok(Self::new(librespot_session, rspotify, config))
    }

    fn new(
        librespot_session: LibrespotSession,
        rspotify: AuthCodeSpotify,
        config: Config,
    ) -> (
        Self,
        UnboundedReceiverStream<PlayerEvent>,
        UnboundedSender<SessionRequest>,
    ) {
        let player = Self::get_new_player(librespot_session.clone());
        let player_event_receiver = player.get_player_event_channel();
        let player_event_receiver_stream = UnboundedReceiverStream::from(player_event_receiver);

        let playback_data: Arc<Mutex<PlaybackData>> = Arc::new(Mutex::new(PlaybackData::default()));
        let (session_request_s, session_request_r) = mpsc::unbounded_channel::<SessionRequest>();

        let librespot_event_handler = tokio::spawn(Self::handle_librespot_events(
            player.get_player_event_channel(),
            playback_data.clone(),
        ));
        let session_request_handler = tokio::spawn(Self::handle_session_requests(
            session_request_r,
            player,
            playback_data,
        ));

        (
            Self {
                config,
                librespot_session,
                rspotify,
                session_request_handler,
                librespot_event_handler,
            },
            player_event_receiver_stream,
            session_request_s,
        )
    }

    pub fn get_new_player(session: LibrespotSession) -> Arc<Player> {
        let player_config = PlayerConfig::default();
        let audio_format = AudioFormat::default();
        let backend = audio_backend::find(None).unwrap();

        Player::new(player_config, session.get_inner(), Box::new(NoOpVolume), move || {
            backend(None, audio_format)
        })
    }

    pub async fn get_new_rspotify(session: &LibrespotSession) -> Result<AuthCodeSpotify, Error> {
        let login5_token = session.get_ref().login5().auth_token().await?;
        let expires_in = chrono::TimeDelta::from_std(login5_token.expires_in).unwrap_or_default();
        let expired_at = chrono::Utc::now().add(expires_in);
        let rspotify_token = rspotify::Token {
            access_token: login5_token.access_token,
            refresh_token: None,
            expires_in,
            // While this is technically untrue because we created the token a bit earlier, it's good enough for now.
            expires_at: Some(expired_at),
            scopes: login5_token.scopes.into_iter().collect(),
        };

        Ok(AuthCodeSpotify::from_token(rspotify_token))
    }

    async fn handle_session_requests(
        mut rx: UnboundedReceiver<SessionRequest>,
        librespot_player: Arc<Player>,
        playback_data: Arc<Mutex<PlaybackData>>,
    ) {
        let mut player_man = PlayerMan::new(librespot_player, playback_data.clone());

        while let Some(req) = rx.recv().await {
            match req {
                SessionRequest::Play(id) => player_man.load_and_play(id).await,
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
                PlayerEvent::Playing {
                    play_request_id: _,
                    position_ms,
                    track_id: _,
                } => {
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

    pub fn rspotify(&self) -> AuthCodeSpotify {
        self.rspotify.clone()
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
        log::info!("Playing track {:?}...", track);
        self.player.load(track, true, 0);
    }

    pub async fn play(&self) {
        self.player.play();
    }

    async fn play_that_one_track(&self) {
        let track = SpotifyId::from_uri("spotify:track:2dQNBDqHumZS3tfzjrfUHi").unwrap();
        self.load_and_play(track).await;
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
