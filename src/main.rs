use clap::Parser;
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

use crate::common::UpdatableOutput;

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

    let mut rufaco_conf = config::load_config(selected_config.unwrap());
    let mut fans = rufaco_conf.create_all_sensors();

    let running = Arc::new(AtomicBool::new(true));
    let mut stop_signal = Signals::new([SIGTERM, SIGINT]).unwrap();
    let running_copy = running.clone();

    thread::spawn(move || {
        for _sig in stop_signal.forever() {
            running_copy.store(false, Ordering::SeqCst);
            info!("Stopping program...");
        }
    });

    // Create the fan parameters if they don't exist
    fans.iter().for_each(|(_id, fan_mutex)| {
        let mut fan = fan_mutex.lock().unwrap();
        let conf = rufaco_conf.fans.iter_mut().find(|val| val.id == fan.id);
        match conf {
            Some(conf) => {
                // If any of the pwm settings is none we create them.
                if conf.minpwm.is_none() || conf.startpwm.is_none() {
                    warn!(
                        "Fan {} does not have minpwm or startpwm configured. Measuring now...",
                        fan.id
                    );
                    let (min_pwm, start_pwm) = fan
                        .measure_fan(
                            Duration::from_millis(args.measure_delay),
                            30,
                            running.clone(),
                        )
                        .unwrap();
                    conf.minpwm = Some(min_pwm);
                    conf.startpwm = Some(start_pwm);
                    // Write config
                    let conf_string = serde_yaml::to_string(&rufaco_conf).unwrap();
                    debug!("Writing config {:?}", conf_string);
                    let mut f = std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open("config.yaml")
                        .expect("Couldn't open config file");
                    write!(f, "{}", conf_string).unwrap();
                }
            }
            None => error!(
                "Unable to find config with id {}. This should never happen!",
                fan.id
            ),
        }
    });

    // Update
    while running.load(Ordering::SeqCst) {
        // First update all sensors
        sensors.iter_mut().for_each(|(_id, sensor)| {
            sensor.lock().unwrap().update_input();
        });

        curves.iter().for_each(|(_id, curve)| {
            curve.lock().unwrap().update_value();
        });

        // Then update all fans
        fans.iter().for_each(|(_id, fan)| {
            fan.lock().unwrap().update_output();
        });
        let sleep_duration = time::Duration::from_millis(100);
        thread::sleep(sleep_duration);
    }
}
