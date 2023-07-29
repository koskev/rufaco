use clap::Parser;
use log::{debug, error, info, trace, warn};
use simplelog::{ColorChoice, TermLogger, TerminalMode};

use std::{
    collections::HashMap,
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{self, Duration},
};

use common::ReadableValueContainer;
use curve::{AverageCurve, CurveContainer, LinearCurve, MaximumCurve, StaticCurve};
use libmedium::{
    hwmon::Hwmons,
    parse_hwmons,
    sensors::{fan::WriteableFanSensor, pwm::WriteablePwmSensor, temp, Sensor},
};

mod common;
mod config;
mod curve;
mod fan;
mod temperature;

use fan::{FanContainer, FanInput, FanOutput, FanSensor};
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};
use temperature::{TempSensor, TempSensorContainer};

use crate::{
    common::{UpdatableInput, UpdatableOutput},
    curve::PidCurve,
    fan::{HwmonFan, HwmonPwm},
};

// TODO: refactor fan and sensor
fn load_hwmon_sensor(
    hwmons: &Hwmons,
    chip_name: &String,
    sensor_name: &String,
) -> Option<Box<dyn temp::TempSensor>> {
    // Load hwmon
    info!("Loading hwmon config with name {}", chip_name);
    for hwmon in hwmons.hwmons_by_name(chip_name) {
        info!("Loading hwmon {:?}", hwmon.name());
        for (_, temp) in hwmon.temps() {
            if sensor_name == &temp.name() {
                info!("Matched hwmon {} and sensor {}", hwmon.name(), temp.name());
                return Some(Box::new(temp.clone()));
            }
        }
    }
    None
}

fn load_hwmon_fan(
    hwmons: &Hwmons,
    chip_name: &String,
    sensor_name: &String,
) -> (Option<Box<dyn FanInput>>, Option<Box<dyn FanOutput>>) {
    // Load hwmon
    info!("Loading hwmon config with name {}", chip_name);
    for hwmon in hwmons.hwmons_by_name(chip_name) {
        info!("Loading hwmon {:?}", hwmon.name());
        for (_, temp) in hwmon.writeable_fans() {
            if sensor_name == &temp.name() {
                info!("Matched hwmon {} and sensor {}", hwmon.name(), temp.name());
                let fan_input = Box::new(HwmonFan {
                    fan_input: Box::new(temp.clone()),
                });
                let fan_pwm = Box::new(HwmonPwm {
                    fan_pwm: Box::new(hwmon.writeable_pwm(temp.index()).unwrap().clone()),
                });
                return (Some(fan_input), Some(fan_pwm));
            }
        }
    }
    (None, None)
}

fn get_sensor(
    sensor_id: &str,
    sensors: &HashMap<String, TempSensorContainer>,
    curves: &HashMap<String, CurveContainer>,
) -> ReadableValueContainer {
    let sensor: ReadableValueContainer;
    if sensors.contains_key(sensor_id) {
        sensor = sensors[sensor_id].clone();
    } else if curves.contains_key(sensor_id) {
        sensor = curves[sensor_id].clone();
    } else {
        // FIXME: Configs are sensitive to the order.
        todo!(
            "Config doesn't contain {}! Be sure to place them in the correct order",
            sensor_id
        )
    }
    sensor
}

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

    let config_search_paths = vec![
        "config.yaml",
        "~/.config/rufaco/config.yaml",
        "/etc/rufaco/config.yaml",
    ];

    let selected_config = config_search_paths.iter().find_map(|f| {
        if std::path::Path::new(f).exists() {
            Some(f)
        } else {
            None
        }
    });

    let mut rufaco_conf = config::load_config(selected_config.unwrap());
    let hwmons = parse_hwmons().unwrap();
    let mut sensors: HashMap<String, TempSensorContainer> = HashMap::new();
    let mut fans: HashMap<String, FanContainer> = HashMap::new();
    let mut curves: HashMap<String, CurveContainer> = HashMap::new();
    for sensorconf in &rufaco_conf.sensors {
        match &sensorconf.sensor {
            config::SensorType::hwmon(conf) => {
                let temp_sensor = load_hwmon_sensor(&hwmons, &conf.chip, &conf.name).unwrap();
                let rufaco_sensor = Arc::new(Mutex::new(TempSensor::new(sensorconf, temp_sensor)));
                let id = rufaco_sensor.lock().unwrap().id.clone();
                sensors.insert(id, rufaco_sensor);
            }
            config::SensorType::file(_path) => todo!(),
        }
    }

    for curveconf in &rufaco_conf.curves {
        let id = curveconf.id.clone();
        info!("Loading curve {id}");
        match &curveconf.function {
            config::CurveFunction::linear(curve) => {
                // TODO: refactor to use function? Borrow problem
                let sensor_id = &curve.sensor;
                info!("Searching for {}", sensor_id);
                let sensor = get_sensor(sensor_id, &sensors, &curves);
                curves.insert(
                    curveconf.id.clone(),
                    Arc::new(Mutex::new(LinearCurve::new(sensor, curve))),
                );
            }
            config::CurveFunction::r#static(curve) => {
                let sc = StaticCurve { value: curve.value };
                curves.insert(id, Arc::new(Mutex::new(sc)));
            }
            config::CurveFunction::maximum(curve) => {
                let mut mc_sensors: Vec<ReadableValueContainer> = vec![];
                for sensor_id in &curve.sensors {
                    let sensor = get_sensor(sensor_id, &sensors, &curves);
                    mc_sensors.push(sensor);
                }
                let mc = MaximumCurve {
                    sensors: mc_sensors,
                };
                curves.insert(id, Arc::new(Mutex::new(mc)));
            }
            config::CurveFunction::average(curve) => {
                let mut ac_sensors: Vec<ReadableValueContainer> = vec![];
                for sensor_id in &curve.sensors {
                    let sensor = get_sensor(sensor_id, &sensors, &curves);
                    ac_sensors.push(sensor);
                }
                let ac = AverageCurve {
                    sensors: ac_sensors,
                };
                curves.insert(id, Arc::new(Mutex::new(ac)));
            }
            config::CurveFunction::pid(curve) => {
                let sensor_id = &curve.sensor;
                let sensor = get_sensor(sensor_id, &sensors, &curves);
                let pid_curve = PidCurve::new(sensor, curve.p, curve.i, curve.d, curve.target);
                curves.insert(id, Arc::new(Mutex::new(pid_curve)));
            }
        }
    }

    for sensorconf in &rufaco_conf.fans {
        match &sensorconf.sensor {
            config::SensorType::hwmon(conf) => {
                let (fan_sensor, pwm_sensor) = load_hwmon_fan(&hwmons, &conf.chip, &conf.name);
                let curve = curves[&sensorconf.curve].clone();
                let rufaco_sensor = Arc::new(Mutex::new(FanSensor::new(
                    sensorconf,
                    fan_sensor.unwrap(),
                    pwm_sensor.unwrap(),
                    curve,
                )));
                let id = rufaco_sensor.lock().unwrap().id.clone();
                fans.insert(id, rufaco_sensor);
            }
            config::SensorType::file(_path) => todo!(),
        }
    }

    let running = Arc::new(AtomicBool::new(true));
    let mut stop_signal = Signals::new(&[SIGTERM, SIGINT]).unwrap();
    let running_copy = running.clone();

    thread::spawn(move || {
        for _sig in stop_signal.forever() {
            running_copy.store(false, Ordering::SeqCst);
            info!("Stopping program...");
        }
    });

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
