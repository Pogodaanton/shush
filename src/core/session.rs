use crate::core::config::Config;
use crate::error::Error;
use librespot::core::SpotifyId;
use librespot::core::session::Session as LibrespotSession;
use librespot::discovery::Credentials;
use librespot::oauth::OAuthToken;
use librespot::playback::audio_backend;
use librespot::playback::config::{AudioFormat, PlayerConfig};
use librespot::playback::mixer::NoOpVolume;
use librespot::playback::player::{Player, PlayerEvent};
use librespot::protocol::authentication::AuthenticationType;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;

pub enum SessionRequest {
    PlayThatOneTrack,
    Pause,
    Resume,
    Stop,
}

#[derive(Debug)]
pub struct Session {
    config: Option<Config>,
    session_request_handler: JoinHandle<()>,
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

        let (session_request_sender, rx) = mpsc::unbounded_channel::<SessionRequest>();
        let session_request_handler = tokio::spawn(Self::handle_session_requests(rx, player));

        Ok((
            Self {
                config: Config {
                    credentials: Some(credentials),
                }
                .into(),
                session_request_handler,
            },
            player_event_receiver_stream,
            session_request_sender,
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
        player: Arc<Player>,
    ) {
        while let Some(req) = rx.recv().await {
            match req {
                SessionRequest::PlayThatOneTrack => {
                    Self::async_play_that_one_song(&player).await.unwrap()
                }
                SessionRequest::Pause => player.pause(),
                SessionRequest::Resume => player.play(),
                SessionRequest::Stop => player.stop(),
            }
        }
    }

    async fn async_play_that_one_song(player: &Player) -> Result<(), Error> {
        let track = SpotifyId::from_uri("spotify:track:2dQNBDqHumZS3tfzjrfUHi")?;

        player.load(track, true, 0);

        log::info!("Playing...");

        Ok(())
    }
}
