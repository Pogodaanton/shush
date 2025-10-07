use librespot::core::SpotifyId;
use crate::compat::{LibrespotPlayer};
use crate::core::config::Config;
use crate::error::{Error, ErrorKind};
use librespot::discovery::Credentials;
use librespot::oauth::OAuthToken;
use librespot::playback::audio_backend;
use librespot::playback::config::{AudioFormat, PlayerConfig};
use librespot::playback::mixer::NoOpVolume;
use librespot::protocol::authentication::AuthenticationType;
use librespot::core::session::Session as LibrespotSession;

#[derive(Default, Debug)]
pub struct Session {
    config: Option<Config>,
    librespot_player: Option<LibrespotPlayer>,
}

impl Session {
    pub async fn authenticate_with_access_token(
        token: OAuthToken,
    ) -> Result<Self, Error> {
        let librespot_session_config = librespot::core::SessionConfig::default();
        let librespot_session =
            librespot::core::session::Session::new(librespot_session_config, None);
        let oauth_credentials = Credentials::with_access_token(token.access_token);

        log::info!("Logging in with oauth access token...");
        librespot_session.connect(oauth_credentials, false).await?;

        let credentials = Self::get_stored_credentials(&librespot_session);
        let player = Self::get_new_player(librespot_session);

        Ok(Self {
            config: Config {
                credentials: Some(credentials),
            }.into(),
            librespot_player: Some(player),
        })
    }

    fn get_stored_credentials(session: &LibrespotSession) -> Credentials {
        Credentials {
            username: session.username().into(),
            auth_type: AuthenticationType::AUTHENTICATION_STORED_SPOTIFY_CREDENTIALS,
            auth_data: session.auth_data(),
        }
    }

    pub fn get_new_player(session: LibrespotSession) -> LibrespotPlayer {
        let player_config = PlayerConfig::default();
        let audio_format = AudioFormat::default();
        let backend = audio_backend::find(None).unwrap();

        let player = librespot::playback::player::Player::new(
            player_config,
            session,
            Box::new(NoOpVolume),
            move || backend(None, audio_format),
        );

        player.into()
    }

    pub async fn play_that_one_song(&mut self) -> Result<(), Error> {
        let mut player = match self.librespot_player.as_mut() {
            None => {
                return Err(Error::new(ErrorKind::Unexpected, "Cannot find librespot player instance."));
            },
            Some(mut player) => player
        }.get();
        let track = SpotifyId::from_uri("spotify:track:21Plp9v154VpwD9uttU4NS")?;

        player.load(track, true, 0);

        player.play();

        log::info!("Playing...");

        player.await_end_of_track().await;

        log::info!("Done");

        Ok(())
    }
}
