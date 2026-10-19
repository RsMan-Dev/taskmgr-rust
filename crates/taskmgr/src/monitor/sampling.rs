use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use taskmgr_core::{ProcessSampler, Root, Sampler};

use super::{Monitor, json};

impl Monitor {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let info = json(&taskmgr_core::system_info(&Root::system()));
        self.as_mut().set_info(info);
        self.as_ref().watch_health();

        let qt = self.qt_thread();
        let interval = self.interval_ms.clone();
        let processes_visible = self.processes_visible.clone();
        let virtual_networks = self.virtual_networks.clone();
        let net = self.net.clone();
        thread::spawn(move || {
            let root = Root::system();
            let mut sampler = Sampler::new(root.clone());
            let mut processes = ProcessSampler::new(&root);
            let watcher = taskmgr_core::windows::WindowWatcher::start();
            let mut last = Instant::now();
            let mut count: u64 = 0;
            let mut was_wanted = false;
            loop {
                let started = Instant::now();
                let helper = net.lock().ok().and_then(|n| n.clone());
                sampler.fallback_voltage = helper
                    .as_ref()
                    .and_then(|h| h.vcore.as_ref())
                    .filter(|v| !v.is_empty())
                    .map(|v| v.iter().sum::<f64>() / v.len() as f64);
                let wanted = processes_visible.load(Ordering::Relaxed);
                sampler.per_process = wanted;
                sampler.virtual_networks = virtual_networks.load(Ordering::Relaxed);
                let snapshot = sampler.sample();
                let snapshot_json = json(&snapshot);
                // The process list is the costly part: skipped while no
                // page shows it.
                let procs = wanted.then(|| {
                    let windows = watcher.as_ref().and_then(|w| w.windows());
                    let rates = helper.map(|h| h.net);
                    let secs = started.duration_since(last).as_secs_f64();
                    last = started;
                    json(&processes.sample(
                        &root,
                        secs,
                        windows.as_ref(),
                        &snapshot.gpus,
                        rates.as_ref(),
                    ))
                });
                // Command lines are long and change rarely: every few samples.
                let details = (wanted && (!was_wanted || count.is_multiple_of(3)))
                    .then(|| json(&processes.details()));
                count += 1;
                was_wanted = wanted;
                let queued = qt.queue(move |mut o| {
                    o.as_mut().set_snapshot(snapshot_json);
                    if let Some(d) = details {
                        o.as_mut().set_process_details(d);
                    }
                    if let Some(p) = procs {
                        o.set_processes(p);
                    }
                });
                if queued.is_err() {
                    break;
                }
                loop {
                    let ms = interval.load(Ordering::Relaxed);
                    if ms > 0 && started.elapsed() >= Duration::from_millis(ms) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        });
    }

    /// SMART data changes slowly; UDisks2 itself polls every 10 minutes.
    fn watch_health(self: Pin<&Self>) {
        let qt = self.qt_thread();
        thread::spawn(move || {
            loop {
                let health = json(&taskmgr_core::smart::read());
                if qt.queue(move |o| o.set_health(health)).is_err() {
                    break;
                }
                thread::sleep(Duration::from_secs(600));
            }
        });
    }

    /// 0 pauses sampling.
    pub fn set_history_points(&self, points: i32) {
        taskmgr_core::util::set_history_len(usize::try_from(points).unwrap_or(60));
    }

    pub fn set_interval(&self, milliseconds: i32) {
        self.interval_ms
            .store(u64::try_from(milliseconds).unwrap_or(0), Ordering::Relaxed);
    }

    pub fn set_virtual_networks(&self, shown: bool) {
        self.virtual_networks.store(shown, Ordering::Relaxed);
    }

    pub fn set_processes_visible(&self, visible: bool) {
        self.processes_visible.store(visible, Ordering::Relaxed);
    }

    pub fn set_keep_above(&self, above: bool) {
        thread::spawn(move || taskmgr_core::windows::keep_above(above));
    }
}
