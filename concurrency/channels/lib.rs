use std::sync::mpsc::{self, Receiver, Sender};

pub fn bounded<T>() -> (Sender<T>, Receiver<T>) {
    mpsc::channel()
}
