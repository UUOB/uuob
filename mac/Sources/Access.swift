// Access: every app, person and AI that can reach your data, with what each
// can read or write (`access`, crates/uuob-app/src/screens.rs). Clicking one
// opens what Apps, People and AI access showed for it before.

import SwiftUI

struct AccessView: View {
    @Environment(Shell.self) private var shell
    @State private var rows: [J] = []
    @State private var show = "all"
    @State private var path: [String] = []
    @State private var adding: Adding?
    @State private var naming: Adding?
    @State private var url = ""

    enum Adding: String, Identifiable {
        case app, showCode, enterCode, ai
        var id: String { rawValue }
    }

    private var shown: [J] { rows.filter { show == "all" || $0.kind.string == show } }

    private func load() async {
        rows = (try? await Core.call("access"))?.array ?? []
        url = (try? await Core.call("mcp_url"))?.string ?? ""
    }

    /// Linking with a person needs a name for them to see.
    private func person(_ what: Adding) {
        Task {
            let me = (try? await Core.call("status"))?.network.name.string ?? ""
            if me.trimmingCharacters(in: .whitespaces).isEmpty { naming = what } else { adding = what }
        }
    }

    var body: some View {
        NavigationStack(path: $path) {
            Page(title: "Access", lead: "Everything that can reach your data, and what each one can read or write.") {
                Menu {
                    Button("An App…") { adding = .app }
                    Menu("A Person") {
                        Button("Show My Code…") { person(.showCode) }
                        Button("Enter Their Code…") { person(.enterCode) }
                    }
                    Button("An AI…") { adding = .ai }
                } label: {
                    HStack(spacing: 8) { Ico("plus", size: 18, stroke: 2.2); Text("Add").font(.system(size: 15, weight: .semibold)) }
                        .foregroundStyle(.white).padding(.horizontal, 18).frame(minHeight: 40).background(Look.accent, in: Capsule())
                }
                .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
                .help("Add an app, a person or an AI")
            } content: {
                Segmented([("all", "All"), ("app", "Apps"), ("person", "People"), ("ai", "AI")], selected: show, stretch: true) { show = $0 }
                    .padding(.top, 14)
                if rows.isEmpty {
                    VStack(spacing: 8) {
                        Ico("shield", size: 44, stroke: 1.5).foregroundStyle(Look.text3)
                        Text("Nothing has access yet").font(.system(size: 17, weight: .semibold))
                        Text("Install an app that works with UUOB on this Mac and open it: it asks to connect. You can also link with a person or connect an AI with Add.")
                            .font(.system(size: 15)).foregroundStyle(Look.text2).multilineTextAlignment(.center)
                    }
                    .frame(maxWidth: .infinity).padding(.vertical, 64).padding(.horizontal, 24)
                } else {
                    Card {
                        ForEach(shown, id: \.key.string) { r in
                            NavigationLink(value: r.key.string) {
                                HStack {
                                    AccessRow(row: r)
                                    Spacer()
                                    Ico("chevron", size: 18).foregroundStyle(Look.text3)
                                }
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
            .navigationDestination(for: String.self) { key in
                let id = String(key.drop { $0 != ":" }.dropFirst())
                Group {
                    if key.hasPrefix("app:") { AppsView(only: id) }
                    else if key.hasPrefix("person:") { PeopleView(only: id) }
                    else { AiView(only: id) }
                }
            }
        }
        .task(id: shell.tick) { await load() }
        .sheet(item: $adding, onDismiss: { Task { await load() } }) { a in
            switch a {
            case .app: AppCodeSheet()
            case .showCode: AddPersonSheet()
            case .enterCode: MeetSheet()
            case .ai: ConnectAiSheet(url: url)
            }
        }
        .sheet(item: $naming) { next in
            NameSheet(current: "", continues: true) { name in
                try await Core.call("set_name", ["name": name])
                naming = nil
                // One sheet after the other: let the first close.
                try? await Task.sleep(for: .milliseconds(400))
                adding = next
            }
        }
    }
}

/// One app, person or AI: its tile, name, kind, and what it can do.
private struct AccessRow: View {
    let row: J

    var body: some View {
        HStack(spacing: 12) {
            switch row.kind.string {
            case "app": AppTile(app: row.id, size: 36)
            case "person": Avatar(name: row.name.string, size: 36)
            default:
                RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.purple)
                    .frame(width: 36, height: 36).overlay(Ico("ai", size: 20).foregroundStyle(.white))
            }
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(row.name.string).font(.system(size: 16, weight: .semibold))
                    Text(badge).font(.system(size: 11, weight: .bold)).foregroundStyle(tint).padding(.horizontal, 7).padding(.vertical, 2)
                        .background(tint.opacity(0.16), in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                    if row.kind.string != "person", !row["where"].string.isEmpty {
                        Text(row["where"].string).font(.system(size: 13)).foregroundStyle(Look.text3)
                    }
                }
                Text(row.summary.string).font(.system(size: 13)).foregroundStyle(Look.text2)
            }
        }
        .padding(.vertical, 2)
    }

    private var badge: String { ["app": "APP", "person": "PERSON"][row.kind.string] ?? "AI" }
    private var tint: Color { ["app": .blue, "person": .teal][row.kind.string] ?? .purple }
}
