// The window: a sidebar with the three screens, and what floats over them
// (questions from apps, offers from people, toasts).

import AppKit
import CoreImage.CIFilterBuiltins
import SwiftUI

enum Screen: String, CaseIterable, Identifiable, Hashable {
    case network = "Network", access = "Access", settings = "Settings"
    var id: Self { self }
    /// Its line icon (Kit.swift).
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
    /// Narrower than 600 points: a slimmer rail and pages (`compact`).
    @State private var compact = false

    private var modal: Modal? {
        if let a = shell.ask { return a.kind.string == "share" ? .share(a) : .ask(a) }
        if let o = shell.offer { return .offer(o.share, person: o.person) }
        return nil
    }

    var body: some View {
        HStack(spacing: 0) {
            Rail(compact: compact)
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
        .onGeometryChange(for: Bool.self) { $0.size.width < 600 } action: { compact = $0 }
        .environment(\.compact, compact)
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

    private func badge(_ s: Screen) -> Int {
        if s == .access { return shell.offers.count + shell.asks.count }
        return 0
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

/// UUOB's rail: each screen an icon in a soft tile with its name under it,
/// a dot when there is news, and the lock at the bottom.
struct Rail: View {
    @Environment(Shell.self) private var shell
    /// A small window: icons only, in a slimmer rail.
    var compact = false

    var body: some View {
        VStack(spacing: 4) {
            ForEach(Screen.allCases) { s in
                let on = (shell.screen ?? .network) == s
                Button { shell.screen = s } label: {
                    VStack(spacing: 4) {
                        Ico(s.icon, size: 20)
                            .frame(width: 40, height: 40)
                            .background(on ? Look.active : .clear, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                            .overlay(alignment: .topTrailing) {
                                if let dot = dot(s) {
                                    Circle().fill(dot).frame(width: 8, height: 8).overlay(Circle().strokeBorder(Look.elevated, lineWidth: 2)).padding(6)
                                }
                            }
                        if !compact { Text(s.rawValue).font(.system(size: 11, weight: .semibold)) }
                    }
                    .foregroundStyle(on ? Look.text : Look.text2)
                    .frame(width: compact ? 52 : 76).padding(.vertical, compact ? 2 : 6)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(s.rawValue)
                .accessibilityLabel(s.rawValue)
            }
            Spacer()
            Ico("lock", size: 16).foregroundStyle(Look.text3).padding(8)
                .help("End-to-end encrypted. Your data is stored only on your own devices.")
        }
        .padding(.top, 44).padding(.bottom, 14)
        .frame(width: compact ? 60 : 84)
        .frame(maxHeight: .infinity)
        .background(Look.rail)
        .overlay(alignment: .trailing) { Rectangle().fill(Look.line).frame(width: 1) }
    }

    private func dot(_ s: Screen) -> Color? {
        switch s {
        case .network where shell.status.devices_total.int > 1: shell.status.devices_online.int > 1 ? Look.ok : Look.text3
        case .access where !shell.asks.isEmpty || !shell.offers.isEmpty: Look.warn
        default: nil
        }
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

/// A sheet's frame: a title, a line, the content and the buttons at the bottom.
struct SheetFrame<Content: View, Buttons: View>: View {
    let title: String
    var lead: String? = nil
    var center = false
    @ViewBuilder var content: Content
    @ViewBuilder var buttons: Buttons

    var body: some View {
        VStack(alignment: center ? .center : .leading, spacing: 14) {
            Text(title).font(.system(size: 22, weight: .semibold)).tracking(-0.44).multilineTextAlignment(center ? .center : .leading)
            if let lead { Text(lead).font(.system(size: 17)).foregroundStyle(Look.text2).multilineTextAlignment(center ? .center : .leading).fixedSize(horizontal: false, vertical: true) }
            content
            HStack(spacing: 10) {
                buttons
            }
            .environment(\.fillButtons, true)
            .padding(.top, 8)
        }
        .padding(28)
        .frame(width: 460)
        .background(Look.elevated)
        .foregroundStyle(Look.text)
        .buttonStyle(UuobButton(large: true))
        .toggleStyle(UuobSwitch())
    }
}
