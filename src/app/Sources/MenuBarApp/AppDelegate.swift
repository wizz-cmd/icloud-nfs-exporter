import Cocoa
import HydrationCore
import SwiftUI

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private var timer: Timer?
    private var config = ConfigReader.load()
    private var daemonRunning = false
    private var settingsWindow: NSWindow?

    // MARK: - Lifecycle

    func applicationDidFinishLaunching(_: Notification) {
        NSApp.setActivationPolicy(.accessory)

        statusItem = NSStatusBar.system.statusItem(
            withLength: NSStatusItem.variableLength)
        updateIcon()
        rebuildMenu()
        startPolling()

        // First launch — open settings if no config exists
        if !FileManager.default.fileExists(atPath: ConfigReader.configPath.path) {
            showSettings()
        }
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

        let settingsItem = NSMenuItem(
            title: "Settings\u{2026}",
            action: #selector(showSettings), keyEquivalent: ",")
        settingsItem.target = self
        menu.addItem(settingsItem)

        let refreshItem = NSMenuItem(
            title: "Refresh", action: #selector(refresh), keyEquivalent: "r")
        refreshItem.target = self
        menu.addItem(refreshItem)

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

    @objc private func showSettings() {
        // Reuse existing window if open
        if let w = settingsWindow, w.isVisible {
            w.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let view = SettingsView(
            config: config,
            onSave: { [weak self] updated in
                self?.saveConfig(updated)
                self?.settingsWindow?.close()
            },
            onCancel: { [weak self] in
                self?.settingsWindow?.close()
            }
        )

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 520, height: 460),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false)
        window.title = "iCloud NFS Exporter Settings"
        window.contentView = NSHostingView(rootView: view)
        window.center()
        window.isReleasedWhenClosed = false
        window.makeKeyAndOrderFront(nil)

        NSApp.activate(ignoringOtherApps: true)
        settingsWindow = window
    }

    private func saveConfig(_ updated: ConfigReader) {
        do {
            try updated.save()
            config = updated
            rebuildMenu()
        } catch {
            let alert = NSAlert()
            alert.messageText = "Failed to save configuration"
            alert.informativeText = error.localizedDescription
            alert.runModal()
        }
    }

    @objc private func refresh() {
        config = ConfigReader.load()
        checkStatus()
        rebuildMenu()
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }
}
