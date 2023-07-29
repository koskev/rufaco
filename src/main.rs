use clap::Parser;
use fanhub::FanHub;
use log::{debug, error, info, warn};
use simplelog::{ColorChoice, TermLogger, TerminalMode};

use std::{
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{self, Duration},
};

mod common;
mod config;
mod curve;
mod fan;
mod fanhub;
mod hwmon;
mod temperature;

use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Delay between fan measurements
    #[arg(long, default_value_t = 1000)]
    measure_delay: u64,
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    let verbosity = match args.verbose {
        true => log::LevelFilter::Debug,
        false => log::LevelFilter::Info,
    };

    TermLogger::init(
        verbosity,
        simplelog::Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .unwrap();

    #[allow(deprecated)]
    let home_config = std::env::home_dir()
        .unwrap()
        .as_path()
        .join(".config/rufaco/config.yaml");
    let config_search_paths = vec![
        "config.yaml",
        home_config.to_str().unwrap(),
        "/etc/rufaco/config.yaml",
    ];

    let selected_config = config_search_paths.iter().find(|f| {
        if std::path::Path::new(f).exists() {
            info!("Loading config from {f}");
            true
        } else {
            debug!("Config does not exist: {f}");
            false
        }
    });

    let running = Arc::new(AtomicBool::new(true));
    let rufaco_conf = config::load_config(selected_config.unwrap());
    let mut fan_hub = FanHub::new(rufaco_conf, args.measure_delay, running.clone());

    let mut stop_signal = Signals::new([SIGTERM, SIGINT]).unwrap();
    let running_copy = running.clone();

    thread::spawn(move || {
        for _sig in stop_signal.forever() {
            running_copy.store(false, Ordering::SeqCst);
            info!("Stopping program...");
        }
    });

    // Update
    while running.load(Ordering::SeqCst) {
        fan_hub.update();
        let sleep_duration = time::Duration::from_millis(100);
        thread::sleep(sleep_duration);
    }
}
