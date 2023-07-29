// Import all useful traits of this crate.
use lm_sensors::prelude::*;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;

type SensorList<'a> = HashMap<String, HashMap<String, SensorType<'a>>>;

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

enum SensorType<'a> {
    Sensor(Sensor<'a>),
    Fan(Fan<'a>),
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

fn filter_list<'a, T: std::iter::IntoIterator<Item = &'a String>>(
    item: &String,
    list: T,
) -> Vec<String> {
    // TODO: Use fancy Rust stuff?
    let re = Regex::new(item).unwrap();
    let mut matches = vec![];
    for test_str in list {
        if re.is_match(&test_str) {
            matches.push(test_str.clone());
        }
    }
    matches
}

fn filter_sensors(sensor_list: SensorList, filter: HashMap<String, Vec<String>>) {
    for (chip_filter, sensor_filter_list) in &filter {
        let mut chip_matches = filter_list(chip_filter, sensor_list.keys());
        // Check if we only have a single match
        if chip_matches.len() != 1 {
            println!("Unable to find chip for {}", chip_filter);
            if chip_matches.len() == 0 {
                println!("No possible matches found");
            } else {
                println!(
                    "Multiple possible matches found: {}",
                    chip_matches.join(", ")
                );
            }
            continue;
        }
        println!("Found match for {}: {}", chip_filter, chip_matches[0]);
        // Matching sensor filter
        for sensor_filter in sensor_filter_list {
            //let mut sensor_matches = filter_list(sensor_filter, sensor_list.get(chip_matches[0]));
        }
    }
}

fn get_all_sensors(sensors: &lm_sensors::LMSensors) -> SensorList {
    let mut found_sensors: SensorList = HashMap::new();
    for chip in sensors.chip_iter(None) {
        let chip_name = chip.name().unwrap_or_default().to_string();
        //println!("chip: {} {}", chip, chip.path().unwrap().to_str().unwrap());
        found_sensors.insert(chip_name.clone(), HashMap::new());
        for feature in chip.feature_iter() {
            //println!("Feature: {}", feature);
            match feature.sub_feature_by_kind(lm_sensors::value::Kind::TemperatureInput) {
                Ok(subfeature) => {
                    let sensor_name = subfeature.name().unwrap().unwrap_or_default().to_string();
                    let my_sensor = Sensor::new(sensors, &subfeature);
                    //my_sensors.push(my_sensor);
                    found_sensors
                        .get_mut(&chip_name)
                        .unwrap_or(&mut HashMap::new())
                        .insert(sensor_name, SensorType::Sensor(my_sensor));
                }
                Err(_) => (),
            }
            match feature.sub_feature_by_kind(lm_sensors::value::Kind::FanInput) {
                Ok(subfeature) => {
                    //println!(
                    //    "Fan on chip: {} {} num {}",
                    //    chip,
                    //    chip.path().unwrap().to_str().unwrap(),
                    //    subfeature.name().unwrap().unwrap()
                    //);
                    let input_name = subfeature.name().unwrap().unwrap();
                    let re = Regex::new(r"fan([\d]+)_input").unwrap();
                    let matches = re.captures(&input_name).unwrap();
                    if matches.len() == 2 {
                        let sensor_name =
                            subfeature.name().unwrap().unwrap_or_default().to_string();
                        // construct pwm path
                        let chip_path = chip.path().unwrap();
                        let pwm_path = chip_path
                            .join(format!("pwm{}", matches[1].to_string()))
                            .to_path_buf();
                        //println!("Got pwm path {}", pwm_path.to_str().unwrap());
                        let fan = Fan::new(Sensor::new(sensors, &subfeature), pwm_path);
                        found_sensors
                            .get_mut(&chip_name)
                            .unwrap_or(&mut HashMap::new())
                            .insert(sensor_name, SensorType::Fan(fan));
                    }
                    //println!("{}", input_name);
                }
                Err(_) => (),
            }
        }
    }
    found_sensors
}

fn main() {
    let sensors = lm_sensors::Initializer::default().initialize().unwrap();

    let mut sensor_filter: HashMap<String, Vec<String>> = HashMap::new();
    sensor_filter
        .entry(".*".to_string())
        .or_default()
        .push("Core 0".to_string());
    //print_chips_unsafe(&sensors);
    let all_sensors = get_all_sensors(&sensors);
    filter_sensors(all_sensors, sensor_filter);
}
