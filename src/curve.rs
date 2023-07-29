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
            let b = *y.0 as f32 - m * *x.0 as f32;
            println!("High {high:?} Low {low:?} m {m} b {b}");
            func_map.insert(*low.0, (m, b));
        }
        Self {
            sensor,
            functions: func_map,
        }
    }

    fn get_neighbors<T>(
        val: i32,
        map: &BTreeMap<i32, T>,
    ) -> (Option<(&i32, &T)>, Option<(&i32, &T)>) {
        let mut before = map.range((Bound::Unbounded, Bound::Included(val)));
        let mut after = map.range((Bound::Excluded(val), Bound::Unbounded));

        (before.next_back(), after.next())
    }
}

impl ReadableValue for LinearCurve {
    fn get_value(&self) -> i32 {
        let input = self.sensor.lock().unwrap().get_value() / 1000;
        let before = self
            .functions
            .range((Bound::Unbounded, Bound::Included(input)))
            .next_back();

        match before {
            Some(val) => {
                let m = val.1 .0;
                let b = val.1 .1;
                let x = input as f32;
                return (m * x + b) as i32;
            }
            None => return 0,
        }
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
        let x = match max {
            Some(val) => val.lock().unwrap().get_value(),
            None => 0,
        };
        x
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
}

impl ReadableValue for PidCurve {
    fn update_value(&mut self) {
        let input = self.sensor.lock().unwrap().get_value() as f32 / 1000.0;
        let output = self.pid.lock().unwrap().next_control_output(input);
        let mut retval = 0;

        if output.output < 0.0 {
            retval = output.output as i32 * -1;
        }

        debug!(
            "Pid {:?} with input {input} and target {} results in {retval}",
            output,
            self.pid.lock().unwrap().setpoint
        );

        self.last_val = retval;
    }

    fn get_value(&self) -> i32 {
        self.last_val as i32
    }
}
