#![no_std]
#![feature(asm)]

pub mod x86_intel;
pub use crate::x86_intel::PerfCounter;

use core::fmt;

/// Abstract trait to control performance counters.
pub trait AbstractPerfCounter {
    /// Reset performance counter.
    fn reset(&self) -> Result<(), fmt::Error>;

    /// Start measuring.
    fn start(&self) -> Result<(), fmt::Error>;

    /// Stop measuring.
    fn stop(&self) -> Result<(), fmt::Error>;

    /// Read the counter value.
    fn read(&mut self) -> Result<u64, fmt::Error>;
}
