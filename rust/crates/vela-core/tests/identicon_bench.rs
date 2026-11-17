//! Performance and memory budget for identicon generation (spec 003-rust-identicon,
//! SC-004/SC-005).
//!
//! These are assertions, not printouts: the "at most one heap allocation" property
//! is the kind that quietly regresses the first time someone reaches for `format!`
//! inside assembly, and nothing else in the suite would notice.
//!
//! Everything lives in ONE `#[test]` on purpose — the counting allocator is global,
//! so a second test running concurrently in this binary would pollute the count.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    /// Counting is ON for the measuring thread only. A process-wide flag also
    /// counted whatever another thread allocated meanwhile — libtest's main
    /// thread runs beside the test's — so a busy CI runner now and then read
    /// 2 for an assembly that does 1 (PR #588's `rust` job, on a release commit
    /// that touched no Rust). `const`-initialised `Cell<bool>`: no lazy
    /// initialisation and no destructor, so reading it allocates nothing.
    static COUNTING: Cell<bool> = const { Cell::new(false) };
}

/// Whether this thread is measuring; `false` while its thread-locals are torn down.
fn counting() -> bool {
    COUNTING.try_with(Cell::get).unwrap_or(false)
}

struct CountingAllocator;

// SAFETY: every method forwards directly to the system allocator; the only added
// behaviour is a relaxed counter increment, which cannot allocate or unwind.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if counting() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if counting() {
            // A realloc means the capacity estimate was wrong — exactly the
            // regression this test exists to catch, so it counts.
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: CountingAllocator = CountingAllocator;

/// Measure allocations performed by `f`, excluding anything before or after.
fn count_allocations<T>(f: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNTING.set(true);
    let out = f();
    COUNTING.set(false);
    (out, ALLOCATIONS.load(Ordering::Relaxed))
}

const SEED: &str = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";

#[test]
fn identicon_bench() {
    use vela_core::identicon as ic;

    // --- allocations (SC-004) --------------------------------------------

    let (hash, allocs) = count_allocations(|| ic::make_hash(SEED));
    assert_eq!(
        allocs, 0,
        "make_hash must be allocation-free (the hash lives in a fixed inline buffer)"
    );
    assert_eq!(hash.len(), 17);

    let (params, allocs) = count_allocations(|| ic::identicon_params(SEED));
    assert_eq!(
        allocs, 0,
        "identicon_params must be allocation-free — colours and artwork are &'static str"
    );
    let params = params.expect("fixture address renders");

    let (_svg, allocs) = count_allocations(|| ic::assemble_svg_circular(&params));
    assert_eq!(
        allocs, 1,
        "assemble_svg_circular must do exactly ONE allocation: a String::with_capacity \
         of the exact final length. More means the capacity estimate drifted."
    );

    let (_svg, allocs) = count_allocations(|| ic::assemble_svg(&params));
    assert_eq!(allocs, 1, "assemble_svg must do exactly ONE allocation");

    let (svg, allocs) = count_allocations(|| ic::identicon_svg_circular(SEED));
    assert_eq!(
        allocs, 1,
        "the full seed -> SVG path must do exactly ONE allocation"
    );
    let svg = svg.expect("fixture address renders");

    // --- latency (SC-004) -------------------------------------------------

    // Warm the branch predictor / caches; the chaos table is static so there is no
    // initialisation to amortise.
    for _ in 0..1_000 {
        std::hint::black_box(ic::identicon_svg_circular(SEED).ok());
    }

    const ITERATIONS: u32 = 20_000;
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        std::hint::black_box(ic::identicon_svg_circular(SEED).ok());
    }
    let per_call = start.elapsed().as_secs_f64() * 1e6 / f64::from(ITERATIONS);

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        std::hint::black_box(ic::make_hash(SEED));
    }
    let hash_per_call = start.elapsed().as_secs_f64() * 1e6 / f64::from(ITERATIONS);

    println!("make_hash            (42-char address): {hash_per_call:.3} us/call");
    println!(
        "identicon_svg_circular:                 {per_call:.3} us/call   (svg {} bytes)",
        svg.len()
    );
    println!("allocations per identicon_svg_circular: 1");

    // A debug build carries bounds checks and no inlining, so the budget is only
    // meaningful in release. Asserting it in debug would just be flaky.
    if cfg!(debug_assertions) {
        println!("(debug build — SC-004 budget asserted in release only)");
    } else {
        assert!(
            per_call < 2.0,
            "SC-004: identicon_svg_circular took {per_call:.3} us/call, budget is 2.0 us"
        );
    }
}
