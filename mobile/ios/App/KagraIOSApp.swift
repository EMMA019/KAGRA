import SwiftUI
import UIKit
import KagraShell

/// Xcode で iOS App ターゲットを作るとき、このファイルを App に追加する。
/// 本物の `libkagra_shared.a` をリンクすると、この画面に共有コアが直接描画する。
@main
struct KagraIOSApp: App {
    var body: some Scene {
        WindowGroup { ShellView() }
    }
}

/// `CAMetalLayer` を backing layer に持つ view。wgpu はこの layer に描く。
final class KagraMetalView: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }

    var onTouch: ((UInt32, CGPoint, UInt32) -> Void)?

    /// レイアウト後のピクセルサイズ。共有コアへ渡す座標系はこれに合わせる。
    var pixelSize: (UInt32, UInt32) {
        let scale = layer.contentsScale
        return (UInt32(bounds.width * scale), UInt32(bounds.height * scale))
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        layer.contentsScale = window?.screen.scale ?? UIScreen.main.scale
        if let metal = layer as? CAMetalLayer {
            metal.drawableSize = CGSize(
                width: bounds.width * layer.contentsScale,
                height: bounds.height * layer.contentsScale
            )
        }
    }

    private func report(_ touches: Set<UITouch>, phase: UInt32) {
        let scale = layer.contentsScale
        for (i, t) in touches.enumerated() {
            let p = t.location(in: self)
            onTouch?(UInt32(i), CGPoint(x: p.x * scale, y: p.y * scale), phase)
        }
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        report(touches, phase: 0)
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        report(touches, phase: 1)
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        report(touches, phase: 2)
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        report(touches, phase: 3)
    }
}

struct ShellView: View {
    @StateObject private var model = ShellModel()

    var body: some View {
        ZStack(alignment: .topLeading) {
            MetalHost(model: model).ignoresSafeArea()

            if !model.rendering {
                // stub リンク時。ここに来たら描画はまだ配線されていない。
                Color(red: 0.11, green: 0.13, blue: 0.19).ignoresSafeArea()
                Text(model.status)
                    .font(.system(.body, design: .monospaced))
                    .foregroundStyle(.white)
                    .padding()
            } else {
                Text(model.status)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundStyle(.white.opacity(0.75))
                    .padding(12)
            }
        }
        .onDisappear { model.stop() }
    }
}

private struct MetalHost: UIViewRepresentable {
    let model: ShellModel

    func makeUIView(context: Context) -> KagraMetalView {
        let view = KagraMetalView()
        view.isMultipleTouchEnabled = true
        view.onTouch = { [weak model] id, point, phase in
            model?.onTouch(id: id, point: point, phase: phase)
        }
        return view
    }

    func updateUIView(_ view: KagraMetalView, context: Context) {
        model.bind(view: view)
    }
}

@MainActor
final class ShellModel: ObservableObject {
    @Published var status = "starting…"
    @Published var rendering = false

    private var session: KagraSession?
    private var displayLink: CADisplayLink?
    private weak var view: KagraMetalView?
    private var attachedSize: (UInt32, UInt32) = (0, 0)

    /// view が確定してから attach する。サイズ変更（回転）でも作り直す。
    func bind(view: KagraMetalView) {
        self.view = view
        if session == nil {
            session = KagraSession()
        }
        let (w, h) = view.pixelSize
        guard w > 0, h > 0 else { return }
        guard (w, h) != attachedSize else { return }

        let ptr = Unmanaged.passUnretained(view).toOpaque()
        session?.detachSurface()
        rendering = session?.attach(view: ptr, width: w, height: h) ?? false
        attachedSize = (w, h)
        if !rendering {
            status = """
                kagra-shared \(session?.version ?? "?")
                renderer unavailable: \(session?.lastError ?? "")

                Link libkagra_shared.a built with --features render
                (see mobile/README.md)
                """
        }
        startLoop()
    }

    private func startLoop() {
        guard displayLink == nil else { return }
        let link = CADisplayLink(target: self, selector: #selector(tick))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    func stop() {
        displayLink?.invalidate()
        displayLink = nil
        session?.detachSurface()
        session = nil
        attachedSize = (0, 0)
        rendering = false
    }

    @objc private func tick() {
        guard let session else { return }
        let frame = session.requestFrame()
        if rendering, !session.render() {
            rendering = false
            status = "render failed: \(session.lastError)"
            return
        }
        if frame % 15 == 0 {
            status = "kagra-shared \(session.version)  frame=\(frame)"
        }
    }

    func onTouch(id: UInt32, point: CGPoint, phase: UInt32) {
        guard let session, let view else { return }
        session.pushPointer(
            id: id,
            x: Float(point.x),
            y: Float(point.y),
            phase: phase,
            pressure: phase == 2 || phase == 3 ? 0 : 1
        )
        // 画面中央を原点にした仮想スティックとして扱う。
        let (w, h) = view.pixelSize
        if phase == 2 || phase == 3 {
            session.setPad(x: 0, y: 0)
        } else if w > 0, h > 0 {
            session.setPad(
                x: Float(point.x / CGFloat(w) * 2 - 1).clamped(to: -1...1),
                y: Float(point.y / CGFloat(h) * 2 - 1).clamped(to: -1...1)
            )
        }
    }
}

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}
