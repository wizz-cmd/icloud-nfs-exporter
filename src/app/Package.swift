// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "MenuBarApp",
    platforms: [.macOS(.v14)],
    dependencies: [
        .package(path: "../hydration"),
    ],
    targets: [
        .executableTarget(
            name: "MenuBarApp",
            dependencies: [
                .product(name: "HydrationCore", package: "Hydration"),
            ]
        ),
    ]
)
