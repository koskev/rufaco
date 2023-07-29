use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use libmedium::{
    sensors::{fan::WriteableFanSensor, pwm::WriteablePwmSensor},
    units::{self, AngularVelocity},
};
use log::{debug, error, warn};

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
        let min_pwm = match conf.minpwm {
            Some(pwm) => pwm,
            None => 0,
        };
        let start_pwm = match conf.startpwm {
            Some(pwm) => pwm,
            None => 0,
        };
        Self {
            id: conf.id.clone(),
            fan_input,
            fan_pwm,
            curve,
            last_val: 0,
            min_pwm,
            start_pwm,
        }
    }
}

impl FanSensor {
    pub fn is_spinning(&self) -> bool {
        self.get_value() != 0
    }

    fn measure_pwm(
        &mut self,
        pwm: u8,
        max_rpm_diff: i32,
        wait_time: Duration,
        stop_signal: Arc<AtomicBool>,
    ) -> Option<i32> {
        debug!("Measuring fan {} with pwm of {}", self.id, pwm);
        self.fan_pwm
            .write_pwm(units::Pwm::from_u8(pwm as u8))
            .unwrap();
        let mut max_diff = 10000;
        let mut rpms: VecDeque<i32> = VecDeque::new();
        let mut mean = 0;
        while max_diff > max_rpm_diff {
            if !stop_signal.load(Ordering::SeqCst) {
                debug!("Stop signal received. Stopping measurement");
                return None;
            }
            thread::sleep(wait_time);
            self.update_input();
            let rpm = self.get_value();
            rpms.push_front(rpm);
            if rpms.len() > 5 {
                rpms.pop_back();
                mean = rpms.iter().sum::<i32>() / rpms.len() as i32;
                max_diff = *rpms
                    .iter()
                    .max_by(|a, b| {
                        let diff_a = i32::abs(mean - *a);
                        let diff_b = i32::abs(mean - *b);

                        diff_a.cmp(&diff_b)
                    })
                    .unwrap();
                max_diff = i32::abs(max_diff - mean);
                debug!(
                    "Measured max diff of {} with vals {:?} and mean {mean} for fan {}",
                    max_diff, rpms, self.id
                );
            }
        }
        return Some(mean);
    }

    pub fn measure_fan(
        &mut self,
        wait_time: Duration,
        max_rpm_diff: i32,
        stop_signal: Arc<AtomicBool>,
    ) -> Option<(u8, u8)> {
        debug!("Measuring fan {}", self.id);
        for pwm in (0..255) {
            debug!("Setting fan {} to {}", self.id, pwm);
            let rpm = self.measure_pwm(pwm, max_rpm_diff, wait_time, stop_signal.clone());
            match rpm {
                Some(rpm) => {
                    debug!("Settled rpm {rpm} with pwm {pwm}");
                    if rpm != 0 {
                        debug!("Rpm is != 0. Found start pwm: {pwm}");
                        self.start_pwm = pwm;
                        break;
                    }
                }
                None => return None,
            }
        }
        // Set to 0 in case the fan never stops
        self.min_pwm = 0;
        debug!("Finding min_pwm for {}", self.id);
        // Measure min pwm
        for pwm in (0..self.start_pwm).rev() {
            let rpm = self.measure_pwm(pwm, max_rpm_diff, wait_time, stop_signal.clone());
            match rpm {
                Some(rpm) => {
                    debug!("Settled rpm {rpm} with pwm {pwm}");
                    if rpm == 0 {
                        break;
                    }
                    debug!("New min_pwm {pwm}");
                    self.min_pwm = pwm;
                }
                None => return None,
            }
        }
        debug!(
            "PWM measurement for {} is min {} start {}",
            self.id, self.min_pwm, self.start_pwm
        );
        Some((self.min_pwm, self.start_pwm))
    }
}

impl UpdatableOutput for FanSensor {
    fn update_output(&mut self) {
        self.update_input();
        let percentage = self.curve.lock().unwrap().get_value() as f32 / 100.;
        // TODO: implement start pwm
        let mut min_pwm;
        if self.is_spinning() {
            min_pwm = self.start_pwm;
        } else {
            min_pwm = self.start_pwm;
        }
        min_pwm = (min_pwm + 3) * (percentage > 0.0) as u8;
        let pwm_range = 255 - min_pwm;
        let mut pwm_val = percentage * pwm_range as f32;
        pwm_val = f32::max(self.min_pwm as f32, pwm_val);
        let _ = self
            .fan_pwm
            .write_pwm(units::Pwm::from_u8(pwm_val as u8))
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
