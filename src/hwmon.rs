use log::info;

use crate::fan::{FanInput, FanOutput, HwmonFan, HwmonPwm};
use libmedium::{
    hwmon::sync_hwmon::Hwmons,
    sensors::{
        sync_sensors::{temp, SyncSensor},
        Sensor,
    },
};

pub fn load_hwmon_sensor(
    hwmons: &Hwmons,
    chip_name: &String,
    sensor_name: &String,
) -> Option<Box<dyn temp::TempSensor>> {
    // Load hwmon
    info!("Loading hwmon config with name {}", chip_name);
    for hwmon in hwmons.hwmons_by_name(chip_name) {
        info!("Loading hwmon {:?}", hwmon.name());
        for temp in hwmon.temps().values() {
            if sensor_name == &temp.name() {
                info!("Matched hwmon {} and sensor {}", hwmon.name(), temp.name());
                return Some(Box::new(temp.clone()));
            }
        }
    }
    None
}

type FanInputOutput = (Option<Box<dyn FanInput>>, Option<Box<dyn FanOutput>>);
pub fn load_hwmon_fan(hwmons: &Hwmons, chip_name: &String, sensor_name: &String) -> FanInputOutput {
    // Load hwmon
    info!("Loading hwmon config with name {}", chip_name);
    for hwmon in hwmons.hwmons_by_name(chip_name) {
        info!("Loading hwmon {:?}", hwmon.name());
        for temp in hwmon.writeable_fans().values() {
            if sensor_name == &temp.name() {
                info!("Matched hwmon {} and sensor {}", hwmon.name(), temp.name());
                let fan_input = Box::new(HwmonFan {
                    fan_input: Box::new(temp.clone()),
                });
                let fan_pwm = Box::new(HwmonPwm {
                    fan_pwm: Box::new(hwmon.writeable_pwm(temp.index()).unwrap().clone()),
                });
                return (Some(fan_input), Some(fan_pwm));
            }
        }
    }
    (None, None)
}
