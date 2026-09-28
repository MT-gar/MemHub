//! Background file watcher: debounce change events on all source roots and
//! trigger a sync. Also runs a periodic full sync as a safety net.

use crate::adapters;
use crate::hub::Hub;
use crate::model::SyncReport;
use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub type ReportHook = Arc<dyn Fn(&SyncReport) + Send + Sync>;

pub struct WatchHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WatchHandle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn watch_roots(w: &mut RecommendedWatcher, current: &mut Vec<PathBuf>, wanted: Vec<PathBuf>) {
    if *current == wanted {
        return;
    }
    for p in current.iter() {
        let _ = w.unwatch(p);
    }
    for p in &wanted {
        if let Err(e) = w.watch(p, RecursiveMode::Recursive) {
            tracing::warn!("watch {}: {e}", p.display());
        }
    }
    *current = wanted;
}

/// Start watching. `interval_secs` = periodic full sync (0 disables it).
pub fn spawn(hub: Arc<Hub>, interval_secs: u64, hook: Option<ReportHook>) -> Result<WatchHandle> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    let mut current: Vec<PathBuf> = Vec::new();
    watch_roots(&mut watcher, &mut current, adapters::watch_roots(&hub.config()));

    let thread = std::thread::Builder::new().name("memhub-watch".into()).spawn(move || {
        let _keep_alive = &watcher;
        let debounce = Duration::from_millis(800);
        let mut dirty_since: Option<Instant> = None;
        let mut last_full = Instant::now();
        let mut last_roots_check = Instant::now();
        loop {
            if stop2.load(Ordering::SeqCst) {
                break;
            }
            match rx.recv_timeout(Duration::from_millis(300)) {
                Ok(Ok(ev)) => {
                    let relevant = ev.paths.iter().any(|p| {
                        let s = p.to_string_lossy().replace('\\', "/");
                        !s.contains("/.git/") && !s.ends_with(".tmp") && !s.ends_with('~')
                    });
                    if relevant && dirty_since.is_none() {
                        dirty_since = Some(Instant::now());
                    } else if relevant {
                        dirty_since = Some(Instant::now());
                    }
                }
                Ok(Err(e)) => tracing::warn!("watch error: {e}"),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            let due_debounce = dirty_since.map(|t| t.elapsed() >= debounce).unwrap_or(false);
            let due_periodic = interval_secs > 0 && last_full.elapsed() >= Duration::from_secs(interval_secs);
            if due_debounce || due_periodic {
                dirty_since = None;
                last_full = Instant::now();
                match hub.sync() {
                    Ok(r) => {
                        if r.added + r.updated + r.archived > 0 {
                            tracing::info!("sync: +{} ~{} archived {} ({} ms)", r.added, r.updated, r.archived, r.duration_ms);
                        }
                        if let Some(h) = &hook {
                            h(&r);
                        }
                    }
                    Err(e) => tracing::error!("sync failed: {e}"),
                }
            }
            if last_roots_check.elapsed() >= Duration::from_secs(10) {
                last_roots_check = Instant::now();
                watch_roots(&mut watcher, &mut current, adapters::watch_roots(&hub.config()));
            }
        }
    })?;
    Ok(WatchHandle { stop, thread: Some(thread) })
}
