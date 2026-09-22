//
//  AlphaIndexRail.swift
//  VelaWallet
//
//  AlphaIndexRail + the letter-section header (spec 018 vocabulary #4/#5,
//  mock C1, SPEC 动效 · 索引条 A–Z). The rail renders the FULL alphabet plus
//  `#` regardless of which sections exist (research D4); a letter with no
//  section jumps to the nearest existing one. Sliding fires one
//  `VelaHaptic.detent` per crossed letter and shows a bubble
//  HUD near the finger (fade-in 120ms / fade-out 80ms). Reduced motion:
//  direct jump, no bubble animation.
//

import SwiftUI

/// Uppercase letter + hairline rule — the A–Z section header (C1).
struct ContactLetterHeader: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let letter: String

    var body: some View {
        HStack(spacing: ContactsGeometry.letterRuleLeading) {
            Text(verbatim: letter)
                .typeRole(Typography.sectionLetter.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
            Rectangle()
                .fill(theme.borderBase)
                .frame(height: Tokens.BorderWidth.hairline)
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .frame(minHeight: ContactsGeometry.letterHeaderHeight)
    }
}

struct AlphaIndexRail: View {
    @Environment(\.theme) private var theme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let letters: [String]
    /// Letters that actually have a section — the rail highlights these.
    var populated: Set<String> = []
    /// Static bubble for the component board (nil = gesture-driven).
    var pinnedBubble: String?
    var onSelect: (String) -> Void = { _ in }

    @State private var active: String?

    private var bubbleLetter: String? { pinnedBubble ?? active }

    var body: some View {
        GeometryReader { proxy in
            VStack(spacing: Tokens.Space.s0) {
                ForEach(letters, id: \.self) { letter in
                    Text(verbatim: letter)
                        .typeRole(Typography.sectionLetter)
                        .foregroundStyle(tint(letter))
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .frame(width: ContactsGeometry.indexRailWidth)
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { value in select(at: value.location.y, in: proxy.size.height) }
                    .onEnded { _ in dismissBubble() }
            )
        }
        .frame(width: ContactsGeometry.indexRailWidth)
        .accessibilityHidden(true)
        .overlay(alignment: .trailing) {
            if let bubbleLetter {
                bubble(bubbleLetter)
                    .offset(x: -ContactsGeometry.bubbleTrailingGap)
                    .transition(.opacity)
            }
        }
    }

    private func tint(_ letter: String) -> Color {
        if letter == bubbleLetter { return theme.accentBase }
        return populated.contains(letter) ? theme.fgMuted : theme.fgSubtle
    }

    private func bubble(_ letter: String) -> some View {
        Text(verbatim: letter)
            .typeRole(Typography.bubbleLetter)
            .foregroundStyle(theme.onAccent)
            .frame(width: ContactsGeometry.bubbleSize, height: ContactsGeometry.bubbleSize)
            .background(Circle().fill(theme.accentBase))
    }

    private func select(at y: CGFloat, in height: CGFloat) {
        guard !letters.isEmpty, height > 0 else { return }
        let step = height / CGFloat(letters.count)
        let index = min(letters.count - 1, max(0, Int(y / step)))
        let letter = letters[index]
        guard letter != active else { return }
        haptic()
        if reduceMotion {
            active = letter
        } else {
            withAnimation(.easeOut(duration: ContactsMotion.bubbleIn)) { active = letter }
        }
        onSelect(letter)
    }

    private func dismissBubble() {
        if reduceMotion {
            active = nil
        } else {
            withAnimation(.easeOut(duration: ContactsMotion.bubbleOut)) { active = nil }
        }
    }

    private func haptic() {
        VelaHaptic.detent.play()
    }
}

#Preview("Index rail dark") {
    HStack {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            ContactLetterHeader(letter: "A")
            ContactLetterHeader(letter: "B")
        }
        Spacer()
        AlphaIndexRail(
            letters: ContactsFixtures.indexLetters,
            populated: Set(ContactsFixtures.sectionLetters),
            pinnedBubble: "H"
        )
    }
    .padding(.vertical, Tokens.Space.s24)
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
}

#Preview("Index rail light") {
    HStack {
        Spacer()
        AlphaIndexRail(
            letters: ContactsFixtures.indexLetters,
            populated: Set(ContactsFixtures.sectionLetters)
        )
    }
    .themed(.light)
}
