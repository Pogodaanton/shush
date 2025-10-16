mod compat;
mod core;
mod error;
mod util;
#[cfg(feature = "iced")]
mod iced_gui;
#[cfg(feature = "gpui")]
mod gpui_gui;

#[cfg(feature = "iced")]
use crate::iced_gui::iced_gui_main;

#[cfg(feature = "gpui")]
use crate::gpui_gui::gpui_gui_main;

use crate::util::LOG_ENV_NAME;

#[cfg(feature = "iced")]
fn main() -> iced::Result {
    setup_logger();
    iced_gui_main()
}

#[cfg(feature = "gpui")]
fn main() {
    setup_logger();
    gpui_gui_main()
}

fn setup_logger() {
    env_logger::Builder::from_env(env_logger::Env::default().filter_or(LOG_ENV_NAME, "info"))
        .filter_module("wgpu_core", log::LevelFilter::Info)
        .filter_module("naga", log::LevelFilter::Info)
        .filter_module("cosmic_text", log::LevelFilter::Info)
        .init();
}