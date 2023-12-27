use std::{
    collections::VecDeque,
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{self, Duration},
};

use libmedium::{
    sensors::{fan::WriteableFanSensor, pwm::WriteablePwmSensor},
    units::{self, AngularVelocity},
};
use log::*;

use crate::{
    common::{ReadableValue, UpdatableInput, UpdatableOutput},
    config::FanConfig,
    curve::CurveContainer,
};

#[cfg(test)]
use mockall::automock;

/// Mutex containing a [FanSensor]
pub type FanContainer = Arc<Mutex<FanSensor>>;

pub struct HwmonFan {
    pub fan_input: Box<dyn WriteableFanSensor>,
}

pub struct HwmonPwm {
    pub fan_pwm: Box<dyn WriteablePwmSensor>,
}

#[cfg_attr(test, automock)]
pub trait FanInput {
    fn get_input(&self) -> Result<AngularVelocity, Box<dyn Error>>;
}

pub trait FanOutput {
    /// Set the fan output to pwm
    fn set_output(&mut self, pwm: u8);

    /// Get the currently set value
    fn get_output(&self) -> u8;
}

impl FanOutput for HwmonPwm {
    fn set_output(&mut self, pwm: u8) {
        self.fan_pwm.write_pwm(units::Pwm::from_u8(pwm)).unwrap();
    }

    fn get_output(&self) -> u8 {
        self.fan_pwm.read_pwm().unwrap().as_u8()
    }
}

impl FanInput for HwmonFan {
    fn get_input(&self) -> Result<AngularVelocity, Box<dyn Error>> {
        let val: Result<AngularVelocity, libmedium::sensors::Error> = self.fan_input.read_input();
        match val {
            Ok(speed) => Ok(speed),
            Err(err) => Err(Box::new(err)),
        }
    }
}

pub struct FanSensor {
    pub id: String,
    /// The fan input sensor
    pub fan_input: Box<dyn FanInput>,
    /// The pwm output of the fan
    pub fan_pwm: Box<dyn FanOutput>,
    pub curve: CurveContainer,
    pub last_val: u32,
    /// Min PWM to keep the fan spinning
    pub min_pwm: u8,
    /// PWM to start the fan
    pub start_pwm: u8,

    /// Value the fan should start spinning at. Fan still spins below this value if it was spinning
    /// previously
    pub start_percent: f32,
    /// Time the fan is at 0%. Used to prevent spin up and down loop
    zero_percent_time: Option<time::Instant>,
}

