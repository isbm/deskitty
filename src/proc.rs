use std::fs;
use std::time::{Duration, Instant};

pub struct ProcStat {
    pid: u32,
    prev_total: u64,
    last_active: Instant,
}

impl ProcStat {
    pub fn new(pid: u32) -> Self {
        Self { pid, prev_total: 0, last_active: Instant::now().checked_sub(Duration::from_secs(3600)).unwrap_or_else(Instant::now) }
    }

    pub fn poll(&mut self) -> Option<(u64, u64)> {
        let stat = fs::read_to_string(format!("/proc/{}/stat", self.pid)).ok()?;
        let after_paren = stat.rsplit(')').next()?;
        let mut fields = after_paren.split_whitespace();
        let _state = fields.next()?;
        let _ppid = fields.next()?;
        let _pgrp = fields.next()?;
        let _session = fields.next()?;
        let _tty_nr = fields.next()?;
        let _tpgid = fields.next()?;
        let _flags = fields.next()?;
        let _minflt = fields.next()?;
        let _cminflt = fields.next()?;
        let _majflt = fields.next()?;
        let _cmajflt = fields.next()?;
        let utime: u64 = fields.next()?.parse().ok()?;
        let stime: u64 = fields.next()?.parse().ok()?;
        let cutime: u64 = fields.next()?.parse().ok()?;
        let cstime: u64 = fields.next()?.parse().ok()?;

        let total = utime + stime + cutime + cstime;
        let delta = total.saturating_sub(self.prev_total);
        self.prev_total = total;
        if delta >= 3 {
            self.last_active = Instant::now();
        }
        Some((total, delta))
    }

    pub fn is_active(&self, idle_timeout: Duration) -> bool {
        self.last_active.elapsed() < idle_timeout
    }
}
