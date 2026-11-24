import SwiftUI

/// One AI (Access → an AI): what it may read and write, app by app.
struct AiView: View {
    let only: String
    @Environment(Shell.self) private var shell
    @Environment(\.dismiss) private var dismiss
    @State private var clients: [(key: String, value: J)] = []
    @State private var containers: [J] = []
    @State private var url = ""
    @State private var connecting = false
    @State private var removing: (key: String, value: J)?

    private func load() async {
        clients = ((try? await Core.call("clients"))?.entries ?? []).filter { $0.value.app.isNull && $0.key == only }
        containers = (try? await Core.call("containers"))?.array ?? []
        url = (try? await Core.call("mcp_url"))?.string ?? ""
    }

    /// Access is given per app and covers all of that app's lists, calendars, …
    private var apps: [J] {
        var seen = Set<String>()
        return containers.filter { seen.insert($0.owner_app.string).inserted }
    }

    private func appName(_ k: J) -> String { containers.first { $0.id == k.owner_app.string }?.name.string ?? k.name.string }
    private func level(_ cid: String, _ k: J) -> String { k.access.array.first { $0.client.string == "ai:\(cid)" }?.access.string ?? "none" }

    var body: some View {
        Page(title: clients.first?.value.name.string ?? "", back: { dismiss() }) {
            ForEach(clients, id: \.key) { client in
                Card {
                    HStack(spacing: 12) {
                        RoundedRectangle(cornerRadius: 11, style: .continuous).fill(Color.purple)
                            .frame(width: 40, height: 40).overlay(Ico("ai", size: 20).foregroundStyle(.white))
                        TwoLines(title: client.value.name.string, detail: "Connected \(client.value.created_at.date.formatted(date: .abbreviated, time: .omitted))")
                        Spacer()
                        Button(role: .destructive) { removing = client } label: { Ico("trash", size: 18) }.buttonStyle(IconButtonStyle()).uuobHint("Disconnect")
                    }
                    ForEach(apps) { k in
                        Trailing {
                            Segmented([("none", "No access"), ("read", "Read"), ("write", "Read & write")], selected: level(client.key, k)) { v in
                                shell.act {
                                    try await Core.call("set_grant", ["client": "ai:\(client.key)", "app": k.owner_app.string, "access": v == "none" ? nil : v])
                                    await load()
                                }
                            }
                        } label: {
                            Text(appName(k)).font(.system(size: 16, weight: .semibold))
                        }
                    }
                }
            }
        }
        .task(id: shell.tick) {
            await load()
            if clients.isEmpty { dismiss() }
        }
        .sheet(isPresented: $connecting, onDismiss: { Task { await load() } }) { ConnectAiSheet(url: url) }
        .confirm("Disconnect “\(removing?.value.name.string ?? "")”?", isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
                 message: "Its key stops working immediately on all your devices.", action: "Disconnect") {
            guard let c = removing else { return }
            shell.act {
                try await Core.call("delete_client", ["id": c.key])
                await load()
            }
        }
    }
}

/// Name a new AI client, then show how to add UUOB to it. The key is shown only once.
struct ConnectAiSheet: View {
    @Environment(Shell.self) private var shell
    @Environment(\.dismiss) private var dismiss
    let url: String
    @State private var created: (name: String, token: String)?

    var body: some View {
        if let created {
            let command = "claude mcp add --transport http uuob \(url) --header \"Authorization: Bearer \(created.token)\""
            let json = """
            {
              "mcpServers": {
                "uuob": {
                  "type": "http",
                  "url": "\(url)",
                  "headers": { "Authorization": "Bearer \(created.token)" }
                }
              }
            }
            """
            DetailSheet(title: "“\(created.name)” is ready", onClose: { dismiss() }, actions: { EmptyView() }) {
                SheetLead("Add UUOB to your AI with one of these. The key is shown only once. Then choose what it can access.")
                VStack(spacing: 16) {
                    Snippet(label: "Claude Code", text: command) { copy(command); shell.show("Command copied") }
                    Snippet(label: "Other MCP apps (JSON)", text: json) { copy(json); shell.show("Config copied") }
                }
                .padding(.top, 16)
            }
        } else {
            NameEditor(placeholder: "Name, e.g. Claude on my laptop", under: "An AI. It starts with no access.", isNew: true, saveLabel: "Create") { name in
                let r = try await Core.call("create_client", ["name": name])
                created = (name: name, token: r.token.string)
            }
        }
    }
}

private struct Snippet: View {
    let label: String
    let text: String
    let copy: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(label).font(.system(size: 15, weight: .semibold))
                Spacer()
                Button(action: copy) { HStack(spacing: 6) { Ico("copy", size: 16); Text("Copy") } }.buttonStyle(UuobButton(small: true))
            }
            Text(text).font(.system(.callout, design: .monospaced)).textSelection(.enabled)
                .padding(12).frame(maxWidth: .infinity, alignment: .leading)
                .background(Look.surface2, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
    }
}
