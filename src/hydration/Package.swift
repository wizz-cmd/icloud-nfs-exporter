// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "Hydration",
    platforms: [.macOS(.v13)],
    products: [
        .library(name: "HydrationCore", targets: ["HydrationCore"]),
    ],
    targets: [
        .target(name: "HydrationCore"),
        .executableTarget(
            name: "HydrationDaemon",
            dependencies: ["HydrationCore"]
        ),
        .testTarget(
            name: "HydrationTests",
            dependencies: ["HydrationCore"]
        ),
    ]
)
