//! Formatting does work linear in the size of the input. The check counts allocations (every
//! step of the engine allocates, so the count follows the work) rather than time, so it cannot
//! flake. Quadrupling an input may at most quadruple the count, plus slack; an engine that
//! walks a subtree once per ancestor, or rescans the rest of the document for every group,
//! multiplies it by about sixteen.

use core_format::DirectiveTable;
use perf_bench::{file, inputs};
use provider_api::LanguageProvider;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    /// Per thread, so tests running in parallel do not count each other's allocations.
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

fn bump() {
    // `try_with`: allocations during thread teardown are not counted.
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Allocations of one `format_file` call, parsing and safety checks included.
fn allocs(p: &dyn LanguageProvider, name: &str, text: &str) -> u64 {
    let (f, opts) = file(p, name, 1, text);
    let before = ALLOCS.with(Cell::get);
    provider_api::format_file(p, &f, &opts, &DirectiveTable::default()).expect("formats");
    ALLOCS.with(Cell::get) - before
}

fn assert_linear(p: &dyn LanguageProvider, name: &str, make: fn(usize) -> String, small: usize) {
    let (a, b) = (
        allocs(p, name, &make(small)),
        allocs(p, name, &make(small * 4)),
    );
    assert!(a > 0, "{name}: nothing was counted");
    assert!(
        b * 100 <= a * 460,
        "{name}: {small} units made {a} allocations but {} made {b} ({:.1}x for 4x the input)",
        small * 4,
        b as f64 / a as f64
    );
}

#[test]
fn json_work_is_linear_in_the_input() {
    let p = lang_json::JsonProvider::new();
    assert_linear(&p, "a.json", inputs::json, 400);
    assert_linear(&p, "a.json", inputs::json_flat, 2000);
}

#[test]
fn json_work_is_linear_in_the_nesting_depth() {
    assert_linear(&lang_json::JsonProvider::new(), "a.json", inputs::json_nested, 30);
}

#[test]
fn yaml_work_is_linear_in_the_input() {
    assert_linear(&lang_yaml::YamlProvider::new(), "a.yaml", inputs::yaml, 600);
}

#[test]
fn yaml_work_is_linear_in_the_nesting_depth() {
    assert_linear(&lang_yaml::YamlProvider::new(), "a.yaml", inputs::yaml_nested, 10);
}
