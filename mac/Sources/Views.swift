// The window: a sidebar with the three screens, and what floats over them
// (questions from apps, offers from people, toasts).

import AppKit
import CoreImage.CIFilterBuiltins
import SwiftUI

enum Screen: String, CaseIterable, Identifiable, Hashable {
    case network = "Network", access = "Access", settings = "Settings"
    var id: Self { self }
    /// Its key with ⌘ (Go → Network ⌘1 …).
    var number: Int { (Self.allCases.firstIndex(of: self) ?? 0) + 1 }
    /// Its line icon (the kit's `Ico`).
    var icon: String {
        switch self {
        case .network: "network"
        case .access: "shield"
        case .settings: "settings"
        }
    }
}

/// What floats over the window, one at a time.
enum Modal: Identifiable {
    case ask(J), share(J), offer(J, person: J)
    var id: String {
        switch self {
        case .ask(let a), .share(let a): "ask:\(a.id)"
        case .offer(let s, _): "offer:\(s.container.string)"
        }
    }
}

struct ContentView: View {
    @Environment(Shell.self) private var shell
    private var modal: Modal? {
        if let a = shell.ask { return a.kind.string == "share" ? .share(a) : .ask(a) }
        if let o = shell.offer { return .offer(o.share, person: o.person) }
        return nil
    }

