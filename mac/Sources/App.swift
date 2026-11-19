// UUOB for the Mac: SwiftUI screens over the Rust core, in a window that
// AppKit keeps, plus the menu bar icon. Closing the window keeps UUOB running
// (apps and your other devices use it); Quit stops it.

import AppKit
import SwiftUI

@main
struct UUOBApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var delegate

    var body: some Scene {
        MenuBarExtra("UUOB", systemImage: "point.3.connected.trianglepath.dotted") {
            Button("Open UUOB") { delegate.showWindow() }
            Divider()
            Button("Quit UUOB") { NSApp.terminate(nil) }.keyboardShortcut("q")
        }
        .commands {
            // Every screen from the menu bar and the keyboard: Go → Network (⌘1) …
            CommandMenu("Go") {
                ForEach(Array(Screen.allCases.enumerated()), id: \.element) { i, s in
                    Button(s.rawValue) {
                        delegate.shell.screen = s
                        delegate.showWindow()
                    }
                    .keyboardShortcut(KeyEquivalent(Character(String(i + 1))))
                }
            }
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let shell = Shell()
    private var window: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let content = NSHostingController(rootView: ContentView().environment(shell))
        content.sceneBridgingOptions = [.toolbars, .title]
        let window = NSWindow(contentViewController: content)
        window.title = "UUOB"
        window.styleMask = [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView]
        window.toolbarStyle = .unified
        window.setContentSize(NSSize(width: 1000, height: 720))
        window.contentMinSize = NSSize(width: 760, height: 520)
        // Closing only hides it; showWindow brings the same window back.
        window.isReleasedWhenClosed = false
        window.center()
        window.setFrameAutosaveName("main")
        self.window = window
        shell.onAskHere = { [weak self] in self?.showWindow() }
        // Started at login (see Settings → Start at login) UUOB stays in the menu bar.
        if !CommandLine.arguments.contains("--hidden") { showWindow() }
        Task { await shell.run() }
    }

    func showWindow() {
        NSApp.activate(ignoringOtherApps: true)
        window?.makeKeyAndOrderFront(nil)
    }

    // Clicking the Dock icon brings the window back.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows: Bool) -> Bool {
        if !hasVisibleWindows { showWindow() }
        return true
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
}

/// What the whole app shows: the network, questions from apps, offers from people, toasts, updates.
@Observable
@MainActor
final class Shell {
    var status = J(nil)
    var people: [J] = []
    /// Questions from apps, answered here.
    var asks: [J] = []
    /// Offers put aside until UUOB is opened again.
    var later: Set<String> = []
    var serverError: String?
    /// The screen in the window. UUOB_SCREEN=access opens on that one (the screenshots in mac.yml).
    var screen: Screen? = Screen.allCases.first { "\($0)" == ProcessInfo.processInfo.environment["UUOB_SCREEN"] } ?? .network
    /// Bumped on network changes, so screens reload.
    var tick = 0
    /// The latest change, for a screen waiting for one (a device joining).
    var lastEvent = J(nil)
    var events = 0
    var toast: String?
    var update: String?
    var updating = false
    var updateError: String?
    /// An app asked something on this Mac: bring the window forward.
    var onAskHere: () -> Void = {}

    private var toastTask: Task<Void, Never>?
    private var checkedUpdate = Date.distantPast

    var ask: J? { asks.first }
    /// Things other people offered to share, waiting for an answer.
    var offers: [(share: J, person: J)] {
        people.flatMap { p in p.receiving.array.filter { $0.offered.bool }.map { (share: $0, person: p) } }
    }
    var offer: (share: J, person: J)? { offers.first { !later.contains($0.share.container.string) } }

    func show(_ text: String) {
        toast = text
        toastTask?.cancel()
        toastTask = Task {
            try? await Task.sleep(for: .seconds(3.2))
            if !Task.isCancelled { toast = nil }
        }
    }

    /// Runs `work`; a failure shows as a toast.
    func act(_ work: @escaping @MainActor () async throws -> Void) {
        Task {
            do { try await work() } catch { show(error.localizedDescription) }
        }
    }

    func refresh() async {
        if let s = try? await Core.call("status") { status = s }
        if let p = try? await Core.call("people") { people = p.array }
        if let e = try? await Core.call("server_error") { serverError = e.str }
    }

    func loadAsks() async {
        guard let list = try? await Core.call("asks") else { return }
        let before = Set(asks.map(\.id))
        asks = list.array
        if asks.contains(where: { $0.here.bool && !before.contains($0.id) }) { onAskHere() }
    }

    /// Starts UUOB and follows its changes for as long as the app runs.
    func run() async {
        do {
            try await Core.start()
        } catch {
            serverError = error.localizedDescription
        }
        await refresh()
        await loadAsks()
        // The screens may have loaded before UUOB was running: now they can.
        tick += 1
        checkUpdate()
        for await e in Core.events() {
            lastEvent = e
            events += 1
            switch e.type.string {
            case "container_changed": continue
            case "asks_changed":
                await loadAsks()
                continue
            case "device_joined": show("\(e.name.string) joined your network")
            case "removed_from_network": show("This device was removed from the network")
            case "lagged": await loadAsks()
            default: break
            }
            await refresh()
            tick += 1
        }
    }

    /// When UUOB starts and when you come back to it, at most hourly.
    func checkUpdate() {
        guard !updating, Date().timeIntervalSince(checkedUpdate) > 3600 else { return }
        checkedUpdate = Date()
        Task { update = (try? await Core.call("update_check"))?.str }
    }

    /// Downloads the new version, checks its signature, puts it in place of this app and restarts.
    func installUpdate() {
        updating = true
        updateError = nil
        Task {
            do {
                let done = try await Core.call("update_install")
                if let app = done.restart.str {
                    let reopen = Process()
                    reopen.executableURL = URL(fileURLWithPath: "/bin/sh")
                    reopen.arguments = ["-c", "sleep 1; open \"$0\"", app]
                    try reopen.run()
                    NSApp.terminate(nil)
                }
            } catch {
                updateError = error.localizedDescription
            }
            updating = false
        }
    }
}
