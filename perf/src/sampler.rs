//! The out-of-band metrics thread.
//!
//! RSS and process CPU% are both read from `/proc`, and `sysinfo`'s process
//! refresh in particular is far too expensive to sit in a 33 ms budget -- doing
//! it inline would put the observer squarely inside the observation. So it runs
//! on its own thread and posts results down a channel that the frame loop
//! drains without ever blocking on it. If the sampler stalls, the animation
//! does not; the dashboard just shows a slightly stale number, which is the
//! right trade for a metric that moves on the order of seconds anyway.
//!
//! Heap counters are deliberately *not* sampled here. Those are three relaxed
//! atomic loads (see [`crate::alloc`]), cheap enough to read directly in the
//! frame loop, and reading them there means they line up exactly with the frame
//! whose particle count is displayed beside them.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// How often RSS is read. Cheap (`/proc/self/statm` is one small read), so it
/// can be brisk enough to make the sparkline look live.
const RSS_PERIOD: Duration = Duration::from_millis(100);

/// CPU% is refreshed every Nth RSS tick. `sysinfo` needs a minimum interval
/// between refreshes to compute a rate at all, and 400 ms clears
/// `MINIMUM_CPU_UPDATE_INTERVAL` comfortably while staying responsive.
const CPU_EVERY: u32 = 4;

/// One reading from the sampler thread.
#[derive(Clone, Copy, Default)]
pub struct Sample {
    /// Resident set size in bytes: pages the kernel actually has backing us.
    pub rss_bytes: u64,
    /// Whole-process CPU, as a percentage of one core. Two threads flat out
    /// would read ~200, so this is not clamped to 100.
    pub cpu_pct: f32,
}

/// A running sampler. Dropping it drops the receiver, which makes the thread's
/// next send fail and the thread exit on its own -- no shutdown flag needed.
pub struct Sampler {
    rx: Receiver<Sample>,
    latest: Sample,
}

impl Sampler {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let pid = sysinfo::get_current_pid().ok();
            let mut sys = System::new();
            let mut sample = Sample::default();
            let mut tick: u32 = 0;

            loop {
                if let Some(m) = memory_stats::memory_stats() {
                    sample.rss_bytes = m.physical_mem as u64;
                }
                if tick.is_multiple_of(CPU_EVERY) {
                    sample.cpu_pct = cpu_percent(&mut sys, pid).unwrap_or(sample.cpu_pct);
                }
                tick = tick.wrapping_add(1);

                if tx.send(sample).is_err() {
                    return;
                }
                thread::sleep(RSS_PERIOD);
            }
        });

        Self {
            rx,
            latest: Sample::default(),
        }
    }

    /// Drain everything queued and return the newest reading. Never blocks: a
    /// frame that arrives between samples simply re-displays the last one.
    pub fn poll(&mut self) -> Sample {
        loop {
            match self.rx.try_recv() {
                Ok(s) => self.latest = s,
                Err(TryRecvError::Empty) => return self.latest,
                Err(TryRecvError::Disconnected) => return self.latest,
            }
        }
    }
}

fn cpu_percent(sys: &mut System, pid: Option<Pid>) -> Option<f32> {
    let pid = pid?;
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_cpu(),
    );
    Some(sys.process(pid)?.cpu_usage())
}
