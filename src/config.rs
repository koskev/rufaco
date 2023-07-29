use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};

use libmedium::{hwmon::Hwmons, parse_hwmons};
use log::info;
use serde::{Deserialize, Serialize};

use crate::{
    common::{ReadableValue, ReadableValueContainer},
    curve::{self, CurveContainer},
    fan::{FanContainer, FanSensor},
    hwmon,
    temperature::{TempSensor, TempSensorContainer},
};

#[derive(Serialize, Deserialize, Debug)]
pub struct HwmonConfig {
    pub chip: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PidCurve {
    pub sensor: String,
    pub target: f32,
    pub p: f32,
    pub i: f32,
    pub d: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinearCurve {
    pub sensor: String,
    pub steps: BTreeMap<i32, i32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StaticCurve {
    pub value: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MaximumCurve {
    pub sensors: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AverageCurve {
    pub sensors: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
pub enum CurveFunction {
    linear(LinearCurve),
    r#static(StaticCurve),
    maximum(MaximumCurve),
    average(AverageCurve),
    pid(PidCurve),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
pub enum SensorType {
    hwmon(HwmonConfig),
    file(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SensorConfig {
    pub id: String,
    pub sensor: SensorType,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FanConfig {
    pub id: String,
    pub startpwm: Option<u8>,
    pub minpwm: Option<u8>,
    pub sensor: SensorType,
    pub curve: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RufacoConfig {
    pub sensors: Vec<SensorConfig>,
    pub fans: Vec<FanConfig>,
    pub curves: Vec<FanCurve>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FanCurve {
    pub id: String,
    pub function: CurveFunction,
}

pub fn load_config(path: &str) -> RufacoConfig {
    let config_content = std::fs::read_to_string(path).unwrap_or_default();
    let config_yaml: RufacoConfig = serde_yaml::from_str(&config_content).unwrap();
    config_yaml
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

impl RufacoConfig {
    fn load_sensors(&mut self, hwmons: &Hwmons) -> HashMap<String, Arc<Mutex<TempSensor>>> {
        let mut sensors: HashMap<String, TempSensorContainer> = HashMap::new();
        for sensorconf in &self.sensors {
            match &sensorconf.sensor {
                SensorType::hwmon(conf) => {
                    let temp_sensor =
                        hwmon::load_hwmon_sensor(&hwmons, &conf.chip, &conf.name).unwrap();
                    let rufaco_sensor =
                        Arc::new(Mutex::new(TempSensor::new(sensorconf, temp_sensor)));
                    let id = rufaco_sensor.lock().unwrap().id.clone();
                    sensors.insert(id, rufaco_sensor);
                }
                SensorType::file(_path) => todo!(),
            }
        }
        sensors
    }

    fn load_curves(
        &mut self,
        sensors: HashMap<String, Arc<Mutex<TempSensor>>>,
    ) -> HashMap<String, Arc<Mutex<dyn ReadableValue>>> {
        let mut curves: HashMap<String, CurveContainer> = HashMap::new();
        for curveconf in &self.curves {
            let id = curveconf.id.clone();
            info!("Loading curve {id}");
            match &curveconf.function {
                CurveFunction::linear(curve) => {
                    let sensor_id = &curve.sensor;
                    info!("Searching for {}", sensor_id);
                    let sensor = get_sensor(sensor_id, &sensors, &curves);
                    curves.insert(
                        id,
                        Arc::new(Mutex::new(curve::LinearCurve::new(sensor, curve))),
                    );
                }
                CurveFunction::r#static(curve) => {
                    let sc = curve::StaticCurve { value: curve.value };
                    curves.insert(id, Arc::new(Mutex::new(sc)));
                }
                CurveFunction::maximum(curve) => {
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
                CurveFunction::average(curve) => {
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
                CurveFunction::pid(curve) => {
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
        &mut self,
        curves: HashMap<String, CurveContainer>,
        hwmons: &Hwmons,
    ) -> HashMap<String, Arc<Mutex<FanSensor>>> {
        let mut fans: HashMap<String, FanContainer> = HashMap::new();
        for sensorconf in &self.fans {
            match &sensorconf.sensor {
                SensorType::hwmon(conf) => {
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
                SensorType::file(_path) => todo!(),
            }
        }
        fans
    }

    pub fn create_all_sensors(&mut self) -> HashMap<String, Arc<Mutex<FanSensor>>> {
        let hwmons = parse_hwmons().unwrap();
        let mut sensors = self.load_sensors(&hwmons);
        let mut curves = self.load_curves(sensors);
        let mut fans: HashMap<String, FanContainer> = self.load_fans(curves, &hwmons);

        fans
    }
}
