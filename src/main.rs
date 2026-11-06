#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
compile_error!("AMStats requiere macOS 13 o posterior en Apple Silicon.");

mod config;
mod metrics;
mod platform;
mod ui;
mod updates;

fn main() {
    if std::env::args().any(|arg| arg == "--diagnose") {
        metrics::diagnose();
    } else {
        ui::run();
    }
}