impl FanSensor {
    pub fn new(
        conf: &FanConfig,
        fan_input: Box<dyn FanInput>,
        fan_pwm: Box<dyn FanOutput>,
        curve: CurveContainer,
    ) -> Self {
        let min_pwm = conf.minpwm.unwrap_or(0);
        let start_pwm = conf.startpwm.unwrap_or(0);
        Self {
            id: conf.id.clone(),
            fan_input,
            fan_pwm,
            curve,
            last_val: 0,
            min_pwm,
            start_pwm,
            start_percent: 20.0,
            zero_percent_time: None,
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
        self.fan_pwm.set_output(pwm);
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
        Some(mean)
    }

    pub fn measure_fan(
        &mut self,
        wait_time: Duration,
        max_rpm_diff: i32,
        stop_signal: Arc<AtomicBool>,
    ) -> Option<(u8, u8)> {
        debug!("Measuring fan {}", self.id);
        let mut max_pwm = 255u8;
        let mut min_pwm = 0u8;
        // stop fan to actually measure start pwm
        let rpm = self
            .measure_pwm(0, max_rpm_diff, wait_time, stop_signal.clone())
            .unwrap();
        // Fan does not stop
        if rpm != 0 {
            max_pwm = 0;
            min_pwm = 0;
        }
        loop {
            // If they are one apart min has 0 rpm and max has the start rpm
            if max_pwm - min_pwm <= 1 {
                break;
            }
            let pwm = (max_pwm - min_pwm) / 2 + min_pwm;
            debug!("max_pwm {max_pwm} min_pwm {min_pwm}");
            debug!("##Setting fan {} to {}", self.id, pwm);
            let rpm = self.measure_pwm(pwm, max_rpm_diff, wait_time, stop_signal.clone());
            match rpm {
                Some(rpm) => {
                    debug!("Settled rpm {rpm} with pwm {pwm}");
                    if rpm != 0 {
                        max_pwm = pwm;
                        debug!("Rpm is != 0. Found new lowest start pwm: {pwm}");
                        self.start_pwm = pwm;
                        // stop fan to actually measure start pwm
                        self.measure_pwm(0, max_rpm_diff, wait_time, stop_signal.clone());
                    } else {
                        min_pwm = pwm;
                    }
                }
                // SIGINT received
                None => return None,
            }
        }
        debug!("Found start pwm: {max_pwm}");
        // Set to 0 in case the fan never stops
        self.min_pwm = 0;
        debug!("Finding min_pwm for {}", self.id);
        // Measure min pwm
        self.measure_pwm(255, max_rpm_diff, wait_time, stop_signal.clone())
            .unwrap();
        for pwm in (0..self.start_pwm + 3).rev() {
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
        let mut min_pwm = if self.is_spinning() {
            self.min_pwm
        } else {
            self.start_pwm
        };

        // add 3 for safety
        //min_pwm = min_pwm + 3;

        if percentage < self.start_percent && self.get_value() == 0 {
            // set to 0 if the fan is not spinning and we are below start_percent
            min_pwm = 0;
        }

        if percentage < 0.1 {
            // fan is currenlty on. We don't use is spnning as we might be faster than the motor
            match self.zero_percent_time {
                Some(time) => {
                    if time::Instant::now() - time > time::Duration::from_secs(10) {
                        min_pwm = 0;
                    }
                }
                None => self.zero_percent_time = Some(time::Instant::now()),
            }
        } else {
            self.zero_percent_time = None;
        }
        let pwm_range = 255 - min_pwm;
        let pwm_val = percentage.mul_add(pwm_range as f32, min_pwm as f32);
        self.fan_pwm.set_output(pwm_val as u8);
        trace!(
            "Got value {percentage} for fan {} pwm {pwm_val} min pwm {min_pwm}",
            self.id,
        );
    }
}

impl UpdatableInput for FanSensor {
    fn update_input(&mut self) {
        let val: Result<AngularVelocity, Box<dyn Error>> = self.fan_input.get_input();
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

#[cfg(test)]
mod test {
    use more_asserts::{assert_ge, assert_le};

    use crate::curve::StaticCurve;

    use super::*;

    struct DummyPwm {
        last_val: u8,
    }

    impl FanOutput for DummyPwm {
        fn set_output(&mut self, pwm: u8) {
            self.last_val = pwm;
        }
        fn get_output(&self) -> u8 {
            self.last_val
        }
    }

    fn init() -> (FanSensor, Arc<Mutex<u32>>, Arc<Mutex<StaticCurve>>) {
        let static_sensor = Arc::new(Mutex::new(StaticCurve { value: 0 }));
        let fan_input_val = Arc::new(Mutex::new(0u32));
        let fan_input_val_2 = fan_input_val.clone();
        let mut fan_input = Box::new(MockFanInput::new());
        fan_input.expect_get_input().returning(move || {
            let val = AngularVelocity::from_rpm(fan_input_val.lock().unwrap().clone());
            Ok(val)
        });
        let fan = FanSensor {
            id: "test_sensor".to_string(),
            min_pwm: 21,
            start_pwm: 42,
            last_val: 0,
            curve: static_sensor.clone(),
            fan_pwm: Box::new(DummyPwm { last_val: 0 }),
            fan_input,
            start_percent: 5.0,
            zero_percent_time: None,
        };
        (fan, fan_input_val_2, static_sensor)
    }

    #[test]
    fn test_spinning() {
        let (mut fan, fan_input_val, _static_sensor) = init();
        fan.update_input();
        assert_eq!(fan.get_value(), 0);
        assert!(!fan.is_spinning());
        *fan_input_val.lock().unwrap() = 4242;
        fan.update_input();
        assert_eq!(fan.get_value(), 4242);
        assert!(fan.is_spinning());
    }

    #[test]
    fn test_fan() {
        let (mut fan, fan_input_val, static_sensor) = init();

        fan.update_output();
        assert_eq!(fan.fan_pwm.get_output(), 0);
        static_sensor.lock().unwrap().value = 100;
        fan.update_output();
        assert_eq!(fan.fan_pwm.get_output(), 255);
        static_sensor.lock().unwrap().value = 0;
        *fan_input_val.lock().unwrap() = 0;
        fan.update_output();
        assert_eq!(fan.fan_pwm.get_output(), 0);
        assert_eq!(fan.get_value(), 0);
        static_sensor.lock().unwrap().value = 1;
        fan.update_output();
        assert_le!(fan.fan_pwm.get_output(), 42);
        *fan_input_val.lock().unwrap() = 4242;
        fan.update_input();
        fan.update_output();
        assert_le!(fan.fan_pwm.get_output(), 42);
        assert_ge!(fan.fan_pwm.get_output(), 21);
    }
}
