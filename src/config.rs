use std::{
    collections::{BTreeMap, HashMap, HashSet},
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
    file(FileConfig),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileConfig {
    pub path: String,
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
    let config_content = std::fs::read_to_string(path).unwrap();
    let config_yaml: RufacoConfig = serde_yaml::from_str(&config_content).unwrap();
    config_yaml
}

impl RufacoConfig {
    fn validate(&self) -> bool {
        let graph: HashMap<String, CurveFunction> = self
            .curves
            .iter()
            .map(|curve| (curve.id.clone(), curve.function.clone()))
            .collect();
        let sensors: HashSet<String> = self
            .sensors
            .iter()
            .map(|sensor| sensor.id.clone())
            .collect();

        // Ensure every curve is valid
        for (_node_id, func) in graph.iter() {
            for sensor_id in func.get_sensor_ids() {
                // sensor neither in curves nor sensors -> invalid config
                if !graph.contains_key(&sensor_id) && !sensors.contains(&sensor_id) {
                    return false;
                }
            }
        }

        // Check for cycles

        true
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::config::{FileConfig, SensorConfig, SensorType};

    use super::{load_config, CurveFunction, FanCurve, RufacoConfig};

    #[test]
    fn minimal_config() {
        let test_sensor = SensorType::file(FileConfig {
            path: "test".to_string(),
        });
        let test_sensor2 = SensorType::file(FileConfig {
            path: "test".to_string(),
        });
        let sensor_config = SensorConfig {
            id: "test_sensor1".to_string(),
            sensor: test_sensor,
        };
        let sensor_config2 = SensorConfig {
            id: "test_sensor2".to_string(),
            sensor: test_sensor2,
        };
        let curve_func = CurveFunction::maximum(super::MaximumCurve {
            sensors: vec!["test_sensor1".to_string(), "test_sensor2".to_string()],
        });
        let curve = FanCurve {
            id: "test_curve".to_string(),
            function: curve_func,
        };
        let curves = vec![curve];
        let conf = RufacoConfig {
            sensors: vec![sensor_config, sensor_config2],
            fans: vec![],
            curves,
        };

        assert!(conf.validate());
        let conf_empty = RufacoConfig {
            sensors: vec![],
            fans: vec![],
            curves: vec![],
        };
        assert!(conf_empty.validate());
    }
    #[test]
    fn all_curves_file() {
        let config = load_config("test/all_curves.yaml");
        assert!(config.validate());
    }

    #[test]
    fn all_curves_config() {
        let test_sensor = SensorType::file(FileConfig {
            path: "test".to_string(),
        });
        let test_sensor2 = SensorType::file(FileConfig {
            path: "test".to_string(),
        });
        let sensor_config = SensorConfig {
            id: "test_sensor1".to_string(),
            sensor: test_sensor,
        };
        let sensor_config2 = SensorConfig {
            id: "test_sensor2".to_string(),
            sensor: test_sensor2,
        };

        let curve_funcs = vec![
            CurveFunction::maximum(super::MaximumCurve {
                sensors: vec!["test_sensor1".to_string(), "test_sensor2".to_string()],
            }),
            CurveFunction::pid(super::PidCurve {
                p: 1.0,
                i: 1.0,
                d: 1.0,
                sensor: "test_sensor1".to_string(),
                target: 5.0,
            }),
            CurveFunction::linear(super::LinearCurve {
                sensor: "test_sensor1".to_string(),
                steps: BTreeMap::new(),
            }),
            CurveFunction::r#static(super::StaticCurve { value: 1 }),
            CurveFunction::average(super::AverageCurve {
                sensors: vec!["test_sensor1".to_string(), "test_sensor2".to_string()],
            }),
        ];

        let mut i = 0;
        let curves: Vec<FanCurve> = curve_funcs
            .into_iter()
            .map(|c| {
                i += 1;
                FanCurve {
                    id: format!("test_curve_{}", i),
                    function: c,
                }
            })
            .collect();

        let conf = RufacoConfig {
            sensors: vec![sensor_config, sensor_config2],
            fans: vec![],
            curves,
        };

        assert!(conf.validate());
    }

    #[test]
    fn non_existing_sensor_config() {
        let test_sensor = SensorType::file(FileConfig {
            path: "test".to_string(),
        });
        let sensor_config = SensorConfig {
            id: "test_sensor1".to_string(),
            sensor: test_sensor,
        };
        let curve_func = CurveFunction::maximum(super::MaximumCurve {
            sensors: vec!["invalid".to_string()],
        });
        let curve = FanCurve {
            id: "test_curve".to_string(),
            function: curve_func,
        };
        let curves = vec![curve];
        //pub fans: Vec<FanConfig>,
        //pub curves: Vec<FanCurve>,
        let conf = RufacoConfig {
            sensors: vec![sensor_config],
            fans: vec![],
            curves,
        };

        assert!(!conf.validate());
    }
}
