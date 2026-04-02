use crate::{Hypervisor, HypervisorConfig};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Hypervisor CLI Arguments
#[derive(Debug, Parser)]
#[command(name = "nyx-hypervisor", about = "Nyx Virtual Machine Monitor")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start a new Virtual Machine
    Run {
        /// Kernel ELF to load (optional if BIOS is used)
        #[arg(short, long)]
        kernel: Option<PathBuf>,

        /// Number of CPUs (default: 1)
        #[arg(short, long, default_value_t = 1)]
        cpus: usize,

        /// Memory Allocation in MB (default: 512)
        #[arg(short, long, default_value_t = 512)]
        memory: u64,

        /// Architecture to emulate (x86_64, aarch64, riscv64) (default: x86_64)
        #[arg(short, long, default_value = "x86_64")]
        arch: String,

        /// ISO/CD-ROM image to mount
        #[arg(short, long)]
        iso: Option<PathBuf>,

        /// Enable KVM hardware acceleration
        #[arg(long)]
        accel: bool,
    },

    /// List running VMs
    List,
}

impl Cli {
    /// Execute the hypervisor command
    pub fn execute(&self) -> Result<(), String> {
        match &self.command {
            Command::Run {
                kernel,
                cpus,
                memory,
                arch,
                accel,
                iso,
            } => {
                let config = HypervisorConfig {
                    num_cpus: *cpus,
                    memory_mb: *memory,
                    kernel_path: kernel
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    iso_path: iso
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    architecture: arch.clone(),
                    accel: *accel,
                };

                let hv = Hypervisor::new(config).map_err(|e| e.to_string())?;
                hv.run().map_err(|e| e.to_string())?;
                Ok(())
            }
            Command::List => {
                println!("No running VMs found.");
                Ok(())
            }
        }
    }
}
