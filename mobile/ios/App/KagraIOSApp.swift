import SwiftUI
import KagraShell

/// Xcode で iOS App ターゲットを作るとき、このファイルを App に追加する。
@main
struct KagraIOSApp: App {
    var body: some Scene {
        WindowGroup { ShellView() }
    }
}

struct ShellView: View {
    @StateObject private var model = ShellModel()

    var body: some View {
        ZStack {
            Color(red: 0.11, green: 0.13, blue: 0.19).ignoresSafeArea()
            Text(model.status)
                .font(.system(.body, design: .monospaced))
                .foregroundStyle(.white)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .padding()
                .contentShape(Rectangle())
                .gesture(
                    DragGesture(minimumDistance: 0)
                        .onChanged { model.onDrag($0.location) }
                        .onEnded { model.onDragEnd($0.location) }
                )
        }
        .onAppear { model.start() }
        .onDisappear { model.stop() }
    }
}

@MainActor
final class ShellModel: ObservableObject {
    @Published var status = "…"
    private var session: KagraSession?
    private var timer: Timer?

    func start() {
        session = KagraSession()
        session?.createSurface(width: 390, height: 844)
        timer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
    }

    func stop() {
        timer?.invalidate()
        timer = nil
        session = nil
    }

    func tick() {
        guard let session else { return }
        let f = session.requestFrame()
        status = "kagra-shared \(session.version)\nframe=\(f)\n\(session.statsJSON())"
    }

    func onDrag(_ p: CGPoint) {
        session?.pushPointer(id: 1, x: Float(p.x), y: Float(p.y), phase: 1)
        session?.setPad(
            x: Float((p.x / 195) * 2 - 1).clamped(to: -1...1),
            y: Float((p.y / 422) * 2 - 1).clamped(to: -1...1)
        )
    }

    func onDragEnd(_ p: CGPoint) {
        session?.pushPointer(id: 1, x: Float(p.x), y: Float(p.y), phase: 2, pressure: 0)
        session?.setPad(x: 0, y: 0)
    }
}

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}
