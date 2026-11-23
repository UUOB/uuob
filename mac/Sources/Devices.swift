import SwiftUI

private func platformName(_ p: String) -> String {
    ["linux": "Linux", "macos": "macOS", "windows": "Windows", "android": "Android", "ios": "iOS"][p] ?? p
}

/// Network: the graph of who is where, then your devices.
struct NetworkView: View {
    @Environment(Shell.self) private var shell
    @State private var devices: [J] = []
    @State private var adding = false
    @State private var joining = false
    @State private var removing: J?
    @State private var renaming = false
    @State private var name = ""

    private func load() async {
        if let d = try? await Core.call("devices") { devices = d.array }
    }

    var body: some View {
        Page(title: "Network") {
            GraphView().padding(.top, 14)
            Card("Devices") {
                ForEach(devices) { d in
                    HStack(spacing: 14) {
                        Ico(d.platform.string == "android" || d.platform.string == "ios" ? "phone" : "laptop", size: 24)
                            .foregroundStyle(Look.text2)
                            .frame(width: 44, height: 44)
                            .background(Look.surface2, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                        VStack(alignment: .leading, spacing: 2) {
                            HStack(spacing: 6) {
                                Text(d.name.string).font(.system(size: 16, weight: .semibold))
                                if d.is_self.bool { Tag("This device") }
                            }
                            HStack(spacing: 6) {
                                Dot(on: d.online.bool)
                                Text("\(d.online.bool ? "Online" : "Offline") · \(platformName(d.platform.string))\(d.keeper.bool ? " · keeps everything" : "")")
                                    .font(.system(size: 13)).foregroundStyle(Look.text2)
                            }
                        }
                        Spacer()
                        if d.is_self.bool {
                            Button { name = d.name.string; renaming = true } label: { Ico("edit", size: 18) }
                                .buttonStyle(IconButtonStyle()).tip("Rename")
                        } else {
                            Button(role: .destructive) { removing = d } label: { Ico("trash", size: 18) }
                                .buttonStyle(IconButtonStyle()).tip("Remove from network")
                        }
                    }
                }
            }
            HStack(spacing: 12) {
                Button { adding = true } label: { HStack(spacing: 8) { Ico("plus", size: 20, stroke: 2.2); Text("Add a device") } }
                    .buttonStyle(UuobButton(.primary, large: true))
                if devices.count <= 1 {
                    Button { joining = true } label: { HStack(spacing: 8) { Ico("link", size: 20); Text("Join a network") } }
                        .buttonStyle(UuobButton(large: true))
                }
            }
            .padding(.top, 24)
        }
        .task(id: shell.tick) { await load() }
        .sheet(isPresented: $adding, onDismiss: { Task { await load() } }) { AddDeviceSheet() }
        .sheet(isPresented: $joining, onDismiss: { Task { await load() } }) { JoinSheet() }
        .sheet(isPresented: $renaming) {
            NameEditor(placeholder: "Device name", under: "This device, as your other devices see it", current: name) { new in
                try await Core.call("set_device_name", ["name": new])
                renaming = false
                await load()
                shell.tick += 1
            }
        }
        .confirm("Remove “\(removing?.name.string ?? "")”?", isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
                 message: "It stops receiving updates right away. Data it already has stays on it. To use it again, add it back with a new code.",
                 action: "Remove") {
            guard let d = removing else { return }
            shell.act {
                try await Core.call("remove_device", ["id": d.id])
                shell.show("\(d.name.string) was removed")
                await load()
                shell.tick += 1
            }
        }
    }
}

/// Settings: your name, this Mac, storage, updates, leaving the network.
struct SettingsView: View {
    @Environment(Shell.self) private var shell
    @State private var devices: [J] = []
    @State private var copies = 1
    @State private var atLogin = false
    @State private var me = ""
    @State private var version = ""
    @State private var url = ""
    @State private var naming = false
    @State private var leaving = false

    private var alone: Bool { devices.count <= 1 }
    private var keepers: Int { devices.filter(\.keeper.bool).count }

    private func load() async {
        if let d = try? await Core.call("devices") { devices = d.array }
        if let s = try? await Core.call("status") {
            copies = max(1, s.network.copies.int)
            me = s.network.name.string
        }
        atLogin = (try? await Core.call("autostart"))?.bool ?? false
        version = (try? await Core.call("version"))?.string ?? ""
        url = (try? await Core.call("mcp_url"))?.string ?? ""
    }

