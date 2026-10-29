use std::time::{Duration, Instant};
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransferState {
    #[default]
    Connecting,
    AwaitingApproval,
    Transferring,
    Completed,
    Failed,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub transfer_id: String,
    pub state: TransferState,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub files_done: u32,
    pub files_total: u32,
    pub current_file: Option<String>,
}

/// Publishes progress through a `watch` channel, coalescing byte updates so a
/// UI sees at most ~10 updates per second regardless of chunk rate.
pub struct ProgressReporter {
    tx: watch::Sender<Progress>,
    cur: Progress,
    last_publish: Instant,
}

const PUBLISH_INTERVAL: Duration = Duration::from_millis(100);

impl ProgressReporter {
    pub fn new() -> (Self, watch::Receiver<Progress>) {
        let (tx, rx) = watch::channel(Progress::default());
        (
            Self {
                tx,
                cur: Progress::default(),
                last_publish: Instant::now(),
            },
            rx,
        )
    }

    pub fn init(&mut self, transfer_id: &str, bytes_total: u64, files_total: u32) {
        self.cur.transfer_id = transfer_id.to_owned();
        self.cur.bytes_total = bytes_total;
        self.cur.files_total = files_total;
        self.publish();
    }

    pub fn set_state(&mut self, state: TransferState) {
        self.cur.state = state;
        self.publish();
    }

    pub fn set_bytes(&mut self, done: u64) {
        self.cur.bytes_done = done;
        self.publish_throttled();
    }

    pub fn add_bytes(&mut self, n: u64) {
        self.cur.bytes_done = self.cur.bytes_done.saturating_add(n);
        self.publish_throttled();
    }

    pub fn begin_file(&mut self, name: &str) {
        self.cur.current_file = Some(name.to_owned());
        self.publish();
    }

    pub fn file_done(&mut self) {
        self.cur.files_done += 1;
        self.publish();
    }

    pub fn bytes_done(&self) -> u64 {
        self.cur.bytes_done
    }

    fn publish_throttled(&mut self) {
        if self.last_publish.elapsed() >= PUBLISH_INTERVAL {
            self.publish();
        }
    }

    pub fn publish(&mut self) {
        self.last_publish = Instant::now();
        // send_replace never fails, even when no receiver is listening.
        self.tx.send_replace(self.cur.clone());
    }
}

/// Exponentially smoothed throughput measured from real byte counts.
#[derive(Debug)]
pub struct SpeedMeter {
    last: Option<(Instant, u64)>,
    bytes_per_sec: f64,
}

impl Default for SpeedMeter {
    fn default() -> Self {
        Self {
            last: None,
            bytes_per_sec: 0.0,
        }
    }
}

impl SpeedMeter {
    const ALPHA: f64 = 0.3;

    /// Feed the cumulative byte count; returns smoothed bytes/sec.
    pub fn sample(&mut self, now: Instant, bytes_done: u64) -> f64 {
        if let Some((t0, b0)) = self.last {
            let dt = now.duration_since(t0).as_secs_f64();
            if dt >= 0.05 {
                let inst = bytes_done.saturating_sub(b0) as f64 / dt;
                self.bytes_per_sec = if self.bytes_per_sec == 0.0 {
                    inst
                } else {
                    Self::ALPHA * inst + (1.0 - Self::ALPHA) * self.bytes_per_sec
                };
                self.last = Some((now, bytes_done));
            }
        } else {
            self.last = Some((now, bytes_done));
        }
        self.bytes_per_sec
    }

    pub fn eta(&self, remaining: u64) -> Option<Duration> {
        (self.bytes_per_sec > 1.0)
            .then(|| Duration::from_secs_f64(remaining as f64 / self.bytes_per_sec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_meter_measures_real_rate() {
        let mut m = SpeedMeter::default();
        let t0 = Instant::now();
        m.sample(t0, 0);
        let v = m.sample(t0 + Duration::from_secs(1), 10_000_000);
        assert!((v - 10_000_000.0).abs() < 1.0);
        assert_eq!(m.eta(20_000_000).unwrap().as_secs(), 2);
    }

    #[test]
    fn reporter_coalesces_but_publishes_state_changes() {
        let (mut r, rx) = ProgressReporter::new();
        r.init("t", 100, 1);
        for _ in 0..50 {
            r.add_bytes(1);
        }
        assert!(rx.borrow().bytes_done < 50, "byte updates are throttled");
        r.set_state(TransferState::Completed);
        assert_eq!(
            rx.borrow().bytes_done,
            50,
            "state change flushes latest bytes"
        );
    }
}
