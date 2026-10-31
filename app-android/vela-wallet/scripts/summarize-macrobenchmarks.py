#!/usr/bin/env python3
"""Native parity Phase 4: one line per benchmark from Macrobenchmark's JSON.

    python3 app-android/vela-wallet/scripts/summarize-macrobenchmarks.py <results-dir-or-json> [...]

Startup: TTID / TTFD medians (and min/max), the launch animation's span.
Frames: jank = the share of frames that missed their deadline
(frameOverrunMs > 0, the same definition as Android vitals' "slow frames"
for a frame-paced app), plus overrun P50/P90/P99 and CPU frame time P50/P90.
Memory: the run's max RSS (anon + file) and Java heap, medians over runs.
"""
import glob
import json
import os
import statistics
import sys


def files(args):
    for arg in args:
        if os.path.isdir(arg):
            yield from sorted(glob.glob(os.path.join(arg, "*benchmarkData.json")))
        else:
            yield arg


def median(metric):
    return metric.get("median") if metric else None


def fmt(value, unit=""):
    return "-" if value is None else f"{value:,.1f}{unit}"


def main(args):
    for path in files(args):
        data = json.load(open(path))
        for bench in data["benchmarks"]:
            name = f"{bench['className'].rsplit('.', 1)[-1]}.{bench['name']}"
            m = bench.get("metrics", {})
            s = bench.get("sampledMetrics", {})
            parts = []
            if "timeToInitialDisplayMs" in m:
                ttid = m["timeToInitialDisplayMs"]
                parts.append(f"TTID {fmt(ttid['median'], ' ms')} (min {fmt(ttid['minimum'])}, max {fmt(ttid['maximum'])})")
            if "timeToFullDisplayMs" in m:
                ttfd = m["timeToFullDisplayMs"]
                parts.append(f"TTFD {fmt(ttfd['median'], ' ms')} (min {fmt(ttfd['minimum'])}, max {fmt(ttfd['maximum'])})")
            for key, metric in m.items():
                if key.endswith("DurMs") and "launchAnimation" in key:
                    parts.append(f"animation {fmt(metric['median'], ' ms')}")
            if "frameOverrunMs" in s:
                frames = [f for run in s["frameOverrunMs"]["runs"] for f in run]
                janky = sum(1 for f in frames if f > 0)
                over = s["frameOverrunMs"]
                cpu = s.get("frameDurationCpuMs", {})
                parts.append(
                    f"jank {100.0 * janky / max(1, len(frames)):.2f}% of {len(frames)} frames, "
                    f"overrun P50/P90/P99 {fmt(over['P50'])}/{fmt(over['P90'])}/{fmt(over['P99'])} ms, "
                    f"cpu P50/P90 {fmt(cpu.get('P50'))}/{fmt(cpu.get('P90'))} ms"
                )
            anon = median(m.get("memoryRssAnonMaxKb")) or median(m.get("memoryRssAnonLastKb"))
            file_ = median(m.get("memoryRssFileMaxKb")) or median(m.get("memoryRssFileLastKb"))
            heap = median(m.get("memoryHeapSizeMaxKb")) or median(m.get("memoryHeapSizeLastKb"))
            if anon is not None:
                parts.append(f"RSS anon+file {fmt((anon + (file_ or 0)) / 1024, ' MB')} (anon {fmt(anon / 1024)}), heap {fmt((heap or 0) / 1024, ' MB')}")
            print(f"{name}: " + "; ".join(parts))


if __name__ == "__main__":
    main(sys.argv[1:] or ["."])
