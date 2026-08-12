// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "KagraShell",
    platforms: [
        .iOS(.v16),
        .macOS(.v13),
    ],
    products: [
        .library(name: "KagraShell", targets: ["KagraShell"]),
        .executable(name: "kagra-shell-cli", targets: ["KagraShellCLI"]),
    ],
    targets: [
        .target(
            name: "KagraSharedC",
            path: "Sources/KagraSharedC",
            publicHeadersPath: "include"
        ),
        .target(
            name: "KagraShell",
            dependencies: ["KagraSharedC"],
            path: "Sources/KagraShell"
        ),
        .executableTarget(
            name: "KagraShellCLI",
            dependencies: ["KagraShell"],
            path: "Sources/KagraShellCLI"
        ),
        .testTarget(
            name: "KagraShellTests",
            dependencies: ["KagraShell"],
            path: "Tests/KagraShellTests"
        ),
    ]
)