    var body: some View {
        SidebarScaffold {
            SideList()
        } rail: {
            RailList()
        } content: {
            Group {
                switch shell.screen ?? .network {
                case .network: NetworkView()
                case .access: AccessView()
                case .settings: SettingsView()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .overlay(alignment: .top) { UpdatePill() }
            .safeAreaInset(edge: .top) {
                if let e = shell.serverError {
                    Text("\(e) Close other copies of UUOB and open it again.")
                        .font(.system(size: 15)).padding(12).frame(maxWidth: .infinity)
                        .background(Look.dangerSoft).foregroundStyle(Look.danger)
                }
            }
        }
        .background(Look.bg)
        .ignoresSafeArea(.container, edges: .top)
        .frame(minWidth: 360, minHeight: 420)
        // The shared keys: the palette (⌘K), the list (⌘?), C to add, ? (the kit's).
        .uuobKeys(UuobCommands.uuob(shell))
        .overlay(alignment: .bottom) {
            if let t = shell.toast {
                // UUOB's toast: a small pill.
                Text(t).font(.system(size: 15, weight: .medium)).foregroundStyle(Look.text)
                    .padding(.horizontal, 20).padding(.vertical, 12)
                    .background(Look.elevated, in: Capsule())
                    .overlay(Capsule().strokeBorder(Look.lineStrong))
                    .shadow(color: .black.opacity(0.4), radius: 14, y: 8)
                    .padding(.bottom, 28)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.spring(duration: 0.3), value: shell.toast)
        .sheet(item: Binding(get: { modal }, set: { _ in })) { m in
            switch m {
            case .ask(let a): AskSheet(ask: a)
            case .share(let a): ShareSheet(ask: a)
            case .offer(let s, let p): OfferSheet(offer: s, person: p)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            Task { await shell.loadAsks() }
            shell.checkUpdate()
        }
    }

}

/// "Update to 0.4.1", floating at the top of the window when a newer UUOB
/// is out (ui/Update.svelte): one click downloads, checks and installs it.
private struct UpdatePill: View {
    @Environment(Shell.self) private var shell

    var body: some View {
        if let v = shell.update {
            VStack(spacing: 6) {
                Button(shell.updating ? "Updating…" : shell.updateError != nil ? "Try again" : "Update to \(v)") { shell.installUpdate() }
                    .buttonStyle(Pill()).disabled(shell.updating)
                    .help("Installs UUOB \(v) and restarts it")
                if let e = shell.updateError {
                    Text(e).font(.system(size: 13)).foregroundStyle(Look.danger).multilineTextAlignment(.center)
                        .padding(.horizontal, 14).padding(.vertical, 8)
                        .background(Look.elevated, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Look.lineStrong))
                        .shadow(color: .black.opacity(0.3), radius: 12, y: 6)
                }
            }
            .frame(maxWidth: 420)
            .padding(.top, 10)
        }
    }

    /// 34 points high, on one line, blue with a shadow.
    struct Pill: ButtonStyle {
        @Environment(\.isEnabled) private var enabled
        func makeBody(configuration: Configuration) -> some View {
            configuration.label
                .font(.system(size: 14, weight: .semibold)).foregroundStyle(.white)
                .lineLimit(1).fixedSize()
                .padding(.horizontal, 16).frame(height: 34)
                .background(Look.accent.opacity(configuration.isPressed ? 0.85 : 1), in: Capsule())
                .shadow(color: .black.opacity(0.3), radius: 12, y: 6)
                .scaleEffect(configuration.isPressed ? 0.97 : 1)
                .opacity(enabled ? 1 : 0.7)
                .contentShape(Capsule())
        }
    }
}

/// The dot on a page's icon: devices online (green) or not, something to answer in Access.
func screenDot(_ shell: Shell, _ s: Screen) -> Color? {
    switch s {
    case .network where shell.status.devices_total.int > 1: shell.status.devices_online.int > 1 ? Look.ok : Look.text3
    case .access where !shell.asks.isEmpty || !shell.offers.isEmpty: Look.warn
    default: nil
    }
}

private let encrypted = "End-to-end encrypted. Your data is stored only on your own devices."

/// The expanded sidebar: the three pages with their names, the lock at the bottom.
private struct SideList: View {
    @Environment(Shell.self) private var shell

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(Screen.allCases) { s in
                SidebarRow(title: s.rawValue, selected: (shell.screen ?? .network) == s, leading: { Ico(s.icon, size: 19) }, trailing: {
                    if let dot = screenDot(shell, s) { Circle().fill(dot).frame(width: 8, height: 8) }
                }) { shell.screen = s }
                .help("\(s.rawValue) (⌘\(s.number))")
            }
            Spacer(minLength: 0)
            HStack(spacing: 8) {
                Ico("lock", size: 14)
                Text("End-to-end encrypted").font(.system(size: 12))
            }
            .foregroundStyle(Look.text3).padding(.horizontal, 10).help(encrypted)
        }
        .padding(.horizontal, 12).padding(.top, 52).padding(.bottom, 4)
        .frame(maxHeight: .infinity, alignment: .top)
    }
}

/// The rail: the pages as icons, names as tooltips.
private struct RailList: View {
    @Environment(Shell.self) private var shell

    var body: some View {
        VStack(spacing: 6) {
            ForEach(Screen.allCases) { s in
                RailButton(help: "\(s.rawValue) (⌘\(s.number))", selected: (shell.screen ?? .network) == s, mark: {
                    Ico(s.icon, size: 20).overlay(alignment: .topTrailing) {
                        if let dot = screenDot(shell, s) {
                            Circle().fill(dot).frame(width: 8, height: 8).overlay(Circle().strokeBorder(Look.rail, lineWidth: 2)).offset(x: 7, y: -7)
                        }
                    }
                }) { shell.screen = s }
            }
            Spacer(minLength: 0)
            Ico("lock", size: 16).foregroundStyle(Look.text3).help(encrypted)
        }
        .padding(.top, 52).padding(.bottom, 4)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
    }
}

// MARK: - Shared pieces

/// The line under a screen's title.
struct Lead: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text).font(.body).foregroundStyle(.secondary).textCase(nil)
            .frame(maxWidth: .infinity, alignment: .leading).padding(.bottom, 6)
    }
}

/// Online or offline.
struct Dot: View {
    let on: Bool
    var body: some View { Circle().fill(on ? Look.ok : Look.text3).frame(width: 9, height: 9) }
}

