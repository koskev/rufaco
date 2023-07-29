use std::io::Write;
use std::{
    collections::{HashMap, HashSet},
    fs::write,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Duration,
};

use libmedium::{hwmon::Hwmons, parse_hwmons};
use log::{debug, error, info, warn};

use crate::{
    common::{ReadableValueContainer, UpdatableInput, UpdatableOutput},
    config::{self, RufacoConfig},
    curve::{self, CurveContainer},
    fan::{FanContainer, FanSensor},
    hwmon,
    temperature::{TempSensor, TempSensorContainer},
};

pub struct FanHub {
    config: RufacoConfig,
    sensors: HashMap<String, TempSensorContainer>,
    curves: HashMap<String, CurveContainer>,
    fans: HashMap<String, FanContainer>,
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

impl FanHub {
    fn load_sensors(
        config: &RufacoConfig,
        hwmons: &Hwmons,
    ) -> HashMap<String, TempSensorContainer> {
        let mut sensors: HashMap<String, TempSensorContainer> = HashMap::new();
        for sensorconf in &config.sensors {
            match &sensorconf.sensor {
                config::SensorType::hwmon(conf) => {
                    let temp_sensor =
                        hwmon::load_hwmon_sensor(&hwmons, &conf.chip, &conf.name).unwrap();
                    let rufaco_sensor =
                        Arc::new(Mutex::new(TempSensor::new(sensorconf, temp_sensor)));
                    let id = rufaco_sensor.lock().unwrap().id.clone();
                    sensors.insert(id, rufaco_sensor);
                }
                config::SensorType::file(_path) => todo!(),
            }
        }
        sensors
    }

    fn load_curves(
        config: &RufacoConfig,
        sensors: &HashMap<String, TempSensorContainer>,
    ) -> HashMap<String, ReadableValueContainer> {
        let mut curves: HashMap<String, CurveContainer> = HashMap::new();
        for curveconf in &config.curves {
            let id = curveconf.id.clone();
            info!("Loading curve {id}");
            match &curveconf.function {
                config::CurveFunction::linear(curve) => {
                    let sensor_id = &curve.sensor;
                    info!("Searching for {}", sensor_id);
                    let sensor = get_sensor(sensor_id, &sensors, &curves);
                    curves.insert(
                        id,
                        Arc::new(Mutex::new(curve::LinearCurve::new(sensor, curve))),
                    );
                }
                config::CurveFunction::r#static(curve) => {
                    let sc = curve::StaticCurve { value: curve.value };
                    curves.insert(id, Arc::new(Mutex::new(sc)));
                }
                config::CurveFunction::maximum(curve) => {
                    let mut mc_sensors: Vec<ReadableValueContainer> = vec![];
                    for sensor_id in &curve.sensors {
                        let sensor = get_sensor(sensor_id, &sensors, &curves);
                        mc_sensors.push(sensor);
                    }
                    let mc = curve::MaximumCurve {
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
                    let ac = curve::AverageCurve {
                        sensors: ac_sensors,
                    };
                    curves.insert(id, Arc::new(Mutex::new(ac)));
                }
                config::CurveFunction::pid(curve) => {
                    let sensor_id = &curve.sensor;
                    let sensor = get_sensor(sensor_id, &sensors, &curves);
                    let pid_curve =
                        curve::PidCurve::new(sensor, curve.p, curve.i, curve.d, curve.target);
                    curves.insert(id, Arc::new(Mutex::new(pid_curve)));
                }
            }
        }
        curves
    }

    fn load_fans(
        config: &RufacoConfig,
        curves: &HashMap<String, CurveContainer>,
        hwmons: &Hwmons,
    ) -> HashMap<String, FanContainer> {
        let mut fans: HashMap<String, FanContainer> = HashMap::new();
        for sensorconf in &config.fans {
            match &sensorconf.sensor {
                config::SensorType::hwmon(conf) => {
                    let (fan_sensor, pwm_sensor) =
                        hwmon::load_hwmon_fan(hwmons, &conf.chip, &conf.name);
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
        fans
    }

    fn setup_fans(&mut self, measure_delay: u64, running: Arc<AtomicBool>) {
        // Create the fan parameters if they don't exist
        self.fans.iter().for_each(|(_id, fan_mutex)| {
            let mut fan = fan_mutex.lock().unwrap();
            let conf = self.config.fans.iter_mut().find(|val| val.id == fan.id);
            match conf {
                Some(conf) => {
                    // If any of the pwm settings is none we create them.
                    if conf.minpwm.is_none() || conf.startpwm.is_none() {
                        warn!(
                            "Fan {} does not have minpwm or startpwm configured. Measuring now...",
                            fan.id
                        );
                        let (min_pwm, start_pwm) = fan
                            .measure_fan(Duration::from_millis(measure_delay), 30, running.clone())
                            .unwrap();
                        conf.minpwm = Some(min_pwm);
                        conf.startpwm = Some(start_pwm);
                        // Write config
                        let conf_string = serde_yaml::to_string(&self.config).unwrap();
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
    }

    pub fn new(config: RufacoConfig, measure_delay: u64, running: Arc<AtomicBool>) -> Self {
        let hwmons = parse_hwmons().unwrap();
        let sensors = FanHub::load_sensors(&config, &hwmons);
        let curves = FanHub::load_curves(&config, &sensors);
        let fans = FanHub::load_fans(&config, &curves, &hwmons);
        let mut new_fanhub = Self {
            config,
            sensors,
            curves,
            fans,
        };

        new_fanhub.setup_fans(measure_delay, running);

        new_fanhub
    }

    pub fn update(&mut self) {
        // First update all sensors
        self.sensors.iter_mut().for_each(|(_id, sensor)| {
            sensor.lock().unwrap().update_input();
        });

        self.curves.iter().for_each(|(_id, curve)| {
            curve.lock().unwrap().update_value();
        });

        // Then update all fans
        self.fans.iter().for_each(|(_id, fan)| {
            fan.lock().unwrap().update_output();
        });
    }
}
