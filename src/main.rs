// Import all useful traits of this crate.
use lm_sensors::prelude::*;
use regex::Regex;
use std::path::PathBuf;
use std::collections::HashMap;

struct Sensor<'a> {
    sensors: &'a lm_sensors::LMSensors,
    raw_chip: sensors_sys::sensors_chip_name,
    raw_feature: sensors_sys::sensors_feature,
    raw_subfeature: sensors_sys::sensors_subfeature,
    //chip: lm_sensors::ChipRef<'a>,
}

trait ReadSensor {
    fn read_value(&self) -> f64;
}

struct Fan<'a> {
    sensor: Sensor<'a>,
    output: PathBuf,
}


impl Sensor<'_> {
    fn new<'a, 'b>(
        sensors: &'a lm_sensors::LMSensors,
        subfeature: &'b lm_sensors::SubFeatureRef<'b>,
    ) -> Sensor<'a> {
        let feature = subfeature.feature();
        let chip = feature.chip();

        let raw_chip = chip.as_ref().clone();
        let raw_feature = feature.as_ref().clone();
        let raw_subfeature = subfeature.as_ref().clone();

        //let new_chip = unsafe { sensors.new_chip_ref(&raw_chip) };
        //let new_feature = unsafe { sensors.new_feature_ref(new_chip, &raw_feature) };
        //let new_subfeature = unsafe { sensors.new_sub_feature_ref(new_feature, &raw_subfeature) };
        Sensor {
            sensors,
            raw_chip,
            raw_feature,
            raw_subfeature,
        }
    }

    fn print_value(&self) {
        println!("chip: {}", self.read_val().unwrap());
    }

    pub fn read_val(&self) -> Result<lm_sensors::Value, lm_sensors::errors::Error> {
        self.get_subfeature().value()
    }

    // TODO: since I am a noob I need to reconstruct this every read
    fn get_subfeature(&self) -> lm_sensors::SubFeatureRef {
        let new_chip = unsafe { self.sensors.new_chip_ref(&self.raw_chip) };
        let new_feature = unsafe { self.sensors.new_feature_ref(new_chip, &self.raw_feature) };
        let new_subfeature = unsafe {
            self.sensors
                .new_sub_feature_ref(new_feature, &self.raw_subfeature)
        };
        new_subfeature
    }
}

impl ReadSensor for Sensor<'_> {
    fn read_value(&self) -> f64 {
        let val = self.get_subfeature().value().unwrap().raw_value();
        val
    }
}

impl ReadSensor for Fan<'_> {
    fn read_value(&self) -> f64 {
        self.sensor.read_value()
    }
}

impl Fan<'_> {
    fn new<'a>(sensor: Sensor<'a>, output: PathBuf) -> Fan<'a> {
        Fan { sensor, output }
    }
}

fn get_filter_for_item(item: &String, filter_map: &HashMap<String, Vec<String>>) -> Vec<String> {
    let mut filter_list = vec![];
    for (chip_filter, feature_filter) in filter_map {
        let chip_regex = Regex::new(&chip_filter).unwrap();
        if chip_regex.is_match(item) {
            filter_list.append(&mut feature_filter.clone());
        }
    }
    filter_list
}

fn get_sensors(sensors: &lm_sensors::LMSensors, filter_map: HashMap<String, Vec<String>>) -> (Vec<Sensor>, Vec<Fan>) {
    let mut my_sensors = vec![];
    let mut my_fans = vec![];
    for chip in sensors.chip_iter(None) {
        let chip_name = chip.name().unwrap_or_default();
        // Find all filters matching this chip
        let feature_filter_list = get_filter_for_item(&chip_name, &filter_map);
        if feature_filter_list.len() > 0 {
            println!("chip: {} {}", chip, chip.path().unwrap().to_str().unwrap());
            for feature in chip.feature_iter() {
                let mut matching_features = vec![];
                for feature_filter in &feature_filter_list {
                    let feature_regex = Regex::new(&feature_filter).unwrap();
                    if feature_regex.is_match(&feature_filter) {
                        matching_features.push(feature_filter);
                    }
                }
                if matching_features.len() != 1 {
                    println!("Could not find feature for {} in chip {}", ,chip_name);

                }
                println!("Feature: {}", feature);
                match feature.sub_feature_by_kind(lm_sensors::value::Kind::TemperatureInput) {
                    Ok(subfeature) => {
                        let my_sensor = Sensor::new(sensors, &subfeature);
                        my_sensors.push(my_sensor);
                    }
                    Err(_) => (),
                }
                match feature.sub_feature_by_kind(lm_sensors::value::Kind::FanInput) {
                    Ok(subfeature) => {
                        println!("Fan on chip: {} {} num {}", chip, chip.path().unwrap().to_str().unwrap(), subfeature.name().unwrap().unwrap());
                        let input_name = subfeature.name().unwrap().unwrap();
                        let re = Regex::new(r"fan([\d]+)_input").unwrap();
                        let matches = re.captures(&input_name).unwrap();
                        if matches.len() == 2 {
                            // construct pwm path
                            let chip_path = chip.path().unwrap();
                            let pwm_path = chip_path.join(format!("pwm{}", matches[1].to_string())).to_path_buf();
                            println!("Got pwm path {}", pwm_path.to_str().unwrap());
                            let fan = Fan::new(Sensor::new(sensors, &subfeature), pwm_path);
                            my_fans.push(fan);
                        }
                        println!("{}", input_name);
                    }
                    Err(_) => (),
                }
            }
        }
    }
    (my_sensors, my_fans)
}

fn main() {
    let sensors = lm_sensors::Initializer::default().initialize().unwrap();

    let mut sensor_filter: HashMap<String, Vec<String>> = HashMap::new();
    sensor_filter.entry("coretemp-isa-*".to_string()).or_default().push("Core 0".to_string());
    //print_chips_unsafe(&sensors);
    let sensors = get_sensors(&sensors, sensor_filter);
}
