#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result {
    ai_chat::run_app()
}

#[cfg(target_os = "android")]
fn main() {
    // Android 入口为 lib 中的 android_main，bin 不需要实际逻辑
}
