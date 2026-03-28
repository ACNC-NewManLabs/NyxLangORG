//! NYX Channels Module

use std::sync::mpsc::{self, Receiver, Sender};

pub struct Channel<T> {
    sender: Sender<T>,
    receiver: Receiver<T>,
}

impl<T> Channel<T> {
    pub fn new() -> Channel<T> {
        let (sender, receiver) = mpsc::channel();
        Channel { sender, receiver }
    }

    pub fn sender(&self) -> Sender<T> {
        self.sender.clone()
    }

    pub fn receiver(&self) -> &Receiver<T> {
        &self.receiver
    }
}
