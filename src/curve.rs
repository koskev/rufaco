use std::{
    collections::BTreeMap,
    ops::Bound,
    sync::{Arc, Mutex},
};

use crate::{
    common::{ReadableValue, ReadableValueContainer},
    config,
};

pub type CurveContainer = Arc<Mutex<dyn Curve>>;

pub trait Curve: ReadableValue {}

pub struct LinearCurve {
    sensor: ReadableValueContainer,
    functions: BTreeMap<i32, (f32, f32)>,
}

impl Curve for LinearCurve {}

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
