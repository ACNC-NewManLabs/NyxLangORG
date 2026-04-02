use crate::runtime::database::distributed::NetworkPool;
use rand::Rng;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};

pub struct ChaosMonkey {
    pub intensity: f64, // 0.0 to 1.0
}

impl ChaosMonkey {
    pub fn new(intensity: f64) -> Self {
        Self { intensity }
    }

    /// Randomly corrupts a file by flipping bits.
    pub fn corrupt_file(&self, path: &str) -> std::io::Result<()> {
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;
        let metadata = file.metadata()?;
        let len = metadata.len();

        let mut rng = rand::thread_rng();
        let num_corruptions = (len as f64 * self.intensity * 0.01).max(1.0) as usize;

        for _ in 0..num_corruptions {
            let pos = rng.gen_range(0..len);
            let mut byte = [0u8; 1];
            file.seek(SeekFrom::Start(pos))?;
            file.read_exact(&mut byte)?;
            byte[0] ^= 0xFF; // Flip all bits in the byte
            file.seek(SeekFrom::Start(pos))?;
            file.write_all(&byte)?;
        }

        file.sync_all()?;
        Ok(())
    }

    /// Simulates a network partition by dropping a specific node from the pool.
    pub fn simulate_partition(&self, pool: &NetworkPool, node_id: u32) {
        let mut conns = pool.connections.lock().unwrap();
        if conns.remove(&node_id).is_some() {
            println!("[Chaos] Partitioned node {} from the cluster", node_id);
        }
    }

    /// Randomly drops all connections to simulate a total network failure.
    pub fn simulate_total_blackout(&self, pool: &NetworkPool) {
        let mut conns = pool.connections.lock().unwrap();
        conns.clear();
        println!("[Chaos] Total network blackout simulated");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_file_corruption_resilience() {
        let path = "nyx_data/chaos_test.bin";
        {
            let mut f = File::create(path).unwrap();
            f.write_all(b"LEGITIMATE-DATA-12345").unwrap();
        }

        let monkey = ChaosMonkey::new(0.5);
        monkey.corrupt_file(path).unwrap();

        let mut f = File::open(path).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        // Data should be different now
        assert_ne!(buf, b"LEGITIMATE-DATA-12345");

        std::fs::remove_file(path).unwrap();
    }
}
