use crate::core::session::Session;

#[derive(Debug, Clone)]
pub struct ProtPlay {
    play_state: PlayState,
}

#[derive(Debug, Clone)]
pub enum Message {
    PlayStateChanged(PlayState),
    PlayThatOneSong,
    Pause,
    Resume,
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
    PlayThatOneSong,
    Pause,
    Resume,
}

impl ProtPlay {
    pub fn new() -> Self {
        Self {
            play_state: PlayState::default(),
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        iced::widget::row![
            iced::widget::button(match self.play_state {
                PlayState::Stopped => "Play",
                PlayState::Playing => "Pause",
                PlayState::Paused => "Resume",
            }).on_press(match self.play_state {
                PlayState::Stopped => Message::PlayThatOneSong,
                PlayState::Playing => Message::Pause,
                PlayState::Paused => Message::Resume,
            })
        ].into()
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::PlayStateChanged(new_state) => { self.play_state = new_state; }
            Message::PlayThatOneSong => return Action::PlayThatOneSong,
            Message::Pause => return Action::Pause,
            Message::Resume => return Action::Resume,
        }

        Action::None
    }


}