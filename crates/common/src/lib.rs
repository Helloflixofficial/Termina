//! Types shared by every Lichii crate.

pub mod paths;
pub mod perf;
pub mod result;
pub mod settings;
pub mod thread;

pub use result::{HleResult, ResultCode};

/// Guest virtual address (process address space).
pub type VAddr = u64;
/// Guest physical address (DRAM offset as seen by the kernel).
pub type PAddr = u64;
/// GPU virtual address (GMMU address space of a channel).
pub type GpuVAddr = u64;
/// Device address (SMMU address space shared by the GPU and other devices).
pub type DAddr = u64;

pub const PAGE_BITS: u32 = 12;
pub const PAGE_SIZE: u64 = 1 << PAGE_BITS;
pub const PAGE_MASK: u64 = PAGE_SIZE - 1;

#[inline]
pub const fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

#[inline]
pub const fn align_up(value: u64, align: u64) -> u64 {
    (value + align - 1) & !(align - 1)
}

/// Which console a title targets. Switch 2 titles have program ids starting with `04`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ConsoleGeneration {
    Switch1,
    Switch2,
}

impl ConsoleGeneration {
    pub fn from_program_id(program_id: u64) -> Self {
        if program_id >> 56 == 0x04 {
            Self::Switch2
        } else {
            Self::Switch1
        }
    }

    /// Number of CPU cores visible to applications.
    pub fn core_count(self) -> usize {
        match self {
            Self::Switch1 => 4,
            Self::Switch2 => 6,
        }
    }
}

/// Host physical RAM threshold (10 GiB) below which background worker pools, shader prebuilds,
/// and transient staging caps scale down to prevent Windows paging stutter.
pub const CONSTRAINED_HOST_RAM_BYTES: u64 = 10 << 30;

/// Returns total physical host memory in bytes when available from the OS.
pub fn host_ram_bytes() -> Option<u64> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut status = MEMORYSTATUSEX {
            dwLength: size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        // SAFETY: `status` is a local struct whose `dwLength` field is initialized above.
        if unsafe { GlobalMemoryStatusEx(&mut status) } != 0 && status.ullTotalPhys > 0 {
            return Some(status.ullTotalPhys);
        }
        None
    }
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
        let kib = line.split_whitespace().nth(1)?.parse::<u64>().ok()?;
        Some(kib * 1024)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

/// Whether this machine has constrained physical RAM (<= 10 GiB usable, e.g. 8 GB laptops).
pub fn is_constrained_host_ram() -> bool {
    host_ram_bytes().is_some_and(|bytes| bytes <= CONSTRAINED_HOST_RAM_BYTES)
}

/// Installs the global tracing subscriber. `filter` uses `EnvFilter` syntax; `RUST_LOG` wins when set.
pub fn init_logging(filter: &str) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_from_program_id() {
        assert_eq!(
            ConsoleGeneration::from_program_id(0x0400C3F00006E000),
            ConsoleGeneration::Switch2
        );
        assert_eq!(
            ConsoleGeneration::from_program_id(0x0100152000022000),
            ConsoleGeneration::Switch1
        );
    }

    #[test]
    fn alignment() {
        assert_eq!(align_up(0x1001, PAGE_SIZE), 0x2000);
        assert_eq!(align_down(0x1fff, PAGE_SIZE), 0x1000);
    }
}
