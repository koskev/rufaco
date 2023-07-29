// Import all useful traits of this crate.
use lm_sensors::prelude::*;

struct Sensor<'a> {
    value: i32,
    min_value: i32,
    max_value: i32,
    chip: lm_sensors::ChipRef<'a>,
    raw_feature: sensors_sys::sensors_feature,
    sensor_facility: &'a lm_sensors::LMSensors,
    feature: lm_sensors::FeatureRef<'a>,
}

//struct Fan<'a> {
//    fan_input: Sensor<'a>,
//    fan_output: str,
//}

impl Sensor<'_> {
    fn new<'a>(
        chip: lm_sensors::ChipRef<'a>,
        raw_feature: sensors_sys::sensors_feature,
        sensor_facility: &'a lm_sensors::LMSensors,
    ) -> Sensor<'a> {
        let feature: lm_sensors::FeatureRef;
        // FIXME: Since I am a Rust noob, I have no clue why I can't save a FeatureRef
        unsafe {
            feature = sensor_facility.new_feature_ref(chip, raw_feature.clone());
        }
        let new_sensor = Sensor {
            max_value: 0,
            min_value: 0,
            value: 0,
            chip,
            raw_feature,
            sensor_facility,
            feature,
        };
        new_sensor
    }

    fn print_value(&self) {
        println!("chip: {}", self.value);
        println!("{}", self.get_feature().name().unwrap().unwrap());
    }

    fn get_feature(&self) -> lm_sensors::FeatureRef {
        let feature: lm_sensors::FeatureRef;
        // FIXME: Since I am a Rust noob, I have no clue why I can't save a FeatureRef
        unsafe {
            feature = self
                .sensor_facility
                .new_feature_ref(self.chip, &self.raw_feature);
        }
        feature
    }
}

fn print_chips(sensors: &lm_sensors::LMSensors) {
    let mut my_sensors = vec![];
    for chip in sensors.chip_iter(None) {
        println!("chip: {}", chip);
        for feature in chip.feature_iter() {
            println!("feature: {}", feature);
            let new_sensor = Sensor::new(chip, *feature.as_ref(), sensors);
            my_sensors.push(new_sensor);
        }
    }

    for sens in my_sensors {
        sens.print_value();
    }
}

fn main() {
    let sensors = lm_sensors::Initializer::default().initialize().unwrap();
    //print_chips_unsafe(&sensors);
    print_chips(&sensors);
}
