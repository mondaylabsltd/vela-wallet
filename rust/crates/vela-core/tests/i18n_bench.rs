//! Performance and memory budget for translation resolution (spec 004-rust-i18n,
//! SC-007 / SC-009).
//!
//! These are **assertions, not printouts**. "At most two heap allocations per
//! `t()`" is exactly the property that regresses the first time someone reaches for
//! `format!` inside the candidate loop, and nothing else in the suite would notice —
//! the 18,975 conformance cases would stay green while every language switch got
//! slower.
//!
//! Everything lives in ONE `#[test]` on purpose: the counting allocator is global,
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
            // A realloc means a capacity estimate was wrong — exactly the
            // regression this test exists to catch, so it counts.
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: CountingAllocator = CountingAllocator;

fn count_allocations<T>(f: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNTING.set(true);
    let out = f();
    COUNTING.set(false);
    (out, ALLOCATIONS.load(Ordering::Relaxed))
}

#[test]
fn i18n_bench() {
    use vela_core::i18n::{Catalog, Count, I18n, Options, Var};

    let en = match Catalog::embedded("en") {
        Ok(c) => c,
        Err(e) => unreachable!("i18n-en must be enabled for this test: {e}"),
    };
    let mut engine = match I18n::new(en) {
        Ok(e) => e,
        Err(e) => unreachable!("{e}"),
    };
    match Catalog::embedded("ru") {
        Ok(c) => {
            engine.load_catalog(c);
        }
        Err(e) => unreachable!("i18n-ru must be enabled: {e}"),
    }
    engine.change_language("ru");

    // The shape SC-007 names: a key lookup, a plural selection and interpolation.
    // Russian at count=21 is the interesting case — it selects `_one` through the
    // four-category rule, which is the whole reason this feature exists.
    let vars: [(&str, Var<'_>); 0] = [];
    let plural_opts = Options {
        count: Some(Count::Num(21.0)),
        vars: &vars,
        ..Options::default()
    };
    let two_vars = [("name", Var::Str("Alice")), ("id", Var::Num(137.0))];
    let interp_opts = Options {
        vars: &two_vars,
        ..Options::default()
    };

    // Warm up: first touch pages in the catalog and the path table.
    for _ in 0..1_000 {
        std::hint::black_box(engine.t("send.recipientCount", &plural_opts).ok());
        std::hint::black_box(engine.t("receive.networkDetail", &interp_opts).ok());
    }

    // -- allocations --------------------------------------------------------

    let (plural_out, plural_allocs) =
        count_allocations(|| engine.t("send.recipientCount", &plural_opts));
    let (interp_out, interp_allocs) =
        count_allocations(|| engine.t("receive.networkDetail", &interp_opts));
    let (_, miss_allocs) = count_allocations(|| engine.t("zz.no.such.key", &Options::default()));

    println!(
        "t() plural (ru, count=21) -> {:?}   {plural_allocs} allocations",
        plural_out.as_deref().unwrap_or("<err>")
    );
    println!(
        "t() interpolated          -> {:?}   {interp_allocs} allocations",
        interp_out.as_deref().unwrap_or("<err>")
    );
    println!("t() missing key                                    {miss_allocs} allocations");

    // -- latency ------------------------------------------------------------

    const ITERATIONS: u32 = 50_000;

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        std::hint::black_box(engine.t("common.cancel", &Options::default()).ok());
    }
    let plain_us = start.elapsed().as_secs_f64() * 1e6 / f64::from(ITERATIONS);

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        std::hint::black_box(engine.t("send.recipientCount", &plural_opts).ok());
    }
    let plural_us = start.elapsed().as_secs_f64() * 1e6 / f64::from(ITERATIONS);

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        std::hint::black_box(engine.t("receive.networkDetail", &interp_opts).ok());
    }
    let interp_us = start.elapsed().as_secs_f64() * 1e6 / f64::from(ITERATIONS);

    // A whole screen's worth. `_layout.tsx` remounts the tree on a language switch,
    // so this is what one switch actually costs.
    const SCREEN_KEYS: u32 = 500;
    let start = Instant::now();
    for i in 0..SCREEN_KEYS {
        let key = if i % 3 == 0 {
            "send.recipientCount"
        } else {
            "common.cancel"
        };
        let opts = if i % 3 == 0 {
            &plural_opts
        } else {
            &interp_opts
        };
        std::hint::black_box(engine.t(key, opts).ok());
    }
    let screen_ms = start.elapsed().as_secs_f64() * 1e3;

    // Catalog load from JSON bytes — the web route's cold path.
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("i18n/locales");
    let mut merged = serde_json::Map::new();
    let mut absorb = |p: std::path::PathBuf| {
        if let Ok(t) = std::fs::read_to_string(&p) {
            if let Ok(serde_json::Value::Object(m)) = serde_json::from_str::<serde_json::Value>(&t)
            {
                merged.extend(m);
            }
        }
    };
    absorb(repo.join("ja.json"));
    for ns in [
        "home",
        "send",
        "receive",
        "assets",
        "addToken",
        "tokenDetail",
        "history",
        "onboarding",
        "connect",
        "about",
        "clearSigning",
        "componentsTx",
        "componentsUi",
        "settingsModals",
        "contacts",
        "explore",
    ] {
        absorb(repo.join("ja").join(format!("{ns}.json")));
    }
    let bytes = serde_json::Value::Object(merged).to_string().into_bytes();
    let start = Instant::now();
    let loaded = Catalog::from_json("ja", &bytes);
    let load_ms = start.elapsed().as_secs_f64() * 1e3;
    assert!(loaded.is_ok(), "the ja catalog must parse");

    println!("t() plain key             : {plain_us:.3} us/call");
    println!("t() with plural           : {plural_us:.3} us/call");
    println!("t() with 2 interpolations : {interp_us:.3} us/call");
    println!("500-key screen            : {screen_ms:.3} ms");
    println!(
        "Catalog::from_json (ja)   : {load_ms:.3} ms  ({} bytes)",
        bytes.len()
    );

    // -- budgets ------------------------------------------------------------
    //
    // Allocation counts hold in debug and release alike — they are structural, not
    // an optimisation artefact — so they are asserted unconditionally. Latency is
    // only meaningful in release: a debug build carries bounds checks and no
    // inlining, and asserting there would just be flaky.

    assert!(
        plural_allocs <= 2,
        "SC-007: a plural t() made {plural_allocs} allocations, budget is 2 \
         (one for the suffixed candidate key, one for the returned String)"
    );
    assert!(
        interp_allocs <= 2,
        "SC-007: an interpolated t() made {interp_allocs} allocations, budget is 2 \
         (the candidate-key scratch buffer and the returned String) — interpolation \
         writes directly into the output, so it adds none of its own"
    );
    assert!(
        miss_allocs <= 2,
        "SC-007: a missing-key t() made {miss_allocs} allocations, budget is 2"
    );

    if cfg!(debug_assertions) {
        println!("(debug build — SC-007 latency budgets asserted in release only)");
    } else {
        assert!(
            plain_us < 1.0,
            "SC-007: plain t() took {plain_us:.3} us, budget is 1.0"
        );
        assert!(
            plural_us < 1.0,
            "SC-007: plural t() took {plural_us:.3} us, budget is 1.0"
        );
        assert!(
            interp_us < 1.0,
            "SC-007: interpolated t() took {interp_us:.3} us, budget is 1.0"
        );
        assert!(
            screen_ms < 0.5,
            "SC-007: a 500-key screen took {screen_ms:.3} ms, budget is 0.5"
        );
        assert!(
            load_ms < 5.0,
            "SC-007: loading one catalog took {load_ms:.3} ms, budget is 5.0"
        );
    }
}
