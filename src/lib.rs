#![no_std]
#![feature(asm)]

pub mod x86_intel;
pub use crate::x86_intel::PerfCounter;
pub use crate::x86_intel::ErrorMsg;

/// Abstract trait to control performance counters.
pub trait AbstractPerfCounter {
    /// Reset performance counter.
    fn reset(&self) -> Result<(), ErrorMsg>;

    /// Start measuring.
    fn start(&self) -> Result<(), ErrorMsg>;

    /// Stop measuring.
    fn stop(&self) -> Result<(), ErrorMsg>;

    /// Read the counter value.
    fn read(&mut self) -> Result<u64, ErrorMsg>;
}
