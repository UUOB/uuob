import SwiftUI

/// One app as Apps shows it: its first container plus one per list, calendar, …
private struct AppGroup: Identifiable {
    let id: String
    let name: String
    let installed: Bool
    let cs: [J]
    /// Copies of the app on devices without UUOB, each linked with a code.
    let copies: [(key: String, value: J)]

    var records: Int { cs.reduce(0) { $0 + $1.records.int } }
    var bytes: Int { cs.reduce(0) { $0 + $1.size_bytes.int + $1.file_bytes.int } }
    var files: Int { cs.reduce(0) { $0 + $1.files.int } }
    var filesHere: Int { cs.reduce(0) { $0 + $1.files_here.int } }
    /// Connected storage devices that have all of it (of what is on this device).
    var safe: Int? { cs.filter { $0.here.bool && $0.records.int + $0.files.int > 0 }.map(\.copies.int).min() }
    var access: [J] { cs.first?.access.array ?? [] }
    var parts: [J] { cs.filter { $0.id != id } }
}

/// One app (Access → an app): its data, who can access it, its copies on
/// devices without UUOB; disconnecting it or deleting its data.
struct AppsView: View {
    let only: String
    @Environment(Shell.self) private var shell
    @Environment(\.dismiss) private var dismiss
    @State private var groups: [AppGroup] = []
    @State private var keepers = 0
    @State private var needed = 1
    @State private var coding = false
    @State private var disconnecting: AppGroup?
    @State private var wiping: AppGroup?

    private func load() async {
        guard let containers = try? await Core.call("containers"), let apps = try? await Core.call("apps"),
              let clients = try? await Core.call("clients") else { return }
        keepers = ((try? await Core.call("devices"))?.array ?? []).filter(\.keeper.bool).count
        needed = max(1, (try? await Core.call("status"))?.network.copies.int ?? 1)
        var seen: [String] = []
        for c in containers.array where c.owner_app.string == only && !seen.contains(c.owner_app.string) { seen.append(c.owner_app.string) }
        groups = seen.map { id in
            AppGroup(id: id, name: apps[id].name.str ?? id, installed: apps[id].installed.bool,
                     cs: containers.array.filter { $0.owner_app.string == id },
                     copies: clients.entries.filter { $0.value.app.string == id && !$0.value.endpoint.isNull }.sorted { $0.value.created_at.double < $1.value.created_at.double })
        }
    }

    private func storageNote(_ g: AppGroup) -> String {
        var parts: [String] = []
        if g.files > 0 { parts.append(g.filesHere == g.files ? plural(g.files, "file", "files") : "\(g.filesHere) of \(g.files) files on this device") }
        if keepers > 0, let safe = g.safe { parts.append(safe >= needed ? "On \(plural(safe, "storage device", "storage devices"))" : "Waiting for a storage device") }
        return parts.joined(separator: " · ")
    }

    private func sharedNote(_ c: J) -> String? {
        if !c.from.isNull { return "From \(c.from.name.string)\(c.can_write.bool ? "" : " · view only")" }
        let with = c.shared_with.array
        return with.isEmpty ? nil : "Shared with \(with.map(\.name.string).joined(separator: ", "))"
    }

