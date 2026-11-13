//
//  DemoPageView.swift
//  VelaWallet
//
//  A stand-in for whatever site is open (spec 022 §2). Deliberately NOT
//  chrome: its words and its pink button belong to the SITE, so nothing here
//  is translated and nothing here uses a Vela colour token — the palette
//  sits in BrandPalette.DemoPage with the other content colours. A real
//  WKWebView replaces this view wholesale.
//

import SwiftUI

struct DemoPageView: View {
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    let page: DemoPageModel
    var onAction: () -> Void = {}

    var body: some View {
        VStack(spacing: Tokens.Space.s12) {
            VStack(alignment: .leading, spacing: Tokens.Space.s12) {
                Text(verbatim: page.title)
                    .font(Face.semiBold.font(Tokens.TextSize.t15, relativeTo: .body, at: dynamicTypeSize))
                    .foregroundStyle(BrandPalette.DemoPage.ink)
                ForEach(Array(page.fields.enumerated()), id: \.offset) { _, field in
                    HStack {
                        Text(verbatim: field.value)
                            .font(Face.regular.font(Tokens.TextSize.t20, relativeTo: .body, at: dynamicTypeSize))
                            .foregroundStyle(BrandPalette.DemoPage.ink)
                        Spacer()
                        Text(verbatim: field.symbol)
                            .font(Face.regular.font(Tokens.TextSize.t13, relativeTo: .body, at: dynamicTypeSize))
                            .foregroundStyle(BrandPalette.DemoPage.inkMuted)
                    }
                    .padding(Tokens.Space.s16)
                    .background(BrandPalette.DemoPage.field,
                                in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
                }
                Button(action: onAction) {
                    Text(verbatim: page.cta)
                        .font(Face.semiBold.font(Tokens.TextSize.t15, relativeTo: .body, at: dynamicTypeSize))
                        .foregroundStyle(BrandPalette.DemoPage.card)
                        .frame(maxWidth: .infinity)
                        .frame(height: Tokens.Control.md)
                        .background(page.ctaTint, in: Capsule())
                }
                .buttonStyle(.plain)
            }
            .padding(Tokens.Space.s20)
            .background(BrandPalette.DemoPage.card,
                        in: RoundedRectangle(cornerRadius: Tokens.Radius.r20))

            Capsule().fill(BrandPalette.DemoPage.card)
                .frame(height: Tokens.Space.s8)
                .padding(.horizontal, Tokens.Space.s20)
            Capsule().fill(BrandPalette.DemoPage.card)
                .frame(height: Tokens.Space.s8)
                .padding(.horizontal, Tokens.Space.s48)
            Spacer(minLength: Tokens.Space.s0)
        }
        .padding(Tokens.Space.s24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(BrandPalette.DemoPage.surface)
    }
}
