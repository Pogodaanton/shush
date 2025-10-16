mod compat;
mod core;
mod error;
mod util;
mod iced_gui;

use crate::util::LOG_ENV_NAME;
use crate::iced_gui::iced_gui_main;

#[cfg(feature = "iced")]
fn main() -> iced::Result {
    env_logger::Builder::from_env(env_logger::Env::default().filter_or(LOG_ENV_NAME, "info"))
        .filter_module("wgpu_core", log::LevelFilter::Info)
        .filter_module("naga", log::LevelFilter::Info)
        .filter_module("cosmic_text", log::LevelFilter::Info)
        .init();
    
    iced_gui_main()
}