    var body: some View {
        Page(title: groups.first?.name ?? "", back: { dismiss() }) {
            ForEach(groups) { g in
                Card {
                    HStack(spacing: 12) {
                        AppTile(app: g.id, size: 40)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(g.name).font(.system(size: 16, weight: .semibold))
                            HStack(spacing: 0) {
                                Text("\(plural(g.records, "item", "items")) · \(size(g.bytes)) · ").foregroundStyle(Look.text2)
                                Text(g.installed ? "Connected" : "Not connected, data kept").foregroundStyle(g.installed ? Look.ok : Look.warn)
                            }
                            .font(.system(size: 13))
                            let note = storageNote(g)
                            if !note.isEmpty { Text(note).font(.system(size: 13)).foregroundStyle(Look.text2) }
                        }
                        Spacer()
                        Button { exportJSON(g) } label: { Ico("copy", size: 18) }.buttonStyle(IconButtonStyle()).uuobHint("Copy as JSON")
                    }
                    if !g.parts.isEmpty {
                        DisclosureGroup("In \(g.name)") {
                            ForEach(g.parts) { part in
                                HStack {
                                    Text(part.name.string)
                                    Text(part.here.bool ? plural(part.records.int, "item", "items") : "Not on this device").foregroundStyle(.secondary)
                                    Spacer()
                                    if let note = sharedNote(part) { HStack(spacing: 4) { Ico("people", size: 14); Text(note) }.foregroundStyle(Look.text2) }
                                }
                            }
                        }
                    }
                    Trailing("Who can access it") {
                        VStack(alignment: .trailing, spacing: 4) {
                            if g.access.isEmpty { Text("Nobody — only you, from this screen.").foregroundStyle(.secondary) }
                            ForEach(g.access, id: \.client.string) { a in
                                let owner = a.client.string == "app:\(g.id)"
                                HStack(spacing: 6) {
                                    Text(a.client_name.string)
                                    Text((a.access.string == "write" ? "Read & write" : "Read only") + (owner ? " · owner" : "")).foregroundStyle(.secondary)
                                    if !owner {
                                        Button { revoke(g, a) } label: { Ico("close", size: 16) }
                                            .buttonStyle(IconButtonStyle()).uuobHint("Remove access")
                                    }
                                }
                            }
                        }
                    }
                    if !g.copies.isEmpty {
                        Trailing("On devices without UUOB") {
                            VStack(alignment: .trailing, spacing: 4) {
                                ForEach(g.copies, id: \.key) { copy in
                                    let device = copy.value.device.str ?? "Another device"
                                    HStack(spacing: 6) {
                                        Text(device)
                                        Text("Connected \(copy.value.created_at.date.formatted(date: .abbreviated, time: .omitted))").foregroundStyle(.secondary)
                                        Button { unlink(g, copy.key, device) } label: { Ico("close", size: 16) }
                                            .buttonStyle(IconButtonStyle()).uuobHint("Disconnect \(g.name) on \(device)")
                                    }
                                }
                            }
                        }
                    }
                    HStack {
                        Spacer()
                        if g.installed {
                            Button("Disconnect \(g.name)…", role: .destructive) { disconnecting = g }.buttonStyle(UuobButton(.danger))
                        } else {
                            Button("Delete Data…", role: .destructive) { wiping = g }.buttonStyle(UuobButton(.danger))
                        }
                    }
                }
            }
        }
        .task(id: shell.tick) {
            await load()
            // Disconnected with its data deleted: nothing left to show.
            if groups.isEmpty { dismiss() }
        }
        .sheet(isPresented: $coding, onDismiss: { Task { await load() } }) { AppCodeSheet() }
        .sheet(item: $disconnecting) { g in
            SheetFrame(title: "Disconnect \(g.name)?",
                       lead: "\(g.name) stops working with your network on all your devices. Keep its \(plural(g.records, "item", "items")) to connect it again later, or erase them from every device.") {
                EmptyView()
            } buttons: {
                Button("Cancel") { disconnecting = nil }.keyboardShortcut(.cancelAction)
                Button("Keep the Data") { disconnecting = nil; uninstall(g, delete: false) }
                Button("Delete the Data Too", role: .destructive) { disconnecting = nil; uninstall(g, delete: true) }.buttonStyle(UuobButton(.danger, large: true))
            }
        }
        .confirm("Delete all \(wiping?.name ?? "") data?", isPresented: Binding(get: { wiping != nil }, set: { if !$0 { wiping = nil } }),
                 message: "\(plural(wiping?.records ?? 0, "item is", "items are")) permanently erased from every device. This can’t be undone.", action: "Delete") {
            guard let g = wiping else { return }
            shell.act {
                try await Core.call("uninstall_app", ["id": g.id, "delete_data": true])
                shell.show("\(g.name) data deleted on all devices")
                await load()
            }
        }
    }

    private func exportJSON(_ g: AppGroup) {
        shell.act {
            var data: [String: Any] = [:]
            for c in g.cs { data[c.id == g.id ? g.name : c.name.string] = try await Core.call("export", ["container": c.id]).value ?? NSNull() }
            let text = try JSONSerialization.data(withJSONObject: data, options: [.prettyPrinted, .sortedKeys])
            copy(String(decoding: text, as: UTF8.self))
            shell.show("Copied all \(g.name) data as JSON")
        }
    }

    private func revoke(_ g: AppGroup, _ a: J) {
        shell.act {
            try await Core.call("set_grant", ["client": a.client.string, "app": g.id, "access": nil])
            shell.show("\(a.client_name.string) can no longer access \(g.name)")
            await load()
        }
    }

    private func unlink(_ g: AppGroup, _ id: String, _ device: String) {
        shell.act {
            try await Core.call("delete_client", ["id": id])
            shell.show("\(g.name) on \(device) was disconnected")
            await load()
        }
    }

    private func uninstall(_ g: AppGroup, delete: Bool) {
        shell.act {
            try await Core.call("uninstall_app", ["id": g.id, "delete_data": delete])
            shell.show(delete ? "\(g.name) was disconnected and its data deleted" : "\(g.name) was disconnected; its data is kept")
            await load()
        }
    }
}

/// A one-time code to type into an app, also on a device without UUOB.
struct AppCodeSheet: View {
    @Environment(\.dismiss) private var dismiss
    @State private var code: J?

    var body: some View {
        CodeCard(title: "Connect with a code", lead: "Type it into the app. It also works on a device without UUOB.", onClose: { dismiss() }) {
            if let code { CodeView(invite: code, waiting: "Works once.", qr: false) } else { ProgressView() }
        }
        .task { code = try? await Core.call("create_app_code") }
    }
}
