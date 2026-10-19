use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtThread, Threading};
use cxx_qt_lib::QString;
use serde_json::json;
use taskmgr_core::Root;
use taskmgr_core::bench::{self, Run};

use super::{Monitor, json};

/// Sends the benchmark state to QML, at most ten times a second unless
/// `force`.
struct Reporter {
    qt: CxxQtThread<Monitor>,
    kind: String,
    last: std::cell::Cell<Option<Instant>>,
}

impl Reporter {
    fn send(&self, state: serde_json::Value, force: bool) {
        let due = self
            .last
            .get()
            .is_none_or(|t| t.elapsed() >= Duration::from_millis(100));
        if !(force || due) {
            return;
        }
        self.last.set(Some(Instant::now()));
        let mut state = state;
        state["kind"] = json!(self.kind);
        let text = json(&state);
        let _ = self.qt.queue(move |o| o.set_benchmark(text));
    }

    fn progress(&self, fraction: f64) {
        self.send(json!({ "running": true, "progress": fraction }), false);
    }
}

impl Monitor {
    /// `kind`: cpu, memory, disk (`option`: folder) or stability (`option`:
    /// minutes, 0 until stopped).
    pub fn start_benchmark(self: Pin<&mut Self>, kind: &QString, option: &QString) {
        if self.bench_running.swap(true, Ordering::SeqCst) {
            return;
        }
        self.bench_stop.store(false, Ordering::SeqCst);
        let (kind, option) = (kind.to_string(), option.to_string());
        let (stop, running) = (
            Arc::clone(&self.bench_stop),
            Arc::clone(&self.bench_running),
        );
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let reporter = Reporter {
                qt: qt.clone(),
                kind: kind.clone(),
                last: std::cell::Cell::new(None),
            };
            reporter.send(json!({ "running": true, "progress": 0.0 }), true);
            let root = Root::system();
            let mut run = Run {
                time: bench::now(),
                kind: kind.clone(),
                cpu_model: taskmgr_core::system_info(&root).cpu.model,
                ..Run::default()
            };
            let progress = |f: f64| reporter.progress(f);
            let outcome: Result<(), String> = match kind.as_str() {
                "cpu" => {
                    run.cpu = Some(bench::run_cpu(&stop, &progress));
                    Ok(())
                }
                "memory" => {
                    run.memory = Some(bench::run_memory(
                        bench::available_memory(&root),
                        &stop,
                        &progress,
                    ));
                    Ok(())
                }
                "disk" => bench::run_disk(std::path::Path::new(&option), &stop, &progress)
                    .map(|d| run.disk = Some(d)),
                "stability" => {
                    let minutes: u64 = option.parse().unwrap_or(0);
                    run.stability = Some(stability(&stop, minutes, &reporter));
                    Ok(())
                }
                _ => Err("Unknown test.".to_owned()),
            };
            let stopped = stop.load(Ordering::SeqCst) && kind != "stability";
            let outcome = outcome.and_then(|()| {
                if stopped {
                    Err("Stopped.".to_owned())
                } else {
                    bench::record(run.clone())
                }
            });
            let error = outcome.err().unwrap_or_default();
            reporter.send(
                json!({ "running": false, "progress": 1.0, "result": run, "error": error }),
                true,
            );
            let history = json(&bench::history());
            let _ = qt.queue(move |o| o.set_benchmark_history(history));
            running.store(false, Ordering::SeqCst);
        });
    }

    pub fn stop_benchmark(&self) {
        self.bench_stop.store(true, Ordering::SeqCst);
    }

    pub fn refresh_benchmark_history(self: Pin<&mut Self>) {
        self.set_benchmark_history(json(&bench::history()));
    }

    pub fn clear_benchmark_history(self: Pin<&mut Self>) -> QString {
        let error = bench::clear_history().err().unwrap_or_default();
        self.set_benchmark_history(json(&bench::history()));
        QString::from(&error)
    }
}

/// Runs the stability workload until stopped or `minutes` elapsed,
/// reporting errors and throughput every second.
fn stability(
    stop: &std::sync::atomic::AtomicBool,
    minutes: u64,
    reporter: &Reporter,
) -> bench::StabilityResult {
    let state = bench::cpu::Stability::default();
    let started = Instant::now();
    let limit = (minutes > 0).then(|| Duration::from_secs(minutes * 60));
    std::thread::scope(|s| {
        s.spawn(|| bench::cpu::stability(bench::threads(), &state, stop));
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(500));
            let elapsed = started.elapsed();
            if limit.is_some_and(|l| elapsed >= l) {
                stop.store(true, Ordering::SeqCst);
            }
            reporter.send(
                json!({
                    "running": true,
                    "progress": limit.map_or(-1.0, |l| elapsed.as_secs_f64() / l.as_secs_f64()),
                    "seconds": elapsed.as_secs(),
                    "blocks": state.blocks.load(Ordering::Relaxed),
                    "errors": state.errors.load(Ordering::Relaxed),
                }),
                true,
            );
        }
    });
    bench::StabilityResult {
        seconds: started.elapsed().as_secs(),
        blocks: state.blocks.load(Ordering::Relaxed),
        errors: state.errors.load(Ordering::Relaxed),
    }
}
