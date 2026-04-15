import Foundation
import HydrationCore

@Observable
final class AppState {
    var config: ConfigReader
    var daemonRunning = false
    var needsOnboarding: Bool

    private var timer: Timer?

    // MARK: - Computed

    var statusIcon: String {
        daemonRunning ? "cloud.fill" : "cloud"
    }

    var statusDescription: String {
        "iCloud NFS Exporter — daemon \(daemonRunning ? "running" : "stopped")"
    }

    // MARK: - Lifecycle

    init() {
        let hasConfig = FileManager.default.fileExists(
            atPath: ConfigReader.configPath.path)
        self.config = ConfigReader.load()
        self.needsOnboarding = !hasConfig
        startPolling()
    }

    deinit {
        timer?.invalidate()
    }

    // MARK: - Actions

    func refresh() {
        config = ConfigReader.load()
        checkStatus()
    }

    func saveConfig(_ updated: ConfigReader) throws {
        try updated.save()
        config = updated
        needsOnboarding = false
    }

    func startPolling() {
        checkStatus()
        timer = Timer.scheduledTimer(
            withTimeInterval: 10, repeats: true
        ) { [weak self] _ in
            self?.checkStatus()
        }
    }

    // MARK: - Private

    private func checkStatus() {
        DispatchQueue.global(qos: .utility).async { [weak self] in
            guard let self else { return }
            let client = IPCClient(socketPath: self.config.socketPath)
            let running = client.isAvailable
            DispatchQueue.main.async {
                self.daemonRunning = running
            }
        }
    }
}
