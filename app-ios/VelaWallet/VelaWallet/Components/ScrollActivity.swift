//
//  ScrollActivity.swift
//  VelaWallet
//
//  Whether a list is moving under a finger — a drag, or the fling after it —
//  for the incoming scan's quiet periods (r10 follow-up, `activity_feed`
//  `interacting {on}`): the core holds a scan that comes due while somebody
//  scrolls and runs it when the list has settled, so no scan and no redraw
//  of the cadence lands in a gesture's way.
//
//  Two halves, because the deployment target is iOS 17.4 and the scroll
//  phase is iOS 18's:
//
//  - `reportsScrolling` goes on the view that HOLDS the scroll view. On
//    iOS 18 and later it is `onScrollPhaseChange` (`isScrolling`: tracking,
//    decelerating, animating) — nothing runs per frame.
//  - `scrollingProbe` goes on the scroll view's CONTENT. Below iOS 18 it
//    finds the `UIScrollView` it sits in and reads `isTracking ||
//    isDecelerating` from its pan and its offset; from iOS 18 it is nothing.
//
//  The report is transport, not a decision: `true` once when the list starts
//  moving, `false` once when it has settled.
//

import SwiftUI
import UIKit

extension View {
    /// iOS 18+: the first scroll view under this one says when it starts and
    /// stops moving. Pair with `scrollingProbe` on the content for iOS 17.
    @ViewBuilder
    func reportsScrolling(_ report: ((Bool) -> Void)?) -> some View {
        if #available(iOS 18.0, *), let report {
            onScrollPhaseChange { _, phase in report(phase.isScrolling) }
        } else {
            self
        }
    }

    /// Below iOS 18: the scroll view this content sits in says when it
    /// starts and stops moving. Nothing from iOS 18 on (`reportsScrolling`).
    @ViewBuilder
    func scrollingProbe(_ report: ((Bool) -> Void)?) -> some View {
        if #available(iOS 18.0, *) {
            self
        } else if let report {
            background(ScrollingProbe(report: report))
        } else {
            self
        }
    }
}

/// The iOS 17 half: an invisible view inside the scroll content that finds
/// its `UIScrollView` and watches its pan and its offset.
private struct ScrollingProbe: UIViewRepresentable {
    let report: (Bool) -> Void

    func makeUIView(context: Context) -> ProbeView {
        let view = ProbeView()
        view.isUserInteractionEnabled = false
        view.report = report
        return view
    }

    func updateUIView(_ view: ProbeView, context: Context) {
        view.report = report
    }

    final class ProbeView: UIView {
        var report: (Bool) -> Void = { _ in }
        private weak var scrollView: UIScrollView?
        private var offset: NSKeyValueObservation?
        private var moving = false
        private var settle: DispatchWorkItem?

        override func didMoveToWindow() {
            super.didMoveToWindow()
            guard window != nil else {
                detach()
                return
            }
            var parent = superview
            while let view = parent, !(view is UIScrollView) { parent = view.superview }
            guard let found = parent as? UIScrollView, found !== scrollView else { return }
            detach()
            scrollView = found
            found.panGestureRecognizer.addTarget(self, action: #selector(panned(_:)))
            offset = found.observe(\.contentOffset, options: []) { [weak self] _, _ in
                MainActor.assumeIsolated { self?.offsetMoved() }
            }
        }

        private func detach() {
            scrollView?.panGestureRecognizer.removeTarget(self, action: #selector(panned(_:)))
            offset?.invalidate()
            offset = nil
            scrollView = nil
            settle?.cancel()
            if moving { set(false) }
        }

        @objc private func panned(_ pan: UIPanGestureRecognizer) {
            switch pan.state {
            case .began: set(true)
            case .ended, .cancelled, .failed: scheduleSettle()
            default: break
            }
        }

        /// A fling keeps the offset moving after the finger lifts; the list
        /// has settled once it has stopped for a beat with neither.
        private func offsetMoved() {
            guard moving else { return }
            scheduleSettle()
        }

        private func scheduleSettle() {
            settle?.cancel()
            let work = DispatchWorkItem { [weak self] in
                guard let self, let scrollView = self.scrollView else { return }
                if !scrollView.isTracking && !scrollView.isDecelerating { self.set(false) }
            }
            settle = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.15, execute: work)
        }

        private func set(_ on: Bool) {
            guard on != moving else { return }
            moving = on
            report(on)
        }
    }
}
