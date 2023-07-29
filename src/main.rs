// Import all useful traits of this crate.
use lm_sensors::prelude::*;

struct Sensor<'a> {
    sensors: &'a lm_sensors::LMSensors,
    raw_chip: sensors_sys::sensors_chip_name,
    raw_feature: sensors_sys::sensors_feature,
    raw_subfeature: sensors_sys::sensors_subfeature,
}

//struct Fan<'a> {
//    fan_input: Sensor<'a>,
//    fan_output: str,
//}

impl Sensor<'_> {
    fn new<'a, 'b>(
        sensors: &'a lm_sensors::LMSensors,
        subfeature: &'b lm_sensors::SubFeatureRef<'b>,
    ) -> Sensor<'a> {
        let feature = subfeature.feature();
        let chip = feature.chip();

        let raw_chip = chip.as_ref();
        let raw_feature = feature.as_ref();
        let raw_subfeature = subfeature.as_ref();
        Sensor {
            sensors,
            raw_chip: *raw_chip,
            raw_feature: *raw_feature,
            raw_subfeature: *raw_subfeature,
        }
    }
    fn print_value(&self) {
        println!("chip: {}", self.read_val().unwrap());
    }

    // TODO: since I am a noob I need to reconstruct this every read
    pub fn read_val(&self) -> Result<lm_sensors::Value, lm_sensors::errors::Error> {
        let new_chip = unsafe { self.sensors.new_chip_ref(&self.raw_chip) };
        let new_feature = unsafe { self.sensors.new_feature_ref(new_chip, &self.raw_feature) };
        let new_subfeature = unsafe {
            self.sensors
                .new_sub_feature_ref(new_feature, &self.raw_subfeature)
        };
        new_subfeature.value()
    }
}

fn create_chips(sensors: &lm_sensors::LMSensors) {
    let mut my_sensors = vec![];
    for chip in sensors.chip_iter(None) {
        println!("chip: {}", chip);
        for feature in chip.feature_iter() {
            match feature.sub_feature_by_kind(lm_sensors::value::Kind::TemperatureInput) {
                Ok(subfeature) => {
                    let raw_chip = chip.as_ref();
                    let raw_feature = feature.as_ref();
                    let raw_subfeature = subfeature.as_ref();

                    let my_sensor = Sensor::new(sensors, &subfeature);

                    my_sensors.push(my_sensor);
                }
                Err(_) => (),
            }
        }
    }

    for sens in my_sensors {
        sens.print_value();
    }
}

fn main() {
    let sensors = lm_sensors::Initializer::default().initialize().unwrap();
    //print_chips_unsafe(&sensors);
    create_chips(&sensors);
}
