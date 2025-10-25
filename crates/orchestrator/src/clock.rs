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
        let dur = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH");
        dur.as_millis() as u64
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
    pub fn advance_ms(&self, delta_ms: u64) {
        let mut t = self.inner.lock().expect("virtual clock poisoned");
        *t = t.saturating_add(delta_ms);
    }

    /// Set the virtual clock to an absolute ms value.
    pub fn set_ms(&self, value: u64) {
        let mut t = self.inner.lock().expect("virtual clock poisoned");
        *t = value;
    }
}

impl Clock for VirtualClock {
    fn now_ms(&self) -> u64 {
        *self.inner.lock().expect("virtual clock poisoned")
    }
}

// Process-wide default clock registry (no external deps; uses std::OnceLock+RwLock)
static PROCESS_CLOCK: OnceLock<RwLock<Arc<dyn Clock>>> = OnceLock::new();

/// Get the current process-wide Clock (Arc clone).
pub fn process_clock() -> Arc<dyn Clock> {
    let lock = PROCESS_CLOCK.get_or_init(|| RwLock::new(Arc::new(SystemClock)));
    let guard = lock.read().expect("process clock poisoned");
    Arc::clone(&*guard)
}

/// Set/swap the process-wide Clock. Used by tests and replay.
pub fn set_process_clock(clock: Arc<dyn Clock>) {
    let lock = PROCESS_CLOCK.get_or_init(|| RwLock::new(Arc::new(SystemClock)));
    let mut guard = lock.write().expect("process clock poisoned");
    *guard = clock;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_clock_is_deterministic() {
        let clk = VirtualClock::new(1_000);
        assert_eq!(clk.now_ms(), 1_000);
        clk.advance_ms(5);
        assert_eq!(clk.now_ms(), 1_005);
        clk.set_ms(2_000);
        assert_eq!(clk.now_ms(), 2_000);
    }

    #[test]
    fn process_clock_can_be_swapped() {
        // Save current and restore at end to avoid cross-test contamination
        let original = process_clock();
        let clk = Arc::new(VirtualClock::new(42));
        set_process_clock(clk);
        assert_eq!(process_clock().now_ms(), 42);
        // Restore
        set_process_clock(original);
    }
}

