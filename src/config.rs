use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct HwmonConfig {
    pub chip: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinearCurve {
    pub sensor: String,
    pub steps: BTreeMap<i32, i32>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
pub enum CurveFunction {
    linear(LinearCurve),
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

pub fn load_config() -> RufacoConfig {
    let config_content = std::fs::read_to_string("config.yaml").unwrap_or_default();
    let config_yaml: RufacoConfig = serde_yaml::from_str(&config_content).unwrap();
    println!("{:?}", config_yaml);
    config_yaml
}
