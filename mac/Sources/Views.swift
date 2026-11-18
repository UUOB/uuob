// The window: a sidebar with the four screens, and what floats over them
// (questions from apps, offers from people, toasts).

import AppKit
import CoreImage.CIFilterBuiltins
import SwiftUI

enum Screen: String, CaseIterable, Identifiable {
    case devices = "Devices", people = "People", apps = "Apps", ai = "AI access"
    var id: String { rawValue }
    var icon: String {
        switch self {
        case .devices: "laptopcomputer.and.iphone"
        case .people: "person.2"
        case .apps: "square.grid.2x2"
        case .ai: "sparkles"
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
    /// UUOB_SCREEN=people opens on that screen (the screenshots in mac.yml).
    @State private var screen: Screen? = Screen.allCases.first { "\($0)" == ProcessInfo.processInfo.environment["UUOB_SCREEN"] } ?? .devices

    private var modal: Modal? {
        if let a = shell.ask { return a.kind.string == "share" ? .share(a) : .ask(a) }
        if let o = shell.offer { return .offer(o.share, person: o.person) }
        return nil
    }

    var body: some View {
        NavigationSplitView {
            List(Screen.allCases, selection: $screen) { s in
                Label(s.rawValue, systemImage: s.icon)
                    .badge(badge(s))
                    .tag(Optional(s))
            }
            .navigationSplitViewColumnWidth(min: 180, ideal: 200)
            .safeAreaInset(edge: .bottom) {
                Label("End-to-end encrypted", systemImage: "lock.fill")
                    .font(.caption).foregroundStyle(.secondary)
                    .padding(12).frame(maxWidth: .infinity, alignment: .leading)
                    .help("Your data is stored only on your own devices.")
            }
        } detail: {
            Group {
                switch screen ?? .devices {
                case .devices: DevicesView()
                case .people: PeopleView()
                case .apps: AppsView()
                case .ai: AiView()
                }
            }
            .safeAreaInset(edge: .top) {
                if let e = shell.serverError {
                    Text("\(e) Close other copies of UUOB and open it again.")
                        .padding(10).frame(maxWidth: .infinity)
                        .background(.red.opacity(0.15)).foregroundStyle(.red)
                }
            }
            .toolbar {
                if let v = shell.update {
                    ToolbarItem(placement: .primaryAction) {
                        Button(shell.updating ? "Updating…" : shell.updateError != nil ? "Try again" : "Update to \(v)") { shell.installUpdate() }
                            .buttonStyle(.borderedProminent)
                            .disabled(shell.updating)
                            .help(shell.updateError ?? "Installs UUOB \(v) and restarts it")
                    }
                }
            }
        }
        .overlay(alignment: .bottom) {
            if let t = shell.toast {
                Text(t).font(.callout.weight(.medium))
                    .padding(.horizontal, 18).padding(.vertical, 10)
                    .background(.regularMaterial, in: Capsule())
                    .shadow(radius: 8, y: 4)
                    .padding(.bottom, 24)
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
        if s == .people { return shell.offers.count }
        return 0
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
    var body: some View { Circle().fill(on ? Color.green : Color.secondary).frame(width: 8, height: 8) }
}

/// Every app gets a tile the same way: a colour from its id and its first letter. UUOB knows no app in particular.
func appColor(_ app: String) -> Color {
    let colors: [Color] = [.blue, .red, .orange, .green, .purple, .pink, .teal, .indigo]
    // A stable hash (Swift's own changes every launch).
    let hash = app.unicodeScalars.reduce(0) { ($0 &* 31 &+ Int($1.value)) & 0x7fffffff }
    return colors[hash % colors.count]
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
        Circle().fill((mine ? Color.accentColor : Color.teal).gradient)
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
            Text(title)
            Text(detail).font(.callout).foregroundStyle(.secondary)
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
            Text(title).font(.title2.weight(.semibold)).multilineTextAlignment(center ? .center : .leading)
            if let lead { Text(lead).foregroundStyle(.secondary).multilineTextAlignment(center ? .center : .leading).fixedSize(horizontal: false, vertical: true) }
            content
            HStack {
                Spacer()
                buttons
            }
            .padding(.top, 6)
        }
        .padding(24)
        .frame(width: 480)
    }
}