    var body: some View {
        Page(title: "Settings") {
            Card("You") {
                HStack(spacing: 12) {
                    Avatar(name: me.isEmpty ? "?" : me, mine: true)
                    TwoLines(title: me.isEmpty ? "No name yet" : me, detail: "The name people you link with see")
                    Spacer()
                    Button { naming = true } label: { Ico("edit", size: 18) }.buttonStyle(IconButtonStyle()).tip("Change name")
                }
            }

            Card("This Mac") {
                Toggle(isOn: Binding(get: { atLogin }, set: { on in
                    shell.act {
                        try await Core.call("set_autostart", ["on": on])
                        await load()
                    }
                })) {
                    Text("Start at login").font(.system(size: 16, weight: .semibold))
                }
                Trailing {
                    Text(shell.serverError == nil ? "Running" : "Stopped").font(.system(size: 15, weight: .semibold)).foregroundStyle(shell.serverError == nil ? Look.ok : Look.danger)
                } label: {
                    TwoLines(title: "Apps can connect", detail: shell.serverError ?? "AI server: \(url)")
                }
            }

            if !alone {
                Card {
                    ForEach(devices) { d in
                        Toggle(isOn: Binding(get: { d.keeper.bool }, set: { on in
                            shell.act {
                                try await Core.call("set_keeper", ["device": d.id, "on": on])
                                await load()
                                shell.tick += 1
                            }
                        })) {
                            TwoLines(title: d.name.string, detail: d.keeper.bool ? "Keeps everything" : "Keeps what its apps ask for")
                        }
                    }
                    if keepers > 0 {
                        Trailing {
                            Segmented([("1", "1"), ("2", "2")], selected: String(copies)) { n in
                                shell.act {
                                    try await Core.call("set_copies", ["copies": Int(n) ?? 1])
                                    await load()
                                }
                            }
                        } label: {
                            TwoLines(title: "Copies", detail: "How many storage devices keep each thing")
                        }
                    }
                } header: {
                    SectionLabel("Storage devices")
                }
            }

            Card("Updates") {
                Trailing {
                    if let v = shell.update {
                        Button(shell.updating ? "Updating…" : "Update to \(v)") { shell.installUpdate() }
                            .buttonStyle(UuobButton(.primary)).disabled(shell.updating)
                    }
                } label: {
                    TwoLines(title: "UUOB \(version)", detail: shell.updateError ?? (shell.update.map { "Version \($0) is ready" } ?? "Up to date"))
                }
            }

            if !alone {
                Card("Network") {
                    Trailing {
                        Button("Leave…", role: .destructive) { leaving = true }.buttonStyle(UuobButton(.danger))
                    } label: {
                        TwoLines(title: "Leave this network", detail: "This Mac stops syncing with your other devices.")
                    }
                }
            }
        }
        .task(id: shell.tick) { await load() }
        .sheet(isPresented: $naming) {
            NameSheet(current: me, continues: false) { name in
                try await Core.call("set_name", ["name": name])
                me = name.trimmingCharacters(in: .whitespaces)
                naming = false
            }
        }
        .confirm("Leave this network?", isPresented: $leaving,
                 message: "This device stops syncing with your other devices. Everything already on it stays here.", action: "Leave") {
            shell.act {
                try await Core.call("leave_network")
                shell.show("This device now has its own network")
                await load()
                shell.tick += 1
            }
        }
    }
}

/// A code (and QR code) for a new device, until it joins.
struct AddDeviceSheet: View {
    @Environment(Shell.self) private var shell
    @Environment(\.dismiss) private var dismiss
    @State private var invite: J?
    @State private var joined: String?
    @State private var error: String?

    var body: some View {
        Group {
            if let joined {
                Success(title: "\(joined) joined", text: "Your apps and data are syncing to it now.") { dismiss() }
            } else if let invite {
                CodeCard(title: "Add a device", lead: "On the other device, open UUOB, choose Join a network and enter this code.", onClose: close) {
                    CodeView(invite: invite, waiting: "Waiting for your device…")
                }
            } else if let error {
                DetailSheet(title: "Couldn’t create a code", onClose: { dismiss() }, actions: { EmptyView() }) { SheetLead(error) }
            } else {
                HStack { ProgressView().controlSize(.small); Text("Creating a code…") }.padding(40)
            }
        }
        .task {
            do { invite = try await Core.call("create_invite") } catch { self.error = error.localizedDescription }
        }
        .onChange(of: shell.events) {
            if shell.lastEvent.type.string == "device_joined" { joined = shell.lastEvent.name.string }
        }
    }

    private func close() {
        if let code = invite?.code.str, joined == nil { Task { try? await Core.call("cancel_invite", ["code": code]) } }
        dismiss()
    }
}

/// A green tick and what just worked.
struct Success: View {
    let title: String
    let text: String
    let done: () -> Void

    var body: some View {
        VStack(spacing: 12) {
            Ico("check", size: 40, stroke: 2.5).foregroundStyle(.white).frame(width: 76, height: 76).background(Look.ok, in: Circle())
            Text(title).font(.system(size: 22, weight: .semibold))
            Text(text).font(.system(size: 17)).foregroundStyle(Look.text2).multilineTextAlignment(.center)
            Button("Done", action: done).buttonStyle(UuobButton(.primary, large: true)).environment(\.fillButtons, true).keyboardShortcut(.defaultAction).padding(.top, 8)
        }
        .padding(28)
        .frame(width: 460)
        .background(Look.elevated)
    }
}

/// Type the code another device shows.
struct JoinSheet: View {
    @Environment(Shell.self) private var shell

    var body: some View {
        CodeEntry(title: "Join a network", lead: "On a device that’s already set up, open Network → Add a device.",
                  note: "What’s on this device is kept and merged in.", action: "Join", busyAction: "Linking…") { code in
            try await Core.call("join", ["code": code])
            shell.show("Linked! Your data will sync in a moment.")
        }
    }
}
