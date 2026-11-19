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
        Form {
            ForEach(clients, id: \.key) { client in
                Section {
                    HStack(spacing: 12) {
                        RoundedRectangle(cornerRadius: 10, style: .continuous).fill(LinearGradient(colors: [.purple, .pink], startPoint: .topLeading, endPoint: .bottomTrailing))
                            .frame(width: 36, height: 36).overlay(Image(systemName: "sparkles").foregroundStyle(.white))
                        TwoLines(title: client.value.name.string, detail: "Connected \(client.value.created_at.date.formatted(date: .abbreviated, time: .omitted))")
                        Spacer()
                        Button(role: .destructive) { removing = client } label: { Image(systemName: "trash") }.buttonStyle(.borderless).help("Disconnect")
                    }
                    ForEach(apps) { k in
                        Picker(appName(k), selection: Binding(get: { level(client.key, k) }, set: { v in
                            shell.act {
                                try await Core.call("set_grant", ["client": "ai:\(client.key)", "app": k.owner_app.string, "access": v == "none" ? nil : v])
                                await load()
                            }
                        })) {
                            Text("No access").tag("none")
                            Text("Read").tag("read")
                            Text("Read & write").tag("write")
                        }
                        .pickerStyle(.segmented)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle(clients.first?.value.name.string ?? "")
        .task(id: shell.tick) {
            await load()
            if clients.isEmpty { dismiss() }
        }
        .sheet(isPresented: $connecting, onDismiss: { Task { await load() } }) { ConnectAiSheet(url: url) }
        .alert("Disconnect “\(removing?.value.name.string ?? "")”?", isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } })) {
            Button("Disconnect", role: .destructive) {
                guard let c = removing else { return }
                shell.act {
                    try await Core.call("delete_client", ["id": c.key])
                    await load()
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Its key stops working immediately on all your devices.")
        }
    }
}

/// Name a new AI client, then show how to add UUOB to it. The key is shown only once.
struct ConnectAiSheet: View {
    @Environment(Shell.self) private var shell
    @Environment(\.dismiss) private var dismiss
    let url: String
    @State private var name = ""
    @State private var created: (name: String, token: String)?
    @State private var error: String?

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
            SheetFrame(title: "“\(created.name)” is ready", lead: "Add UUOB to your AI with one of these. The key is shown only once.") {
                Snippet(label: "Claude Code", text: command) { copy(command); shell.show("Command copied") }
                Snippet(label: "Other MCP apps (JSON)", text: json) { copy(json); shell.show("Config copied") }
                Text("Next, choose what it can access on this screen.").font(.callout).foregroundStyle(.secondary)
            } buttons: {
                Button("Done") { dismiss() }.keyboardShortcut(.defaultAction)
            }
            .frame(width: 620)
        } else {
            SheetFrame(title: "Connect an AI", lead: "Give it a name you’ll recognize. It starts with no access.") {
                TextField("e.g. Claude on my laptop", text: $name).textFieldStyle(.roundedBorder).onSubmit(create)
                if let error { Text(error).foregroundStyle(.red) }
            } buttons: {
                Button("Cancel", role: .cancel) { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Create", action: create).keyboardShortcut(.defaultAction).disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
    }

    private func create() {
        Task {
            do {
                let r = try await Core.call("create_client", ["name": name])
                created = (name: name, token: r.token.string)
            } catch {
                self.error = error.localizedDescription
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
                Text(label).font(.headline)
                Spacer()
                Button(action: copy) { Label("Copy", systemImage: "doc.on.doc") }.controlSize(.small)
            }
            Text(text).font(.system(.callout, design: .monospaced)).textSelection(.enabled)
                .padding(12).frame(maxWidth: .infinity, alignment: .leading)
                .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 8))
        }
    }
}
