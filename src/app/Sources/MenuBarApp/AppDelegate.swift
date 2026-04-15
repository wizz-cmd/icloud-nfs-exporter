import Cocoa
import HydrationCore

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private var timer: Timer?
    private var config = ConfigReader.load()
    private var daemonRunning = false

    // MARK: - Lifecycle

    func applicationDidFinishLaunching(_: Notification) {
        NSApp.setActivationPolicy(.accessory)

        statusItem = NSStatusBar.system.statusItem(
            withLength: NSStatusItem.variableLength)
        updateIcon()
        rebuildMenu()
        startPolling()
    }

    func applicationWillTerminate(_: Notification) {
        timer?.invalidate()
    }

    // MARK: - Polling

    private func startPolling() {
        checkStatus()
        timer = Timer.scheduledTimer(
            withTimeInterval: 10, repeats: true
        ) { [weak self] _ in
            self?.checkStatus()
        }
    }

    private func checkStatus() {
        DispatchQueue.global(qos: .utility).async { [weak self] in
            guard let self else { return }
            let client = IPCClient(socketPath: self.config.socketPath)
            let running = client.isAvailable
            DispatchQueue.main.async {
                if running != self.daemonRunning {
                    self.daemonRunning = running
                    self.updateIcon()
                    self.rebuildMenu()
                }
            }
        }
    }

    // MARK: - UI

    private func updateIcon() {
        let name = daemonRunning ? "cloud.fill" : "cloud"
        statusItem.button?.image = NSImage(
            systemSymbolName: name,
            accessibilityDescription: "iCloud NFS Exporter")
    }

    private func rebuildMenu() {
        let menu = NSMenu()

        // Header
        let title = NSMenuItem(
            title: "iCloud NFS Exporter", action: nil, keyEquivalent: "")
        title.isEnabled = false
        menu.addItem(title)
        menu.addItem(.separator())

        // Daemon status
        addDisabledItem(
            to: menu,
            title: "Daemon: \(daemonRunning ? "Running" : "Stopped")")

        // Folders
        menu.addItem(.separator())
        if config.folders.isEmpty {
            addDisabledItem(to: menu, title: "No folders configured")
        } else {
            for folder in config.folders {
                let label = folder.label.isEmpty
                    ? URL(fileURLWithPath: folder.source).lastPathComponent
                    : folder.label
                addDisabledItem(to: menu, title: label)

                let mount = ConfigReader.mountPointFor(
                    source: folder.source, mountBase: config.mountBase)
                addDisabledItem(to: menu, title: "  \u{21B3} \(mount)")
            }
        }

        // Actions
        menu.addItem(.separator())

        let refreshItem = NSMenuItem(
            title: "Refresh", action: #selector(refresh), keyEquivalent: "r")
        refreshItem.target = self
        menu.addItem(refreshItem)

        let configItem = NSMenuItem(
            title: "Open Config\u{2026}",
            action: #selector(openConfig), keyEquivalent: ",")
        configItem.target = self
        menu.addItem(configItem)

        let diagnoseItem = NSMenuItem(
            title: "Run Diagnostics\u{2026}",
            action: #selector(runDiagnostics), keyEquivalent: "d")
        diagnoseItem.target = self
        menu.addItem(diagnoseItem)

        menu.addItem(.separator())

        let quitItem = NSMenuItem(
            title: "Quit", action: #selector(quit), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        statusItem.menu = menu
    }

    private func addDisabledItem(to menu: NSMenu, title: String) {
        let item = NSMenuItem(title: title, action: nil, keyEquivalent: "")
        item.isEnabled = false
        menu.addItem(item)
    }

    // MARK: - Actions

    @objc private func refresh() {
        config = ConfigReader.load()
        checkStatus()
        rebuildMenu()
    }

    @objc private func openConfig() {
        let path = ConfigReader.configPath
        if FileManager.default.fileExists(atPath: path.path) {
            NSWorkspace.shared.open(path)
        } else {
            let alert = NSAlert()
            alert.messageText = "Config not found"
            alert.informativeText =
                "Run 'icne setup' to create the configuration file."
            alert.runModal()
        }
    }

    @objc private func runDiagnostics() {
        let scripts = Bundle.main.bundlePath
            .components(separatedBy: "/src/app/").first ?? "."
        let icne = "\(scripts)/scripts/icne"
        if FileManager.default.isExecutableFile(atPath: icne) {
            let task = Process()
            task.executableURL = URL(fileURLWithPath: "/usr/bin/open")
            task.arguments = [
                "-a", "Terminal",
                icne, "diagnose",
            ]
            try? task.run()
        }
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }
}
