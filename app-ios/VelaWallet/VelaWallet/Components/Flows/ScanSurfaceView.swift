//
//  ScanSurfaceView.swift
//  VelaWallet
//
//  The scanner (spec 021 component 27) — S1, full screen.
//
//  The frame is four corner brackets and nothing else, so the thing being aimed
//  at stays visible: a closed rectangle around a QR code competes with the
//  code's own quiet zone.
//
//  Spec 055 put a camera behind it. `session` is absent in the gallery and on a
//  device with no camera, and then the frame holds the inert surface it always
//  did — so the drawing is still exactly the drawing.
//

import AVFoundation
import SwiftUI

struct ScanSurfaceView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: ScanModel
    var onClose: () -> Void = {}
    var onTool: (ScanTool) -> Void = { _ in }
    /// The live viewfinder. Absent in the gallery and wherever there is no
    /// camera to show.
    var session: AVCaptureSession?
    /// Why there is no viewfinder, in the core's words. Shown under the frame,
    /// where the aiming hint would otherwise be — because when the camera is
    /// not running, "point the camera at a code" is advice nobody can take.
    var refusalText: String?
    /// The affordance that goes with the refusal: 授予权限, when that is the
    /// thing that would help.
    var refusalAction: (label: String, act: () -> Void)?
    /// Whether the torch is lit, so the control can say so.
    var torchOn = false

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            HStack {
                Spacer()
                Button(action: onClose) {
                    LucideIcon(.close, size: LucideIconSize.flowBack)
                        .foregroundStyle(theme.fgBase)
                        .frame(width: Tokens.Control.sm, height: Tokens.Control.sm)
                        .background(Circle().fill(theme.bgRaised))
                        .contentShape(Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(model.closeLabel)
            }
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
            .padding(.top, Tokens.Space.s16)

            Spacer()
            ScanFrame(session: session)
            Text(verbatim: refusalText ?? model.hint)
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(refusalText == nil ? theme.fgMuted : theme.warningBase)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .padding(.vertical, Tokens.Space.s16)
            if let action = refusalAction {
                Button(action: action.act) {
                    Text(verbatim: action.label)
                        .typeRole(Typography.button.scaled(textScale))
                        .foregroundStyle(theme.accentBase)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
            Spacer()

            HStack(spacing: Tokens.Space.s32) {
                ForEach(model.tools) { tool in
                    Button { onTool(tool.id) } label: {
                        VStack(spacing: Tokens.Space.s4) {
                            LucideIcon(glyph(for: tool.id), size: LucideIconSize.flowScanTool)
                                .foregroundStyle(
                                    tool.id == .torch && torchOn ? theme.accentBase : theme.fgBase
                                )
                                .frame(
                                    width: WalletFlowGeometry.scanToolDisc,
                                    height: WalletFlowGeometry.scanToolDisc
                                )
                                .background(Circle().fill(theme.bgRaised))
                            Text(verbatim: tool.label)
                                .typeRole(Typography.rowSub.scaled(textScale))
                                .foregroundStyle(theme.fgMuted)
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.bottom, Tokens.Space.s48)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.bgBase.ignoresSafeArea())
    }

    private func glyph(for tool: ScanTool) -> LucideGlyph {
        switch tool {
        case .gallery: .image
        case .torch: .zap
        case .flip: .rotateCcw
        }
    }
}

/// Corner brackets over an inert feed placeholder.
private struct ScanFrame: View {
    @Environment(\.theme) private var theme

    var session: AVCaptureSession?

    var body: some View {
        GeometryReader { proxy in
            let side = proxy.size.width
            ZStack {
                if let session {
                    CameraPreviewView(session: session)
                        .clipShape(RoundedRectangle(cornerRadius: Tokens.Radius.r8))
                } else {
                    RoundedRectangle(cornerRadius: Tokens.Radius.r8).fill(theme.bgSunken)
                }
                Path { path in
                    let arm = WalletFlowGeometry.scanBracketArm
                    // top-left
                    path.move(to: CGPoint(x: 0, y: arm))
                    path.addLine(to: CGPoint(x: 0, y: 0))
                    path.addLine(to: CGPoint(x: arm, y: 0))
                    // top-right
                    path.move(to: CGPoint(x: side - arm, y: 0))
                    path.addLine(to: CGPoint(x: side, y: 0))
                    path.addLine(to: CGPoint(x: side, y: arm))
                    // bottom-left
                    path.move(to: CGPoint(x: 0, y: side - arm))
                    path.addLine(to: CGPoint(x: 0, y: side))
                    path.addLine(to: CGPoint(x: arm, y: side))
                    // bottom-right
                    path.move(to: CGPoint(x: side - arm, y: side))
                    path.addLine(to: CGPoint(x: side, y: side))
                    path.addLine(to: CGPoint(x: side, y: side - arm))
                }
                .stroke(
                    theme.fgBase,
                    style: StrokeStyle(
                        lineWidth: WalletFlowGeometry.scanBracketStroke,
                        lineCap: .round,
                        lineJoin: .round
                    )
                )
            }
        }
        .aspectRatio(1, contentMode: .fit)
        .frame(maxWidth: .infinity)
        .padding(.horizontal, Tokens.Layout.screenPaddingX * 2)
    }
}
