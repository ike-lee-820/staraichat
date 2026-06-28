#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> iced::Result {
    ai_chat::run_app()
}
