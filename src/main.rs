// Import all useful traits of this crate.
use lm_sensors::prelude::*;

struct Sensor<'a> {
    value: i32,
    min_value: i32,
    max_value: i32,
    chip: lm_sensors::ChipRef<'a>,
    feature: sensors_sys::sensors_feature,
}

//struct Fan<'a> {
//    fan_input: Sensor<'a>,
//    fan_output: str,
//}

impl Sensor<'_> {
    fn new<'a>(chip: lm_sensors::ChipRef<'a>, feature: sensors_sys::sensors_feature) -> Sensor<'a> {
        let new_sensor = Sensor {
            max_value: 0,
            min_value: 0,
            value: 0,
            feature,
            chip,
        };
        new_sensor
    }
    fn print_value(&self) {
        println!("chip: {}", self.value);
    }
}

//fn print_chips_unsafe(sensors: &lm_sensors::LMSensors) {
//    let mut y = 0;
//    let mut all_sensors = vec![];
//    let mut detected_sensor = std::ptr::null();
//    let mut my_sensors = vec![];
//    loop {
//        unsafe {
//            detected_sensor = sensors_sys::sensors_get_detected_chips(std::ptr::null(), &mut y);
//        }
//        if detected_sensor.is_null() {
//            break;
//        }
//        all_sensors.push(detected_sensor);
//    }
//
//    for sensor in all_sensors {
//        let chip;
//        unsafe {
//            chip = sensors.new_raw_chip(*sensor);
//        }
//        println!("chip: {}", chip);
//        for feature in chip.feature_iter() {
//            println!("feature: {}", feature);
//            let new_sensor = Sensor {
//                chip_name: "".to_string(),
//                max_value: 0,
//                min_value: 0,
//                value: 0,
//                feature: feature.clone(),
//            };
//            my_sensors.push(new_sensor);
//            break;
//        }
//        //std::mem::forget(chip);
//    }
//}

fn print_chips(sensors: &lm_sensors::LMSensors) {
    let mut my_sensors = vec![];
    for chip in sensors.chip_iter(None) {
        println!("chip: {}", chip);
        for feature in chip.feature_iter() {
            println!("feature: {}", feature);
            let new_sensor = Sensor::new(chip, *feature.as_ref());
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
