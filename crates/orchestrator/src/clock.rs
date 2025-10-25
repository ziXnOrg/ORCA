//! Virtual Time service: deterministic Clock trait + implementations (RED phase stubs)

use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Clock abstraction for deterministic time in orchestrator control paths.
/// Returns milliseconds since UNIX epoch.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
}

/// System (production) clock. Wraps SystemTime.
pub struct SystemClock;

impl Default for SystemClock {
    fn default() -> Self {
        SystemClock
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        // RED stub: implemented in GREEN phase
        unimplemented!("SystemClock::now_ms (GREEN phase)");
    }
}

/// Virtual (deterministic/replay) clock with manual control.
pub struct VirtualClock {
    inner: Mutex<u64>,
}

impl VirtualClock {
    /// Create a new virtual clock seeded at start_ms.
    pub fn new(start_ms: u64) -> Self {
        Self { inner: Mutex::new(start_ms) }
    }

    /// Advance the virtual clock by delta_ms.
    pub fn advance_ms(&self, _delta_ms: u64) {
        // RED stub: implemented in GREEN phase
        unimplemented!("VirtualClock::advance_ms (GREEN phase)");
    }

    /// Set the virtual clock to an absolute ms value.
    pub fn set_ms(&self, _value: u64) {
        // RED stub: implemented in GREEN phase
        unimplemented!("VirtualClock::set_ms (GREEN phase)");
    }
}

impl Clock for VirtualClock {
    fn now_ms(&self) -> u64 {
        // RED stub: implemented in GREEN phase
        unimplemented!("VirtualClock::now_ms (GREEN phase)");
    }
}

// Process-wide default clock registry (no external deps; uses std::OnceLock+RwLock)
static PROCESS_CLOCK: OnceLock<RwLock<Arc<dyn Clock>>> = OnceLock::new();

/// Get the current process-wide Clock (Arc clone).
pub fn process_clock() -> Arc<dyn Clock> {
    // RED stub: implemented in GREEN phase
    unimplemented!("process_clock() (GREEN phase)");
}

/// Set/swap the process-wide Clock. Used by tests and replay.
pub fn set_process_clock(_clock: Arc<dyn Clock>) {
    // RED stub: implemented in GREEN phase
    unimplemented!("set_process_clock() (GREEN phase)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn virtual_clock_is_deterministic_red() {
        // RED: This should panic until GREEN implements VirtualClock.
        let clk = VirtualClock::new(1_000);
        assert_eq!(clk.now_ms(), 1_000);
        clk.advance_ms(5);
        assert_eq!(clk.now_ms(), 1_005);
    }

    #[test]
    #[should_panic]
    fn process_clock_can_be_swapped_red() {
        // RED: This should panic until GREEN implements registry.
        let clk = Arc::new(VirtualClock::new(42));
        set_process_clock(clk);
        let now = process_clock().now_ms();
        assert_eq!(now, 42);
    }
}

