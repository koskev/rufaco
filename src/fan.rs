use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use libmedium::{
    sensors::{fan::WriteableFanSensor, pwm::WriteablePwmSensor},
    units::{self, AngularVelocity},
};
use log::{debug, error};

use crate::{
    common::{ReadableValue, UpdatableInput, UpdatableOutput},
    config::FanConfig,
    curve::CurveContainer,
};

pub type FanContainer = Arc<Mutex<FanSensor>>;

pub struct FanSensor {
    pub id: String,
    pub fan_input: Box<dyn WriteableFanSensor>,
    pub fan_pwm: Box<dyn WriteablePwmSensor>,
    pub curve: CurveContainer,
    pub last_val: u32,
    /// Min PWM to keep the fan spinning
    pub min_pwm: u8,
    /// PWM to start the fan
    pub start_pwm: u8,
}

impl FanSensor {
    pub fn new(
        conf: &FanConfig,
        fan_input: Box<dyn WriteableFanSensor>,
        fan_pwm: Box<dyn WriteablePwmSensor>,
        curve: CurveContainer,
    ) -> Self {
        Self {
            id: conf.id.clone(),
            fan_input,
            fan_pwm,
            curve,
            last_val: 0,
            min_pwm: conf.minpwm,
            start_pwm: conf.startpwm,
        }
    }
}

    fn get_rpm(&self) -> Result<AngularVelocity, libmedium::sensors::Error> {
        self.fan_input.read_input()
    }

    fn set_output(&self, percent: i8) {
        println!("{:?}", self.fan_pwm.hwmon_path());
        self.fan_pwm.write_pwm(units::Pwm::from_u8(255)).unwrap();
        //self.fan.write_pwm();
    }
}

impl UpdatableOutput for FanSensor {
    fn update_output(&mut self) {
        let val = self.curve.lock().unwrap().get_value();
        // TODO: implement start pwm
        let pwm_range = 255 - self.min_pwm;
        let pwm_val =
            (self.min_pwm + (val as f32 / 100. * pwm_range as f32) as u8) * (val > 0) as u8;
        let _ = self
            .fan_pwm
            .write_pwm(units::Pwm::from_u8(pwm_val))
            .unwrap();
        //println!("Got value {val} for fan {} pwm {pwm_val}", self.id);
    }
}

impl UpdatableInput for FanSensor {
    fn update_input(&mut self) {
        let val: Result<AngularVelocity, libmedium::sensors::Error> = self.fan_input.read_input();
        match val {
            Ok(speed) => self.last_val = speed.as_rpm(),
            Err(err) => error!("Failed to read sensor {} with error {}", self.id, err),
        }
    }
}

impl ReadableValue for FanSensor {
    fn get_value(&self) -> i32 {
        self.last_val as i32
    }
}
