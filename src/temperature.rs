use std::sync::{Arc, Mutex};

use libmedium::{sensors::temp, units::Temperature};

use crate::{
    common::{ReadableValue, UpdatableInput},
    config::SensorConfig,
};

pub type TempSensorContainer = Arc<Mutex<TempSensor>>;

pub struct TempSensor {
    pub id: String,
    pub sensor: Box<dyn temp::TempSensor>,
    pub last_val: i32,
}

impl TempSensor {
    pub fn new(conf: &SensorConfig, sensor: Box<dyn temp::TempSensor>) -> Self {
        Self {
            id: conf.id.clone(),
            sensor,
            last_val: 0,
        }
    }

    fn get_temperature(&self) -> Result<Temperature, libmedium::sensors::Error> {
        self.sensor.read_input()
    }
}

impl ReadableValue for TempSensor {
    fn get_value(&self) -> i32 {
        self.last_val
    }
}

impl UpdatableInput for TempSensor {
    fn update_input(&mut self) {
        let val = self.get_temperature();
        match val {
            Ok(temp) => self.last_val = temp.as_millidegrees_celsius(),
            Err(err) => println!("Failed to read sensor {} with error {}", self.id, err),
        }
    }
}
