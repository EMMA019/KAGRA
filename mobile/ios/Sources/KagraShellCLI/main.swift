import KagraShell

@main
struct KagraShellCLI {
    static func main() {
        guard let session = KagraSession() else {
            fputs("failed to create session\n", stderr)
            return
        }
        session.createSurface(width: 390, height: 844)
        session.setPad(x: 0.5, y: 0)
        let f1 = session.requestFrame()
        let f2 = session.requestFrame()
        print("kagra-shared \(session.version)")
        print("frames: \(f1) -> \(f2)")
        print(session.statsJSON())
    }
}
