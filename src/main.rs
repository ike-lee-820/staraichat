#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result {
    ai_chat::run_app()
}
