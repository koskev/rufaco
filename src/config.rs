use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    ops::IndexMut,
    vec,
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct HwmonConfig {
    pub chip: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PidCurve {
    pub sensor: String,
    pub target: f32,
    pub p: f32,
    pub i: f32,
    pub d: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinearCurve {
    pub sensor: String,
    pub steps: BTreeMap<i32, i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StaticCurve {
    pub value: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MaximumCurve {
    pub sensors: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AverageCurve {
    pub sensors: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
pub enum CurveFunction {
    linear(LinearCurve),
    r#static(StaticCurve),
    maximum(MaximumCurve),
    average(AverageCurve),
    pid(PidCurve),
}

impl CurveFunction {
    fn get_sensor_ids(&self) -> Vec<String> {
        match self {
            CurveFunction::linear(curve) => vec![curve.sensor.clone()],
            CurveFunction::pid(curve) => vec![curve.sensor.clone()],
            CurveFunction::r#static(_curve) => vec![],
            CurveFunction::maximum(curve) => curve.sensors.clone(),
            CurveFunction::average(curve) => curve.sensors.clone(),
        }
    }
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

struct Edge {
    from: String,
    to: String,
}

struct Node {
    id: String,
}

impl RufacoConfig {
    fn validate(&mut self) -> bool {
        true
    }
}
