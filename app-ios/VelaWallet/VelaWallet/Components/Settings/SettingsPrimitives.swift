//
//  SettingsPrimitives.swift
//  VelaWallet
//
//  The settings vocabulary's smallest pieces (spec 023). Every one of the
//  forty mocks in `design/settings/` is assembled from these plus the rows
//  beside them. Nothing here reads a model bigger than it draws, and nothing
//  formats.
//

import SwiftUI
import UIKit
import UIKit.UIGestureRecognizerSubclass

/// The one badge every settings screen uses. Latency, reachability, provider
/// state and compatibility are all this object in the mocks, differing only in
/// tone — so they are one component and not four.
struct StatusPill: View {
    @Environment(\.theme) private var theme
    let pill: StatusPillModel

    private var colors: (fg: Color, bg: Color) {
        switch pill.tone {
        case .ok: (theme.successBase, theme.successSoft)
        case .warn: (theme.warningBase, theme.warningSoft)
        case .error: (theme.errorBase, theme.errorSoft)
        // Unset, not failed — the mocks grey these rather than colouring them.
        case .neutral: (theme.fgSubtle, theme.bgRaised)
        }
    }

    var body: some View {
        HStack(spacing: Tokens.Space.s4) {
            if pill.dot {
                Circle().fill(colors.fg).frame(width: 6, height: 6)
            }
            Text(pill.label)
                .typeRole(Typography.label)
                .foregroundStyle(colors.fg)
        }
        .padding(.horizontal, Tokens.Space.s8)
        .padding(.vertical, Tokens.Space.s2)
        .background(colors.bg, in: Capsule())
    }
}

/// The tinted explanation box. Eight mocks use it; `.success` swaps the
/// triangle for a check, because a green triangle reads as an alarm.
struct SettingsCallout: View {
    @Environment(\.theme) private var theme
    let callout: CalloutModel

    private var style: (fg: Color, bg: Color, glyph: LucideGlyph) {
        switch callout.tone {
        case .warning: (theme.warningBase, theme.warningSoft, .triangleAlert)
        case .danger: (theme.errorBase, theme.errorSoft, .triangleAlert)
        case .info: (theme.infoBase, theme.infoSoft, .info)
        case .success: (theme.successBase, theme.successSoft, .check)
        }
    }

