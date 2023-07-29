use std::sync::{Arc, Mutex};

pub trait ReadableValue {
    fn get_value(&self) -> i32;
    fn update_value(&mut self) {}
}

pub trait UpdatableInput {
    fn update_input(&mut self);
}

pub trait UpdatableOutput {
    fn update_output(&mut self);
}

pub type ReadableValueContainer = Arc<Mutex<dyn ReadableValue>>;
