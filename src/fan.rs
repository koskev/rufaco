use std::sync::{Arc, Mutex};

use libmedium::{
    sensors::{fan::WriteableFanSensor, pwm::WriteablePwmSensor},
    units::{self, AngularVelocity},
};

use crate::{common::UpdatableOutput, config::FanConfig, curve::CurveContainer};

pub type FanContainer = Arc<Mutex<FanSensor>>;

pub struct FanSensor {
    pub id: String,
    pub fan_input: Box<dyn WriteableFanSensor>,
    pub fan_pwm: Box<dyn WriteablePwmSensor>,
    pub curve: CurveContainer,
    pub last_val: i32,
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
        let pwm_val = (val as f32 / 100. * 255.0) as u8;
        let _ = self
            .fan_pwm
            .write_pwm(units::Pwm::from_u8(pwm_val))
            .unwrap();
        //println!("Got value {val} for fan {} pwm {pwm_val}", self.id);
    }
}
