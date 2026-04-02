use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

/// Actor-based Concurrency for Nyx, effectively mimicking Erlang OTP processes and Go routines.
/// Replaces traditional lock-heavy shared state with message-passing isolated scopes.
pub struct Actor<T: Send + 'static> {
    pub mailbox_sender: Sender<T>,
}

impl<T: Send + 'static> Actor<T> {
    /// Spawns a lightweight Green-Thread proxy mapping to an OS thread with a private mailbox
    pub fn spawn<F>(handler: F) -> Self 
    where 
        F: FnOnce(Receiver<T>) + Send + 'static 
    {
        let (tx, rx) = channel();
        thread::spawn(move || {
            handler(rx);
        });

        Self {
            mailbox_sender: tx,
        }
    }

    /// Fire-and-forget message passing
    pub fn send(&self, message: T) {
        let _ = self.mailbox_sender.send(message);
    }
}
