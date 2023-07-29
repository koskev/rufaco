use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use curve::{CurveContainer, LinearCurve};
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

use fan::{FanContainer, FanSensor};
use temperature::{TempSensor, TempSensorContainer};

use crate::common::{UpdatableInput, UpdatableOutput};

// TODO: refactor fan and sensor
fn load_hwmon_sensor(
    hwmons: &Hwmons,
    chip_name: &String,
    sensor_name: &String,
) -> Option<Box<dyn temp::TempSensor>> {
    // Load hwmon
    println!("Loading hwmon config with name {}", chip_name);
    for hwmon in hwmons.hwmons_by_name(chip_name) {
        println!("Loading hwmon {:?}", hwmon.name());
        for (_, temp) in hwmon.temps() {
            if sensor_name == &temp.name() {
                println!("Matched hwmon {} and sensor {}", hwmon.name(), temp.name());
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
) -> (
    Option<Box<dyn WriteableFanSensor>>,
    Option<Box<dyn WriteablePwmSensor>>,
) {
    // Load hwmon
    println!("Loading hwmon config with name {}", chip_name);
    for hwmon in hwmons.hwmons_by_name(chip_name) {
        println!("Loading hwmon {:?}", hwmon.name());
        for (_, temp) in hwmon.writeable_fans() {
            if sensor_name == &temp.name() {
                println!("Matched hwmon {} and sensor {}", hwmon.name(), temp.name());
                let fan_input = Box::new(temp.clone());
                let fan_pwm = Box::new(hwmon.writeable_pwm(temp.index()).unwrap().clone());
                return (Some(fan_input), Some(fan_pwm));
            }
        }
    }
    (None, None)
}

fn main() {
    let rufaco_conf = config::load_config();
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
        match &curveconf.function {
            config::CurveFunction::linear(curve) => {
                let sensor = sensors[&curve.sensor].clone();
                curves.insert(
                    curveconf.id.clone(),
                    Arc::new(Mutex::new(LinearCurve::new(sensor, curve))),
                );
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

    // Update
    loop {
        // First update all sensors
        sensors.iter_mut().for_each(|(_id, sensor)| {
            sensor.lock().unwrap().update_input();
        });

        // Then update all fans
        fans.iter().for_each(|(_id, fan)| {
            fan.lock().unwrap().update_output();
        });
    }
}
