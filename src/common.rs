use std::sync::{Arc, Mutex};

#[derive(PartialEq, PartialOrd, Clone, Copy, Debug)]
pub enum SensorType {
    TEMPERATURE,
    PERCENTAGE,
    RPM,
}

#[derive(PartialEq, Clone, Copy)]
pub struct SensorValue {
    kind: SensorType,
    factor: f64,
    value: f64,
}

impl SensorValue {
    pub fn new(kind: SensorType, factor: f64, value: f64) -> Self {
        Self {
            kind,
            factor,
            value,
        }
    }
    pub fn as_raw_value(&self) -> f64 {
        self.value
    }

    pub fn as_scaled_value(&self) -> f64 {
        self.value * self.factor
    }

    pub fn get_sensor_type(&self) -> SensorType {
        self.kind
    }
}

impl PartialOrd for SensorValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_scaled_value().partial_cmp(&other.as_scaled_value())
    }
}

pub trait ReadableValue {
    fn get_value(&self) -> SensorValue;
    fn update_value(&mut self) {}
}

pub trait UpdatableInput {
    fn update_input(&mut self);
}

pub trait UpdatableOutput {
    fn update_output(&mut self);
}

pub type ReadableValueContainer = Arc<Mutex<dyn ReadableValue>>;

#[cfg(test)]
mod test {
    use super::{SensorType, SensorValue};

    #[test]
    fn test_sensor_value_cmp() {
        let sensor_value1 = SensorValue::new(SensorType::PERCENTAGE, 1.0, 50.0);
        let sensor_value2 = SensorValue::new(SensorType::PERCENTAGE, 1.0, 25.0);

        assert!(sensor_value1 > sensor_value2);
        assert!(!(sensor_value1 < sensor_value2));
        assert!(sensor_value2 < sensor_value1);
        assert!(sensor_value1 != sensor_value2);
    }

    #[test]
    fn test_sensor_value() {
        let mut sensor_value = SensorValue::new(SensorType::PERCENTAGE, 0.0, 50.0);

        assert_eq!(sensor_value.get_sensor_type(), SensorType::PERCENTAGE);

        assert_eq!(sensor_value.as_raw_value(), 50.0);
        assert_eq!(sensor_value.as_scaled_value(), 0.0);
        sensor_value.factor = 1.0;

        assert_eq!(sensor_value.as_scaled_value(), 50.0);
        sensor_value.factor = 1.5;
        assert_eq!(sensor_value.as_scaled_value(), 75.0);
        sensor_value.factor = 1. / 10.;
        assert_eq!(sensor_value.as_scaled_value(), 5.0);
    }
}
