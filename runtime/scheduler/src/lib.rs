// Copyright (c) 2026 SURYA SEKHAR ROY. All Rights Reserved.
// Nyx Industrial Task Scheduler™
use std::collections::VecDeque;
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

pub trait Task {
    fn poll(&mut self) -> bool;
    fn priority(&self) -> Priority {
        Priority::Normal
    }
}

pub struct CooperativeScheduler {
    queues: [VecDeque<Box<dyn Task>>; 4],
}

impl CooperativeScheduler {
    pub fn new() -> Self {
        Self {
            queues: [
                VecDeque::new(), // Low
                VecDeque::new(), // Normal
                VecDeque::new(), // High
                VecDeque::new(), // Critical
            ],
        }
    }

    pub fn push(&mut self, task: Box<dyn Task>) {
        let p = task.priority() as usize;
        self.queues[p].push_back(task);
    }

    pub fn run(&mut self) {
        loop {
            let mut executed = false;
            // Iterate from Critical (3) down to Low (0)
            for i in (0..4).rev() {
                if let Some(mut task) = self.queues[i].pop_front() {
                    if !task.poll() {
                        let p = task.priority() as usize;
                        self.queues[p].push_back(task);
                    }
                    executed = true;
                    break; // Execute one task and re-check priorities
                }
            }
            if !executed {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTask(u32);
    impl Task for MockTask {
        fn poll(&mut self) -> bool {
            if self.0 > 0 {
                self.0 -= 1;
                false
            } else {
                true
            }
        }
    }

    #[test]
    fn test_scheduling() {
        let mut scheduler = CooperativeScheduler::new();
        scheduler.push(Box::new(MockTask(2)));
        scheduler.push(Box::new(MockTask(1)));
        scheduler.run();
    }
}