/// Every app gets a tile the same way: a colour from its id and its first letter. UUOB knows no app in particular.
/// The colour is the same on every platform: FNV-1a of the id (crates/uuob-app/src/screens.rs `tint`).
func appColor(_ app: String) -> Color {
    let colors: [Color] = [.blue, .red, .orange, .green, .purple, .pink, .teal, .indigo]
    let hash = app.utf8.reduce(UInt32(0x811c9dc5)) { ($0 ^ UInt32($1)) &* 0x01000193 }
    return colors[Int(hash % UInt32(colors.count))]
}

struct AppTile: View {
    let app: String
    var size: CGFloat = 36
    var body: some View {
        RoundedRectangle(cornerRadius: size * 0.26, style: .continuous).fill(appColor(app).gradient)
            .frame(width: size, height: size)
            .overlay(Text(initial(app)).font(.system(size: size * 0.46, weight: .semibold, design: .rounded)).foregroundStyle(.white))
    }
}

func initial(_ name: String) -> String {
    name.trimmingCharacters(in: .whitespaces).first.map { String($0).uppercased() } ?? "?"
}

struct Avatar: View {
    let name: String
    var size: CGFloat = 36
    var mine = false
    var body: some View {
        Circle().fill(mine ? AnyShapeStyle(Look.accent) : AnyShapeStyle(Look.person))
            .frame(width: size, height: size)
            .overlay(Text(initial(name)).font(.system(size: size * 0.42, weight: .semibold, design: .rounded)).foregroundStyle(.white))
    }
}

/// A row title with a grey line under it.
struct TwoLines: View {
    let title: String
    let detail: String
    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.system(size: 16, weight: .semibold))
            Text(detail).font(.system(size: 13)).foregroundStyle(Look.text2).fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// A QR code of `text`, black on white so any camera reads it.
struct QrCode: View {
    let text: String
    var size: CGFloat = 150

    var body: some View {
        Image(nsImage: image).interpolation(.none).resizable()
            .frame(width: size, height: size)
            .padding(14).background(.white, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
    }

    private var image: NSImage {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(text.utf8)
        filter.correctionLevel = "M"
        guard let out = filter.outputImage, let cg = CIContext().createCGImage(out, from: out.extent) else { return NSImage() }
        return NSImage(cgImage: cg, size: NSSize(width: out.extent.width, height: out.extent.height))
    }
}

/// An invite code with its QR code and a countdown.
struct CodeView: View {
    let invite: J
    let waiting: String
    var qr = true

    var body: some View {
        VStack(spacing: 18) {
            HStack(spacing: 8) {
                ForEach(Array(invite.code.string.split(separator: "-").enumerated()), id: \.offset) { i, part in
                    Text(part).font(.system(size: 22, weight: .semibold, design: .rounded))
                        .padding(.horizontal, 12).padding(.vertical, 9)
                        .background(i == 0 ? Color.accentColor.opacity(0.16) : Color.secondary.opacity(0.12), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                        .foregroundStyle(i == 0 ? Color.accentColor : Color.primary)
                }
            }
            .textSelection(.enabled)
            if qr { QrCode(text: invite.code.string) }
            TimelineView(.periodic(from: .now, by: 1)) { context in
                let left = max(0, Int(invite.expires_at.date.timeIntervalSince(context.date)))
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("\(waiting) Code expires in \(left / 60):\(String(format: "%02d", left % 60))").foregroundStyle(.secondary)
                }
            }
        }
    }
}

/// Four words or more: a code worth trying.
func complete(_ code: String) -> Bool {
    code.split(whereSeparator: { $0 == " " || $0 == "-" }).count >= 4
}

func plural(_ n: Int, _ one: String, _ many: String) -> String { "\(n) \(n == 1 ? one : many)" }

func size(_ bytes: Int) -> String { ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .file) }

func copy(_ text: String) {
    NSPasteboard.general.clearContents()
    NSPasteboard.general.setString(text, forType: .string)
}
