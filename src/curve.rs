use std::{
    collections::BTreeMap,
    ops::Bound,
    sync::{Arc, Mutex},
};

use pid::Pid;

use crate::{
    common::{ReadableValue, ReadableValueContainer},
    config,
};

use log::debug;

pub type CurveContainer = Arc<Mutex<dyn ReadableValue>>;

//pub trait Curve: ReadableValue {}

pub struct LinearCurve {
    sensor: ReadableValueContainer,
    functions: BTreeMap<i32, (f32, f32)>,
}

//impl Curve for LinearCurve {}

impl LinearCurve {
    pub fn new(sensor: ReadableValueContainer, conf: &config::LinearCurve) -> Self {
        let mut it = conf.steps.iter();
        let mut high = it.next().unwrap();
        let mut low;

        let mut func_map = BTreeMap::new();

        for val in it {
            low = high;
            high = val;
            let x = (low.0, high.0);
            let y = (low.1, high.1);
            let m = (y.1 - y.0) as f32 / (x.1 - x.0) as f32;
            // y = mx + b
            //
            let b = m.mul_add(-*x.0 as f32, *y.0 as f32);
            println!("High {high:?} Low {low:?} m {m} b {b}");
            func_map.insert(*low.0, (m, b));
        }
        Self {
            sensor,
            functions: func_map,
        }
    }
}

impl ReadableValue for LinearCurve {
    fn get_value(&self) -> i32 {
        let input = self.sensor.lock().unwrap().get_value() / 1000;
        let before = self
            .functions
            .range((Bound::Unbounded, Bound::Included(input)))
            .next_back();

        before.map_or(0, |val| {
            let m = val.1 .0;
            let b = val.1 .1;
            let x = input as f32;
            m.mul_add(x, b) as i32
        })
    }
}

pub struct StaticCurve {
    pub value: i32,
}

//impl Curve for StaticCurve {}

impl ReadableValue for StaticCurve {
    fn get_value(&self) -> i32 {
        self.value
    }
}

pub struct MaximumCurve {
    pub sensors: Vec<ReadableValueContainer>,
}

//impl Curve for MaximumCurve {}
impl ReadableValue for MaximumCurve {
    fn get_value(&self) -> i32 {
        let max = self.sensors.iter().max_by(|a, b| {
            let val_a = a.lock().unwrap().get_value();
            let val_b = b.lock().unwrap().get_value();
            val_a.cmp(&val_b)
        });
        max.map_or(0, |val| val.lock().unwrap().get_value())
    }
}

pub struct AverageCurve {
    pub sensors: Vec<ReadableValueContainer>,
}

impl ReadableValue for AverageCurve {
    fn get_value(&self) -> i32 {
        let mut total = 0;
        self.sensors.iter().for_each(|val| {
            total += val.lock().unwrap().get_value();
        });
        total / self.sensors.len() as i32
    }
}

pub struct PidCurve {
    sensor: ReadableValueContainer,
    /// PID is behind a mutex to allow get_value to be immutable self
    pid: Arc<Mutex<Pid<f32>>>,
    last_val: i32,
}

impl PidCurve {
    pub fn new(sensor: ReadableValueContainer, p: f32, i: f32, d: f32, target: f32) -> Self {
        let limit = 100.0;
        let mut pid = Pid::new(target, limit);
        pid.p(p, limit);
        pid.i(i, limit);
        pid.d(d, limit);
        Self {
            sensor,
            pid: Arc::new(Mutex::new(pid)),
            last_val: 0,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.pid.lock().unwrap().setpoint(target);
    }
}

impl ReadableValue for PidCurve {
    fn update_value(&mut self) {
        let input = self.sensor.lock().unwrap().get_value() as f32 / 1000.0;
        let output = self.pid.lock().unwrap().next_control_output(input);

        let retval = if output.output < 0.0 {
            -(output.output as i32)
        } else {
            0
        };

        debug!(
            "Pid {:?} with input {input} and target {} results in {retval}",
            output,
            self.pid.lock().unwrap().setpoint
        );

        self.last_val = retval;
    }

    fn get_value(&self) -> i32 {
        self.last_val
    }
}

#[cfg(test)]
mod test {
    use more_asserts::assert_gt;

    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_curve_linear() {
        let static_sensor = Arc::new(Mutex::new(StaticCurve { value: 0 }));
        let mut curve_steps = BTreeMap::new();
        curve_steps.insert(0, 10);
        curve_steps.insert(100, 110);
        curve_steps.insert(200, 310);
        let curve_conf = config::LinearCurve {
            sensor: "test".to_string(),
            steps: curve_steps,
        };
        let linear_curve = LinearCurve::new(static_sensor.clone(), &curve_conf);

        // Test curve parameter
        let curve_functions = &linear_curve.functions;
        assert_eq!(curve_functions.len(), 2);
        assert_eq!(curve_functions[&0].0, 1.0);
        assert_eq!(curve_functions[&0].1, 10.0);
        assert_eq!(curve_functions[&100].0, 2.0);
        assert_eq!(curve_functions[&100].1, -90.0);

        // Test acutal values
        assert_eq!(linear_curve.get_value(), 10);
        static_sensor.lock().unwrap().value = 100 * 1000;
        assert_eq!(linear_curve.get_value(), 110);
        static_sensor.lock().unwrap().value = 150 * 1000;
        assert_eq!(linear_curve.get_value(), 210);
    }

    #[test]
    fn test_curve_max() {
        let static_sensor_low = Arc::new(Mutex::new(StaticCurve { value: 10 }));
        let static_sensor_mid = Arc::new(Mutex::new(StaticCurve { value: 50 }));
        let static_sensor_high = Arc::new(Mutex::new(StaticCurve { value: 100 }));
        let sensors: Vec<Arc<Mutex<dyn ReadableValue>>> =
            vec![static_sensor_low, static_sensor_mid, static_sensor_high];
        let max_curve = MaximumCurve { sensors };

        assert_eq!(max_curve.get_value(), 100);
    }

    #[test]
    fn test_curve_avg() {
        let static_sensor_low = Arc::new(Mutex::new(StaticCurve { value: 10 }));
        let static_sensor_mid = Arc::new(Mutex::new(StaticCurve { value: 50 }));
        let static_sensor_high = Arc::new(Mutex::new(StaticCurve { value: 100 }));
        let sensors: Vec<Arc<Mutex<dyn ReadableValue>>> =
            vec![static_sensor_low, static_sensor_mid, static_sensor_high];
        let avg_curve = AverageCurve { sensors };

        assert_eq!(avg_curve.get_value(), 53);
    }

    #[test]
    fn test_curve_pid() {
        let static_sensor = Arc::new(Mutex::new(StaticCurve { value: 10 }));
        let mut pid_curve = PidCurve::new(static_sensor.clone(), 1.0, 1.0, 1.0, 0.0);
        pid_curve.update_value();

        assert_eq!(pid_curve.get_value(), 0);
        static_sensor.lock().unwrap().value = 100 * 1000;
        pid_curve.update_value();
        assert_eq!(pid_curve.get_value(), 100);

        pid_curve.set_target(50.0);
        static_sensor.lock().unwrap().value = 10 * 1000;
        pid_curve.update_value();
        assert_eq!(pid_curve.get_value(), 0);

        static_sensor.lock().unwrap().value = 49 * 1000;
        pid_curve.update_value();
        assert_gt!(pid_curve.get_value(), 0);

        static_sensor.lock().unwrap().value = 51 * 1000;
        pid_curve.update_value();
        assert_gt!(pid_curve.get_value(), 0);
    }
}
