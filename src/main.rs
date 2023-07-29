use lm_sensors::chip::SharedChip;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use yaml_rust::{Yaml, YamlLoader};

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

fn filter_list<'a, T: std::iter::IntoIterator<Item = &'a String>>(
    item: &String,
    list: T,
) -> Vec<String> {
    // TODO: Use fancy Rust stuff?
    let re = Regex::new(item).unwrap();
    let mut matches = vec![];
    for test_str in list {
        println!("Testing {} with {}", test_str, item);
        if re.is_match(&test_str) {
            matches.push(test_str.clone());
        }
    }
    matches
}

fn filter_chip_or_feature<
    'a,
    T1: std::iter::IntoIterator<Item = &'a String> + Clone,
    T2: std::iter::IntoIterator<Item = &'a String>,
>(
    sensor_list: T1,
    filter: T2,
) -> HashMap<String, String> {
    let mut sensors: HashMap<String, String> = HashMap::new();
    for sensor_filter in filter {
        let chip_matches = filter_list(sensor_filter, sensor_list.clone());
        // Check if we only have a single match
        if chip_matches.len() != 1 {
            println!("Unable to find sensor for {}", sensor_filter);
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
        println!("Found match for {}: {}", sensor_filter, chip_matches[0]);
        sensors.insert(sensor_filter.to_string(), chip_matches[0].to_string());
    }
    sensors
}

fn filter_sensorlist<'a>(
    mut sensor_list: SensorList<'a>,
    filter: &HashMap<String, Vec<String>>,
) -> SensorList<'a> {
    let chips = filter_chip_or_feature(sensor_list.keys(), filter.keys());
    let mut filtered_sensor_list: SensorList = HashMap::new();
    for (chip_filter, chip) in chips {
        // Matching sensor filter
        println!("{} {}", chip_filter, chip);
        filtered_sensor_list.insert(chip.clone(), HashMap::new());
        let features = filter_chip_or_feature(
            sensor_list.get(&chip).unwrap().keys(),
            filter.get(&chip_filter).unwrap(),
        );
        for (_, feature) in features {
            let sens: SensorType = sensor_list
                .get_mut(&chip)
                .unwrap()
                .remove(&feature)
                .unwrap();
            filtered_sensor_list
                .get_mut(&chip)
                .unwrap()
                .insert(feature, sens);
        }
    }
    filtered_sensor_list
}

fn get_all_sensors(sensors: &lm_sensors::LMSensors) -> SensorList {
    let mut found_sensors: SensorList = HashMap::new();
    for chip in sensors.chip_iter(None) {
        let chip_name = chip.name().unwrap_or_default().to_string();
        //println!("chip: {} {}", chip, chip.path().unwrap().to_str().unwrap());
        found_sensors.insert(chip_name.clone(), HashMap::new());
        for feature in chip.feature_iter() {
            let sensor_name = feature.to_string();
            //println!("Feature: {}", feature.label().unwrap());
            match feature.sub_feature_by_kind(lm_sensors::value::Kind::TemperatureInput) {
                Ok(subfeature) => {
                    let my_sensor = Sensor::new(sensors, &subfeature);
                    //my_sensors.push(my_sensor);
                    found_sensors
                        .get_mut(&chip_name)
                        .unwrap()
                        .insert(sensor_name.clone(), SensorType::Sensor(my_sensor));
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
                        // construct pwm path
                        let chip_path = chip.path().unwrap();
                        let pwm_path = chip_path
                            .join(format!("pwm{}", matches[1].to_string()))
                            .to_path_buf();
                        //println!("Got pwm path {}", pwm_path.to_str().unwrap());
                        let fan = Fan::new(Sensor::new(sensors, &subfeature), pwm_path);
                        found_sensors
                            .get_mut(&chip_name)
                            .unwrap()
                            .insert(sensor_name.clone(), SensorType::Fan(fan));
                    }
                    //println!("{}", input_name);
                }
                Err(_) => (),
            }
        }
    }
    found_sensors
}

enum SensorKind {
    Hwmon,
    File,
}

struct SensorConf {
    sensor_id: String,
    sensor_type: SensorKind,
    values: HashMap<String, String>,
}

struct RufacoConf {
    sensors: Vec<SensorConf>,
}

impl RufacoConf {
    fn new() -> Self {
        Self { sensors: vec![] }
    }

    fn add_sensor(&mut self, conf: SensorConf) {
        self.sensors.push(conf);
    }
}

impl SensorConf {
    fn new(id: String, sensor_type: SensorKind, values: HashMap<String, String>) -> Self {
        Self {
            sensor_id: id,
            sensor_type,
            values,
        }
    }
}

fn load_config() -> RufacoConf {
    let config_content = fs::read_to_string("config.yaml").unwrap_or_default();
    let config_yaml = YamlLoader::load_from_str(&config_content).unwrap_or_default();
    let mut conf = RufacoConf::new();
    //println!("{:?}", config_yaml[0]);
    for sensor in config_yaml[0]["sensors"].as_vec() {
        let test = &sensor[0];
        println!("{:?}", test.as_hash().unwrap());
        let sensor_config = test.as_hash().unwrap();
        let sensor_id = &sensor_config[&Yaml::String("id".to_string())]
            .as_str()
            .unwrap()
            .to_string();
        println!("Read sensor with ID: {}", sensor_id);
        // TODO: check for possible types
        let hw_conf_items: HashMap<String, String> = sensor_config
            [&Yaml::String("hwmon".to_string())]
            .as_vec()
            .unwrap()[0]
            .as_hash()
            .unwrap()
            .iter()
            .map(|(key, val)| {
                (
                    key.as_str().unwrap().to_string(),
                    val.as_str().unwrap().to_string(),
                )
            })
            .collect();
        println!("{:?}", hw_conf_items);
        let sensor_conf = SensorConf::new(sensor_id.clone(), SensorKind::Hwmon, hw_conf_items);
        conf.add_sensor(sensor_conf);
    }
    conf
}

fn main() {
    let sensors = lm_sensors::Initializer::default().initialize().unwrap();
    let conf = load_config();

    for sensor in conf.sensors {
        if sensor.sensor_type == SensorKind::Hwmon {}
    }

    let mut sensor_filter: HashMap<String, Vec<String>> = HashMap::new();
    sensor_filter
        .entry("coretemp-.*".to_string())
        .or_default()
        .push("Core 0".to_string());
    //print_chips_unsafe(&sensors);
    let all_sensors = get_all_sensors(&sensors);
    let filtered_sensors = filter_sensorlist(all_sensors, &sensor_filter);
    for (chip, feature_list) in filtered_sensors {
        println!("Chip: {}", chip);
        for feature in feature_list {
            println!("Feature: {}", feature.0);
        }
    }
}
