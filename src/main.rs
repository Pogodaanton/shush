mod components;
mod util;
mod error;

use iced::widget;
use crate::components::auth_wizard;

#[derive(Debug, Clone)]
enum Message {
    AuthWizard(auth_wizard::Message)
}

enum View {
    AuthWizard(auth_wizard::AuthWizard)
}

struct App {
    view: View
}

impl App {
    fn new() -> Self {
        Self {
            view: View::AuthWizard(auth_wizard::AuthWizard::new()),
        }
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::AuthWizard(sub_message) => {
                if let View::AuthWizard(auth_wizard) = &mut self.view {
                    match auth_wizard.update(sub_message) {
                        auth_wizard::Action::Run(task) => return task.map(Message::AuthWizard),
                        auth_wizard::Action::None => {},
                    }
                }
            }
        };

        iced::Task::none()
    }

    fn view(&self) -> iced::Element<'_, Message> {
        let frame = match &self.view {
            View::AuthWizard(auth_wizard) => auth_wizard.view().map(Message::AuthWizard),
        };

        widget::container(frame)
            .center_x(iced::Length::Fill)
            .center_y(iced::Length::Fill)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into()
    }
}

fn main() -> Result<(), iced::Error> {
    iced::application("Shush", App::update, App::view)
        .run_with(|| (App::new(), iced::Task::none()))
}