    var body: some View {
        HStack(alignment: .top, spacing: Tokens.Space.s12) {
            LucideIcon(style.glyph, size: LucideIconSize.rowGlyph)
                .foregroundStyle(style.fg)
                // Optical alignment with the first line, not the box.
                .padding(.top, Tokens.Space.s2)
            Text(callout.text)
                .typeRole(Typography.body)
                .foregroundStyle(style.fg)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(Tokens.Space.s12)
        .background(style.bg, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
    }
}

/// A chain's circular avatar — its own logo when the chain-data endpoint has
/// one (spec 047), else one letter over its brand colour, which is also what
/// shows while the logo loads.
struct ChainMark: View {
    let mark: ChainMarkModel
    var size: CGFloat = 32

    var body: some View {
        RemoteLogoView(urls: mark.logoUrl.map { [$0] } ?? [], size: size) {
            Circle()
                .fill(mark.color)
                .frame(width: size, height: size)
                .overlay(
                    Text(mark.letter)
                        .typeRole(Typography.body)
                        .fontWeight(.bold)
                        .foregroundStyle(.white)
                )
        }
    }
}

/// The hairline the mocks draw between rows.
struct SettingsDivider: View {
    @Environment(\.theme) private var theme

    var body: some View {
        Rectangle().fill(theme.borderBase).frame(height: 1)
    }
}

/// The small caps label above a group of rows — 外观 / 区域格式 / 高级. 高级 is
/// the one that collapses (ST1b), so the chevron is optional and the whole
/// label becomes tappable only when it is present.
struct SettingsSectionLabel: View {
    @Environment(\.theme) private var theme
    let label: String
    var collapsible = false
    var collapsed = false
    var onToggle: () -> Void = {}

    var body: some View {
        HStack {
            Text(label)
                .typeRole(Typography.body)
                .foregroundStyle(theme.fgSubtle)
            Spacer()
            if collapsible {
                LucideIcon(collapsed ? .chevronDown : .chevronRight, size: LucideIconSize.rowGlyph)
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        .padding(.top, Tokens.Space.s20)
        .padding(.bottom, Tokens.Space.s8)
        .contentShape(Rectangle())
        .onTapGesture { if collapsible { onToggle() } }
    }
}

/// A labelled mono field. Every endpoint on ST9b / ST11 / ST12 / SR2 / SR5 is
/// one of these: a label row that may carry a latency pill, the value in a
/// sunken box, an optional in-field action, and an optional hint under it.
struct SettingsUrlField: View {
    @Environment(\.theme) private var theme
    let field: UrlFieldModel

    /// Whether this box is the one being typed into.
    ///
    /// Spec 081: the doc on `onCommit` below has always said "on submit **or
    /// on losing focus**", and only `.onSubmit` was wired — so a person who
    /// typed an endpoint and tapped anywhere else had saved nothing, while the
    /// health badge went on reporting the OLD host as online. Only the
    /// keyboard's Done key committed. The core persists on `EndpointBlurred`;
    /// `EndpointEdited` sets a draft and nothing more.
    @FocusState private var focused: Bool

    /// The editable mode (spec 050).
    ///
    /// **`nil` is the drawn state and renders exactly as it always has** — a
    /// `Text` in a bordered box, byte-for-byte the same view for every gallery
    /// board and every fixture call site, which is what keeps the screenshot
    /// sweep meaningful.
    ///
    /// Passing a binding turns the same box into a `TextField` with the same
    /// type role, the same colours and the same placeholder. That is the whole
    /// of the control this feature adds: the wizard was drawn in full — ST10,
    /// ST10b 兼容, ST10c 不兼容 — with no editable field anywhere, so it could
    /// be shown and could not be used, and `network_admin`'s sixteen operations
    /// had no way to receive a chain id from a person.
    ///
    /// An atom the drawing was missing, not a screen the drawing never had.
    var text: Binding<String>?
    /// Committed on submit — and, with `commitsOnBlur`, on losing focus, where
    /// the core runs its probes.
    var onCommit: () -> Void = {}
    /// "Saved as soon as you leave the field" (the network page says so under
    /// its RPC): leaving the field hands it over too, not only Return. Off by
    /// default, because some commits also close a sheet, and tapping beside a
    /// field is not a decision to close it.
    var commitsOnBlur = false
    /// The blue action inside the field — "Check key", "Get a key" — as a
    /// button. `nil` leaves it the drawn label.
    var onAction: (() -> Void)?

    private var border: Color {
        switch field.tone {
        case .error: theme.errorBase
        case .ok: theme.successBase
        // A hairline even at rest: on dark, sunken and base are one step apart
        // and the box would otherwise have no edge at all.
        default: theme.borderBase
        }
    }

    /// The mono face both branches wear, defined once.
    ///
    /// The editable and read-only halves must be typographically identical, or
    /// the box would visibly change shape the moment a screen went live. Written
    /// as `Font.system` rather than hidden behind implicit-member syntax so the
    /// literal audit still counts it — one honest violation instead of two.
    private var monoFace: Font {
        Font.system(size: Tokens.TextSize.t13, design: .monospaced)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            if !field.label.isEmpty || field.badge != nil {
                HStack {
                    if !field.label.isEmpty {
                        Text(field.label)
                            .typeRole(Typography.label)
                            .foregroundStyle(theme.fgSubtle)
                    }
                    Spacer()
                    if let badge = field.badge { StatusPill(pill: badge) }
                }
            }
            HStack(spacing: Tokens.Space.s8) {
                if let text {
                    TextField(field.placeholder ?? "", text: text)
                        .font(monoFace)
                        .foregroundStyle(theme.fgBase)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        // A chain id is digits and an RPC is a URL; neither
                        // wants a capitalising, autocorrecting keyboard.
                        .keyboardType(.URL)
                        .submitLabel(.done)
                        .focused($focused)
                        .onSubmit(onCommit)
                        .focused($focused)
                        .onChange(of: focused) { wasFocused, isFocused in
                            // Losing focus IS the blur the core waits for — and
                            // only a field that HAD focus can lose it, so a box
                            // nobody touched does not commit (main, 081).
                            if commitsOnBlur, wasFocused, !isFocused { onCommit() }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                } else {
                    Text(field.value.isEmpty ? (field.placeholder ?? "") : field.value)
                        .font(monoFace)
                        .foregroundStyle(field.value.isEmpty ? theme.fgSubtle : theme.fgBase)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                if let action = field.action {
                    // Drawn in link blue since 023 and tappable nowhere. It is
                    // a control on Android (`EditableUrlField(onAction:)`,
                    // which tests the key), so it is one here too; where no
                    // handler is given — the gallery — it stays a label rather
                    // than a button that does nothing.
                    if let onAction {
                        Button(action: onAction) {
                            Text(action)
                                .typeRole(Typography.body)
                                .foregroundStyle(theme.infoBase)
                        }
                        .buttonStyle(.plain)
                    } else {
                        Text(action)
                            .typeRole(Typography.body)
                            .foregroundStyle(theme.infoBase)
                    }
                }
            }
            .padding(.horizontal, Tokens.Space.s12)
            .frame(minHeight: 44)
            .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
            .overlay(
                RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                    .strokeBorder(border, lineWidth: 1)
            )
            if let hint = field.hint {
                Text(hint)
                    .typeRole(Typography.body)
                    .foregroundStyle(theme.fgSubtle)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// The product's ONE segmented control (design review 2026-07): three-up for
/// the theme picker. (Its two-up avatar-style use was retired in spec 074.)
struct SettingsSegmentedControl: View {
    @Environment(\.theme) private var theme
    let model: SegmentedModel
    var onSelect: (String) -> Void = { _ in }

    var body: some View {
        HStack(spacing: Tokens.Space.s4) {
            ForEach(model.segments) { segment in
                let selected = segment.id == model.selected
                HStack(spacing: Tokens.Space.s4) {
                    if let icon = segment.icon {
                        LucideIcon(icon, size: 14)
                            .foregroundStyle(selected ? theme.fgBase : theme.fgMuted)
                    }
                    Text(segment.label)
                        .typeRole(Typography.body)
                        .fontWeight(selected ? .semibold : .regular)
                        .foregroundStyle(selected ? theme.fgBase : theme.fgMuted)
                        .lineLimit(1)
                    // Three equal thirds of a 392pt screen do not hold "Follow
                    // System": clipping it to "Follow" is a different, wrong
                    // promise. Shrinking is the failure that still tells the truth.
                    .minimumScaleFactor(0.72)
                }
                .frame(maxWidth: .infinity, minHeight: 36)
                .background(
                    selected ? theme.bgRaised : Color.clear,
                    in: RoundedRectangle(cornerRadius: Tokens.Radius.r8)
                )
                // strokeBorder leaves the fill untouched, so the whole cell has
                // to be made hittable explicitly (button-feedback rule).
                .contentShape(Rectangle())
                // A pick that takes effect: Select, as Android's segments.
                .onTapGesture {
                    VelaHaptic.select.play()
                    onSelect(segment.id)
                }
            }
        }
        .padding(Tokens.Space.s4)
        .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        .overlay(
            // Dark mode sinks sunken BELOW raised, so the unselected track needs
            // a hairline to stay legible against bg.base.
            RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                .strokeBorder(theme.borderBase, lineWidth: 1)
        )
        .accessibilityElement(children: .contain)
        .accessibilityLabel(model.label)
    }
}

/// A ——●—— A. The tick row plus the two glyph ends, sized to what they promise.
///
/// A real slider (the founder, 2026-09: 没有滑动的感觉 — it was a row of six
/// tap targets): dragged or tapped, the thumb goes to the stop under the
/// finger, one `detent` per stop crossed, and the size is committed when the
/// finger lifts — Android's `VelaTextScaleSlider` and the web's range input.
/// The thumb follows the finger on local state; committing per stop would
/// re-lay the whole app out under the drag.
///
/// The thumb answers the finger as the desktop's does (the founder, 2026-09:
/// 现在的桌面视觉很好): from the moment a finger lands on the track — a tap
/// included — through the whole drag, the thumb grows and a soft ring comes
/// up around it, and both settle back when it lifts. Under a pointer (the
/// app on an iPad with a trackpad, or on a Mac) both grow a little and the
/// tick a click would land on is marked. The growth is a spring with no
/// bounce; reduced motion switches it instantly.
struct TextScaleSlider: View {
    @Environment(\.theme) private var theme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let model: TextScaleModel
    /// Which stop was chosen. Absent in the gallery, where the slider is a
    /// picture of a size already set — it takes no touch and plays nothing.
    var onSelect: ((Int) -> Void)?

    @State private var drag: TextScaleDrag

    /// `pictured` is for rendering only: it seeds the gesture state, so an
    /// `ImageRenderer` picture — which has no finger — can show the slider
    /// pressed or under a pointer. The app never passes it.
    init(model: TextScaleModel, onSelect: ((Int) -> Void)? = nil, pictured: TextScaleDrag = TextScaleDrag()) {
        self.model = model
        self.onSelect = onSelect
        _drag = State(initialValue: pictured)
    }

    /// The stops sit the pressed ring's radius in from the track's ends, so
    /// the ring is whole on either end stop. It was the thumb's radius before
    /// the ring; the glyph gap came down from `s12` to keep the end stops
    /// about where they were. The one number the drawing and the finger both
    /// measure the track by.
    static let inset: CGFloat = TextScaleLift.pressed.ring / 2
    private static let glyphGap: CGFloat = Tokens.Space.s4
    /// The web's `--space-sm` ticks, Android's 4dp; and the tick a click would
    /// land on, marked under a pointer (the desktop's 6 → 8, from iOS's 4).
    private static let tick: CGFloat = Tokens.Space.s4
    private static let tickAimed: CGFloat = 6
    /// Lifts the thumb off the ring — the web's thumb wears `--shadow-md`, the
    /// desktop a tighter 18% one. `Tokens.Shadow` (4–6% black) does not show
    /// on a dot this small, so this is the desktop's.
    private static let shadowOpacity: Double = 0.18
    private static let shadowRadius: CGFloat = 1.5
    /// The grow and shrink: critically damped, as the desktop's (its 30/s
    /// spring is a 0.21 s period) — a ring that wobbles reads as a toy.
    private static let liftSpring: Animation = .spring(response: 0.2, dampingFraction: 1)

    var body: some View {
        HStack(spacing: Self.glyphGap) {
            Text("A")
                .font(.system(size: Tokens.TextSize.t13, weight: .bold))
                .foregroundStyle(theme.fgBase)
            GeometryReader { geo in
                track(width: geo.size.width, height: geo.size.height)
            }
            // The whole row's height is the target, not the 4pt dots.
            .frame(height: Tokens.Layout.hitTarget)
            Text("A")
                .font(.system(size: Tokens.TextSize.t20, weight: .bold))
                .foregroundStyle(theme.fgBase)
        }
        .padding(.vertical, Tokens.Space.s12)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(model.label)
        .accessibilityIdentifier("text-scale-slider")
        // The stop, spoken and adjustable — a slider that only sighted fingers
        // can move is not a slider.
        .accessibilityValue(Text(verbatim: String(shown + 1)))
        .accessibilityAdjustableAction { direction in
            guard onSelect != nil else { return }
            let next = direction == .increment ? model.index + 1 : model.index - 1
            guard next >= 0, next < model.steps else { return }
            drag.move(to: next, committed: model.index)
            commit()
        }
    }

    /// The stop the thumb sits on: under the finger while it is down, the
    /// stored size otherwise.
    private var shown: Int { drag.shown ?? model.index }

    private func track(width: CGFloat, height: CGFloat) -> some View {
        let center = { (stop: Int) in
            TextScaleTrack.center(of: stop, width: width, steps: model.steps, inset: Self.inset)
        }
        let lift = drag.lift
        let aimed = drag.aimed(committed: model.index)
        // Glides stop to stop instead of blinking between them.
        let glide: Animation? = reduceMotion ? nil : .interactiveSpring(response: 0.18, dampingFraction: 0.82)
        let grow: Animation? = reduceMotion ? nil : Self.liftSpring
        return ZStack {
            ForEach(0..<model.steps, id: \.self) { stop in
                let marked = stop == aimed
                let size = marked ? Self.tickAimed : Self.tick
                Circle()
                    .fill(marked ? theme.fgSubtle : theme.borderStrong)
                    .frame(width: size, height: size)
                    .animation(grow, value: marked)
                    .position(x: center(stop), y: height / 2)
            }
            // The ring comes up from under the thumb: at rest it is the
            // thumb's size and not there.
            Circle()
                .fill(theme.fgMuted)
                .frame(width: lift.ring, height: lift.ring)
                .opacity(lift.ringOpacity)
                .animation(grow, value: lift)
                .position(x: center(shown), y: height / 2)
                .animation(glide, value: shown)
            Circle()
                .fill(theme.fgMuted)
                .frame(width: lift.thumb, height: lift.thumb)
                .shadow(color: .black.opacity(Self.shadowOpacity), radius: Self.shadowRadius, y: 1)
                .animation(grow, value: lift)
                .position(x: center(shown), y: height / 2)
                .animation(glide, value: shown)
        }
        .overlay {
            if onSelect != nil {
                TextScaleTouchSurface(
                    onPress: { down in drag.press(down) },
                    onHover: { x in
                        let over = x.map { stop(at: $0, width: width) }
                        // A pointer moving inside one stop redraws nothing.
                        if over != drag.hovered { drag.hover(over: over) }
                    },
                    onDrag: { x in
                        drag.move(to: stop(at: x, width: width), committed: model.index)
                    },
                    onRelease: { commit() },
                    onTap: { x in
                        drag.move(to: stop(at: x, width: width), committed: model.index)
                        commit()
                    }
                )
            }
        }
    }

    private func stop(at x: CGFloat, width: CGFloat) -> Int {
        TextScaleTrack.stop(at: x, width: width, steps: model.steps, inset: Self.inset)
    }

    private func commit() {
        if let next = drag.release(committed: model.index) { onSelect?(next) }
    }
}

/// Where the text-size stops sit on the track: evenly spaced between `inset`
/// in from each end (`TextScaleSlider.inset`, the pressed ring's radius), so
/// the thumb and its ring on the last stop are still inside the track. Pure,
/// so the arithmetic is tested without a finger.
enum TextScaleTrack {
    /// The stop nearest `x` — Android's `stepAt`, on the stops as drawn.
    static func stop(at x: CGFloat, width: CGFloat, steps: Int, inset: CGFloat) -> Int {
        guard steps > 1, width > inset * 2 else { return 0 }
        let pitch = (width - inset * 2) / CGFloat(steps - 1)
        let nearest = Int(((x - inset) / pitch).rounded())
        return min(steps - 1, max(0, nearest))
    }

    /// The x of a stop's centre.
    static func center(of stop: Int, width: CGFloat, steps: Int, inset: CGFloat) -> CGFloat {
        guard steps > 1, width > inset * 2 else { return width / 2 }
        return inset + (width - inset * 2) * CGFloat(stop) / CGFloat(steps - 1)
    }
}

/// One gesture on the text-size slider: the stop under the finger, the
/// detent each new stop earns, and how far the thumb is lifted. Apart from
/// the view so a test can run a finger across it.
struct TextScaleDrag {
    /// The stop under the finger while it is down; `nil` at rest.
    private(set) var shown: Int?
    /// A finger is on the track: from the moment it lands, before the tap or
    /// the pan has decided what the touch is, until it lifts or the page's
    /// scroll takes it.
    private(set) var touching = false
    /// The stop a pointer over the track is nearest (a trackpad or a mouse);
    /// `nil` when none is over it.
    private(set) var hovered: Int?

    /// Pressed from touch-down through the whole drag — the pan's release,
    /// not the finger's, ends a drag, and the two arrive in either order.
    var lift: TextScaleLift {
        if touching || shown != nil { return .pressed }
        return hovered == nil ? .rest : .hover
    }

    /// The tick to mark as the one a click would land on: under a pointer,
    /// not while pressed, and not the stop the thumb already sits on.
    func aimed(committed: Int) -> Int? {
        guard lift == .hover, let stop = hovered, stop != committed else { return nil }
        return stop
    }

    /// A finger landed on the track (`true`) or is gone from it (`false`).
    mutating func press(_ down: Bool) {
        touching = down
    }

    /// A pointer is over `stop`, or has left the track (`nil`).
    mutating func hover(over stop: Int?) {
        hovered = stop
    }

    /// The finger is over `stop`. A stop it was not already on is a detent —
    /// one per stop, however the finger got there; wobbling inside a stop
    /// plays nothing.
    mutating func move(to stop: Int, committed: Int) {
        let from = shown ?? committed
        shown = stop
        if stop != from { VelaHaptic.detent.play() }
    }

    /// The finger lifted: the stop to store, when it is not the stored one.
    mutating func release(committed: Int) -> Int? {
        defer { self.shown = nil }
        guard let stop = shown, stop != committed else { return nil }
        return stop
    }
}

/// How far the text-size thumb is lifted, and what each lift draws — the
/// desktop's three moments (`ui/step_slider.rs`) at iOS's sizes: its 16/20/22
/// thumb over a 34/40 ring becomes 20/24/26 over 40/44 here, because the
/// resting thumb was already the web's 20.
enum TextScaleLift: Equatable {
    case rest, hover, pressed

    /// The thumb's diameter.
    var thumb: CGFloat {
        switch self {
        case .rest: 20
        case .hover: 24
        case .pressed: 26
        }
    }

    /// The ring's diameter: at rest the thumb's own, hidden under it; pressed,
    /// the platform's touch target — the finger's own size.
    var ring: CGFloat {
        switch self {
        case .rest: TextScaleLift.rest.thumb
        case .hover: 40
        case .pressed: Tokens.Layout.hitTarget
        }
    }

    /// The ring's strength, as a share of `fg.muted` — the desktop's.
    var ringOpacity: Double {
        switch self {
        case .rest: 0
        case .hover: 0.10
        case .pressed: 0.16
        }
    }
}

/// The slider's touch, in UIKit because SwiftUI cannot say "horizontal only".
///
/// Settings is a scroll view, and the slider is a full-width row in it. A
/// SwiftUI drag that began on the row either took every vertical scroll that
/// started there — moving the text size of somebody who only meant to scroll
/// past it — or shared the touch with the scroll view and wobbled the page
/// under a sideways drag. A pan that begins only when the finger moves more
/// sideways than up or down is what Android's `detectHorizontalDragGestures`
/// is; vertical movement is left to the page.
///
/// Neither the pan nor the tap says anything until it has decided — the tap
/// not until the finger lifts — so a third recognizer reports the finger
/// landing, for the thumb to answer it at once; and a pointer (trackpad,
/// mouse) is reported as it moves over the track.
private struct TextScaleTouchSurface: UIViewRepresentable {
    var onPress: (Bool) -> Void
    var onHover: (CGFloat?) -> Void
    var onDrag: (CGFloat) -> Void
    var onRelease: () -> Void
    var onTap: (CGFloat) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.backgroundColor = .clear
        let pan = UIPanGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.pan(_:)))
        pan.delegate = context.coordinator
        view.addGestureRecognizer(pan)
        view.addGestureRecognizer(
            UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.tap(_:)))
        )
        let press = TextScalePressRecognizer()
        press.onPress = { [coordinator = context.coordinator] down in coordinator.surface.onPress(down) }
        press.delegate = context.coordinator
        view.addGestureRecognizer(press)
        view.addGestureRecognizer(
            UIHoverGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.hover(_:)))
        )
        return view
    }

    func updateUIView(_ view: UIView, context: Context) {
        context.coordinator.surface = self
    }

    final class Coordinator: NSObject, UIGestureRecognizerDelegate {
        var surface: TextScaleTouchSurface

        init(_ surface: TextScaleTouchSurface) { self.surface = surface }

        @objc func pan(_ recognizer: UIPanGestureRecognizer) {
            switch recognizer.state {
            case .began, .changed:
                surface.onDrag(recognizer.location(in: recognizer.view).x)
            case .ended, .cancelled, .failed:
                surface.onRelease()
            default:
                break
            }
        }

        @objc func tap(_ recognizer: UITapGestureRecognizer) {
            guard recognizer.state == .ended else { return }
            surface.onTap(recognizer.location(in: recognizer.view).x)
        }

        @objc func hover(_ recognizer: UIHoverGestureRecognizer) {
            switch recognizer.state {
            case .began, .changed:
                surface.onHover(recognizer.location(in: recognizer.view).x)
            default:
                surface.onHover(nil)
            }
        }

        func gestureRecognizerShouldBegin(_ recognizer: UIGestureRecognizer) -> Bool {
            guard let pan = recognizer as? UIPanGestureRecognizer else { return true }
            let moved = pan.translation(in: pan.view)
            let sideways = moved == .zero ? pan.velocity(in: pan.view) : moved
            return abs(sideways.x) > abs(sideways.y)
        }

        /// The press watcher rides along with the slider's own pan and tap, so
        /// it hears the finger lift after a drag; the page's scroll it does
        /// not, so the scroll taking the touch ends the press.
        func gestureRecognizer(
            _ recognizer: UIGestureRecognizer,
            shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer
        ) -> Bool {
            guard recognizer is TextScalePressRecognizer || other is TextScalePressRecognizer else { return false }
            return recognizer.view === other.view
        }
    }
}

/// Reports a finger landing on the slider and leaving it, and nothing else.
///
/// It never recognizes, so it can take no touch from the pan, the tap or the
/// page's scroll. It ends when the finger lifts, and when another gesture
/// takes the touch UIKit fails it — either way `reset` is where the press
/// ends, so there is one place it can.
private final class TextScalePressRecognizer: UIGestureRecognizer {
    var onPress: (Bool) -> Void = { _ in }
    private var down = false

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesBegan(touches, with: event)
        guard !down else { return }
        down = true
        onPress(true)
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesEnded(touches, with: event)
        state = .failed
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesCancelled(touches, with: event)
        state = .failed
    }

    override func reset() {
        super.reset()
        guard down else { return }
        down = false
        onPress(false)
    }

    override func canPrevent(_ preventedGestureRecognizer: UIGestureRecognizer) -> Bool { false }
}
