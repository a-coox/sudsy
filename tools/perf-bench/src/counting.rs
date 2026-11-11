//! Allocation counting and sampling (feature `count`): how many allocations each phase makes
//! and, sampled, which code makes them.

use std::alloc::{GlobalAlloc, Layout, System};
use std::backtrace::Backtrace;
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub struct Counting;

static COUNT: AtomicU64 = AtomicU64::new(0);
static SAMPLING: AtomicU64 = AtomicU64::new(0);
const EVERY: u64 = 97;

thread_local! {
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static SAMPLES: RefCell<Vec<Backtrace>> = const { RefCell::new(Vec::new()) };
}

fn note() {
    let n = COUNT.fetch_add(1, Relaxed);
    if SAMPLING.load(Relaxed) != 0 && n % EVERY == 0 {
        BUSY.with(|busy| {
            if !busy.replace(true) {
                let bt = Backtrace::force_capture();
                SAMPLES.with(|s| s.borrow_mut().push(bt));
                busy.set(false);
            }
        });
    }
}

// Not inlined: scripts/bench-measure.sh reads their inclusive instruction counts under callgrind to
// report the allocator's share apart from the code that calls it.
unsafe impl GlobalAlloc for Counting {
    #[inline(never)]
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        note();
        unsafe { System.alloc(l) }
    }
    #[inline(never)]
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    #[inline(never)]
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        note();
        unsafe { System.realloc(p, l, n) }
    }
}

pub fn count() -> u64 {
    COUNT.load(Relaxed)
}

/// Starts or stops collecting one backtrace per `EVERY` allocations.
pub fn sample(on: bool) {
    SAMPLING.store(u64::from(on), Relaxed);
}

/// Prints the most frequent allocation sites: the nearest frame in our own crates plus its caller.
pub fn report(top: usize) {
    let samples = SAMPLES.with(|s| std::mem::take(&mut *s.borrow_mut()));
    let mut sites: std::collections::HashMap<String, u64> = Default::default();
    for bt in &samples {
        let text = bt.to_string();
        let frames: Vec<&str> = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("at "))
            .map(|l| l.split_once(": ").map_or(l, |x| x.1).trim())
            .collect();
        let ours = |f: &&str| {
            f.contains("core_format")
                || f.contains("lang_json")
                || f.contains("lang_yaml")
                || f.contains("core_syntax")
                || f.contains("provider_api")
        };
        let Some(i) = frames.iter().position(ours) else {
            continue;
        };
        let key = format!("{} <- {}", frames[i], frames.get(i + 1).unwrap_or(&"?"));
        *sites.entry(key).or_default() += 1;
    }
    let mut sites: Vec<_> = sites.into_iter().collect();
    sites.sort_by_key(|s| std::cmp::Reverse(s.1));
    let total = samples.len().max(1);
    for (site, n) in sites.iter().take(top) {
        println!("  {:>5.1}%  {site}", 100.0 * *n as f64 / total as f64);
    }
}
