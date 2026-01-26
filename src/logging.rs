use std::fs;
use std::path::Path;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::defs::LOGS_DIR;

pub fn init_logging() -> () {
    // 1. Prepare the directory
    if !Path::new(LOGS_DIR).exists() {
        fs::create_dir_all(LOGS_DIR).expect("failed to create logs directory");
    }

    // 2. Setup the specific file path: "logs/latest.log"
    let file_path = format!("{}/latest.log", LOGS_DIR);
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .expect("failed to open log file");

    // 3. Create a non-blocking writer (crucial for performance)
    let (non_blocking, guard) = tracing_appender::non_blocking(file);

    // 4. Define the File Layer (No ANSI colors, usually specific format)
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    // 5. Define the Console Layer (With colors)
    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true);

    // 6. Define the filter (Read RUST_LOG env var, fallback to INFO)
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // 7. Register the subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    std::mem::forget(guard);
}
