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
        Form {
            Section {
                GraphView()
            } header: {
                Lead("Your devices in the middle, each app on the device it runs on, people at the edge.")
            }
            Section("Devices") {
                ForEach(devices) { d in
                    HStack(spacing: 12) {
                        Image(systemName: d.platform.string == "android" || d.platform.string == "ios" ? "iphone" : "laptopcomputer")
                            .font(.title2).foregroundStyle(.secondary).frame(width: 36)
                        VStack(alignment: .leading, spacing: 2) {
                            HStack(spacing: 6) {
                                Text(d.name.string).font(.headline)
                                if d.is_self.bool {
                                    Text("This device").font(.caption.weight(.semibold)).foregroundStyle(Color.accentColor)
                                        .padding(.horizontal, 7).padding(.vertical, 2)
                                        .background(Color.accentColor.opacity(0.14), in: Capsule())
                                }
                            }
                            HStack(spacing: 6) {
                                Dot(on: d.online.bool)
                                Text("\(d.online.bool ? "Online" : "Offline") · \(platformName(d.platform.string))\(d.keeper.bool ? " · keeps everything" : "")").foregroundStyle(.secondary)
                            }
                        }
                        Spacer()
                        if d.is_self.bool {
                            Button("Rename…") { name = d.name.string; renaming = true }
                        } else {
                            Button(role: .destructive) { removing = d } label: { Image(systemName: "trash") }
                                .buttonStyle(.borderless).help("Remove from network")
                        }
                    }
                    .padding(.vertical, 4)
                }
                HStack {
                    Button("Add a Device…") { adding = true }.buttonStyle(.borderedProminent)
                    if devices.count <= 1 { Button("Join a Network…") { joining = true } }
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Network")
        .toolbar {
            ToolbarItem {
                Button { adding = true } label: { Label("Add a Device", systemImage: "plus") }.help("Add a device")
            }
        }
        .task(id: shell.tick) { await load() }
        .sheet(isPresented: $adding, onDismiss: { Task { await load() } }) { AddDeviceSheet() }
        .sheet(isPresented: $joining, onDismiss: { Task { await load() } }) { JoinSheet() }
        .alert("This device’s name", isPresented: $renaming) {
            TextField("Name", text: $name)
            Button("Save") {
                shell.act {
                    try await Core.call("set_device_name", ["name": name])
                    await load()
                    shell.tick += 1
                }
            }
            .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
            Button("Cancel", role: .cancel) {}
        }
        .alert("Remove “\(removing?.name.string ?? "")”?", isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } })) {
            Button("Remove", role: .destructive) {
                guard let d = removing else { return }
                shell.act {
                    try await Core.call("remove_device", ["id": d.id])
                    shell.show("\(d.name.string) was removed")
                    await load()
                    shell.tick += 1
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("It stops receiving updates right away. Data it already has stays on it. To use it again, add it back with a new code.")
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
        Form {
            Section("You") {
                HStack(spacing: 12) {
                    Avatar(name: me.isEmpty ? "?" : me, mine: true)
                    TwoLines(title: me.isEmpty ? "No name yet" : me, detail: "The name people you link with see")
                    Spacer()
                    Button("Change…") { naming = true }
                }
            }

            Section("This Mac") {
                Toggle(isOn: Binding(get: { atLogin }, set: { on in
                    shell.act {
                        try await Core.call("set_autostart", ["on": on])
                        await load()
                    }
                })) {
                    TwoLines(title: "Start at login", detail: "UUOB starts in the menu bar when you log in, so your apps and devices stay in sync.")
                }
                LabeledContent {
                    Text(shell.serverError == nil ? "Running" : "Stopped").foregroundStyle(shell.serverError == nil ? .green : .red)
                } label: {
                    TwoLines(title: "Apps can connect", detail: shell.serverError ?? "Only from this Mac, and only the ones you allow in Access. AI server: \(url)")
                }
            }

            if !alone {
                Section {
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
                        Picker(selection: Binding(get: { copies }, set: { n in
                            shell.act {
                                try await Core.call("set_copies", ["copies": n])
                                await load()
                            }
                        })) {
                            Text("1").tag(1)
                            Text("2").tag(2)
                        } label: {
                            TwoLines(title: "Copies", detail: "Storage devices that must have something before another device frees its own copy.")
                        }
                        .pickerStyle(.segmented)
                    }
                } header: {
                    Text("Storage devices")
                } footer: {
                    Text(keepers > 0
                        ? "Apps with large data, like photos, can keep less on your other devices. Nothing is removed from a device until \(copies == 1 ? "a storage device has" : "\(copies) storage devices have") it."
                        : "Turn this on for a device with plenty of space that is usually on. Apps with large data, like photos, can then keep less on your other devices.")
                    .foregroundStyle(.secondary)
                }
            }

            Section("Updates") {
                LabeledContent {
                    if let v = shell.update {
                        Button(shell.updating ? "Updating…" : "Update to \(v)") { shell.installUpdate() }
                            .buttonStyle(.borderedProminent).disabled(shell.updating)
                    }
                } label: {
                    TwoLines(title: "UUOB \(version)", detail: shell.updateError ?? (shell.update.map { "Version \($0) is ready" } ?? "Up to date"))
                }
            }

            if !alone {
                Section("Network") {
                    LabeledContent {
                        Button("Leave…", role: .destructive) { leaving = true }
                    } label: {
                        TwoLines(title: "Leave this network", detail: "This Mac stops syncing with your other devices.")
                    }
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Settings")
        .task(id: shell.tick) { await load() }
        .sheet(isPresented: $naming) {
            NameSheet(current: me, continues: false) { name in
                try await Core.call("set_name", ["name": name])
                me = name.trimmingCharacters(in: .whitespaces)
                naming = false
            }
        }
        .alert("Leave this network?", isPresented: $leaving) {
            Button("Leave", role: .destructive) {
                shell.act {
                    try await Core.call("leave_network")
                    shell.show("This device now has its own network")
                    await load()
                    shell.tick += 1
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This device stops syncing with your other devices. Everything already on it stays here.")
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
                SheetFrame(title: "Add a device", lead: "On the other device, open UUOB, choose Join a network and enter this code.", center: true) {
                    CodeView(invite: invite, waiting: "Waiting for your device…").padding(.vertical, 8)
                } buttons: {
                    Button("Cancel", role: .cancel) { close() }.keyboardShortcut(.cancelAction)
                }
            } else if let error {
                SheetFrame(title: "Couldn’t create a code", lead: error) {
                    EmptyView()
                } buttons: {
                    Button("Close") { dismiss() }.keyboardShortcut(.cancelAction)
                }
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
            Image(systemName: "checkmark.circle.fill").font(.system(size: 64)).foregroundStyle(.green).symbolEffect(.bounce, value: title)
            Text(title).font(.title2.weight(.semibold))
            Text(text).foregroundStyle(.secondary).multilineTextAlignment(.center)
            Button("Done", action: done).buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction).padding(.top, 8)
        }
        .padding(32)
        .frame(width: 420)
    }
}

/// Type the code another device shows.
struct JoinSheet: View {
    @Environment(Shell.self) private var shell
    @Environment(\.dismiss) private var dismiss
    @State private var code = ""
    @State private var busy = false
    @State private var error: String?

    var body: some View {
        SheetFrame(title: "Join a network", lead: "On a device that’s already set up, open Network → Add a device and type the code it shows.") {
            TextField("e.g. 42 amber river falcon", text: $code)
                .textFieldStyle(.roundedBorder).font(.title3).autocorrectionDisabled()
                .onSubmit(join)
            if let error { Text(error).foregroundStyle(.red) }
            Text("Anything already on this device is kept and merged into the network.").font(.callout).foregroundStyle(.secondary)
        } buttons: {
            Button("Cancel", role: .cancel) { dismiss() }.keyboardShortcut(.cancelAction)
            Button(busy ? "Linking…" : "Join", action: join).keyboardShortcut(.defaultAction).disabled(busy || !complete(code))
        }
    }

    private func join() {
        guard complete(code), !busy else { return }
        busy = true
        error = nil
        Task {
            do {
                try await Core.call("join", ["code": code])
                shell.show("Linked! Your data will sync in a moment.")
                dismiss()
            } catch {
                self.error = error.localizedDescription
            }
            busy = false
        }
    }
}
