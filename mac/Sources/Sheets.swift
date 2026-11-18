// What floats over the window: an app's question, the share sheet, an offer from someone.

import SwiftUI

/// "“Tasks” wants to use your network": an app asks to connect, or for another app's data.
struct AskSheet: View {
    @Environment(Shell.self) private var shell
    let ask: J

    private var connect: Bool { ask.kind.string == "connect" }

    var body: some View {
        VStack(spacing: 14) {
            HStack(spacing: 14) {
                AppTile(app: ask.app.string, size: 58)
                Image(systemName: "arrow.right").foregroundStyle(.secondary)
                if connect {
                    Image(nsImage: NSApp.applicationIconImage).resizable().frame(width: 64, height: 64)
                } else {
                    AppTile(app: ask.container.string, size: 58)
                }
            }
            Group {
                if connect {
                    Text("“\(ask.name.string)” wants to use your network").font(.title2.weight(.semibold))
                    Text("It gets its own place for its data, synced to all your devices. It can’t see anything else unless you allow it.").foregroundStyle(.secondary)
                } else {
                    Text("“\(ask.app_name.string)” wants to \(ask.access.string == "write" ? "read and change" : "read") your \(ask.container_name.string) data").font(.title2.weight(.semibold))
                    Text("You can change this any time in Apps.").foregroundStyle(.secondary)
                }
            }
            .multilineTextAlignment(.center)
            HStack(spacing: 10) {
                Button { answer(false) } label: { Text("Don’t Allow").frame(maxWidth: .infinity) }.keyboardShortcut(.cancelAction)
                Button { answer(true) } label: { Text("Allow").frame(maxWidth: .infinity) }.keyboardShortcut(.defaultAction)
            }
            .controlSize(.large)
            .padding(.top, 8)
        }
        .padding(28)
        .frame(width: 440)
        .interactiveDismissDisabled()
    }

    private func answer(_ allow: Bool) {
        Task {
            try? await Core.call("answer", ["id": ask.id, "allow": allow])
            if allow && connect { shell.show("\(ask.name.string) is connected") }
            await shell.loadAsks()
            shell.tick += 1
        }
    }
}

/// Someone shared a list or a calendar with you.
struct OfferSheet: View {
    @Environment(Shell.self) private var shell
    let offer: J
    let person: J

    var body: some View {
        VStack(spacing: 14) {
            HStack(spacing: 14) {
                Avatar(name: person.name.string, size: 58)
                Image(systemName: "arrow.right").foregroundStyle(.secondary)
                AppTile(app: offer.app.string, size: 58)
            }
            Text("\(person.name.string) shared “\(offer.name.string)” with you").font(.title2.weight(.semibold)).multilineTextAlignment(.center)
            Text("It will appear in \(offer.app_name.string) on all your devices. \(offer.access.string == "write" ? "You can both change it." : "You can view it, but not change it.")")
                .foregroundStyle(.secondary).multilineTextAlignment(.center)
            HStack(spacing: 10) {
                Button { answer(false) } label: { Text("Decline").frame(maxWidth: .infinity) }
                Button { answer(true) } label: { Text("Accept").frame(maxWidth: .infinity) }.keyboardShortcut(.defaultAction)
            }
            .controlSize(.large)
            .padding(.top, 8)
            Button("Later") { shell.later.insert(offer.container.string) }.buttonStyle(.link).keyboardShortcut(.cancelAction)
        }
        .padding(28)
        .frame(width: 440)
    }

    private func answer(_ accept: Bool) {
        let container = offer.container.string
        shell.later.insert(container)
        shell.act {
            if accept {
                try await Core.call("accept_share", ["container": container])
                shell.show("“\(offer.name.string)” was added to \(offer.app_name.string)")
            } else {
                try await Core.call("delete_container", ["id": container])
            }
            await shell.refresh()
            shell.tick += 1
        }
    }
}

/// The share sheet: an app asked to share one of its lists or calendars, and
/// the user chooses here who gets it. Changes apply as they are made.
struct ShareSheet: View {
    @Environment(Shell.self) private var shell
    let ask: J

    private enum Step { case people, name, new }
    @State private var step = Step.people
    @State private var me = ""
    @State private var access = "write" // for someone new
    @State private var invite: J?
    @State private var error: String?
    @State private var known: Set<String> = []

    private var container: String { ask.container.string }

    var body: some View {
        Group {
            switch step {
            case .name:
                NameSheet(current: "", continues: true, save: { name in
                    try await Core.call("set_name", ["name": name])
                    me = name.trimmingCharacters(in: .whitespaces)
                    someoneNew()
                }, cancel: back)
            case .new:
                SheetFrame(title: "Share with someone new", lead: "They open UUOB on their device, go to People → Enter a code and type this.", center: true) {
                    Picker("", selection: Binding(get: { access }, set: { access = $0; newInvite() })) {
                        Text("Can view").tag("read")
                        Text("Can edit").tag("write")
                    }
                    .pickerStyle(.segmented).labelsHidden().fixedSize()
                    if let invite { CodeView(invite: invite, waiting: "Waiting for them…").padding(.vertical, 8) }
                    else if let error { Text(error).foregroundStyle(.red) }
                    else { ProgressView() }
                } buttons: {
                    Button("Back") { back() }.keyboardShortcut(.cancelAction)
                }
            case .people:
                SheetFrame(title: "Share “\(ask.container_name.string)”", lead: "From \(ask.app_name.string). People you choose get this and nothing else.") {
                    VStack(spacing: 0) {
                        ForEach(shell.people) { p in
                            HStack(spacing: 10) {
                                Avatar(name: p.name.string, size: 30)
                                Text(p.name.string)
                                Spacer()
                                Picker("", selection: Binding(get: { level(p) }, set: { v in set(p, v) })) {
                                    Text("No access").tag("none")
                                    Text("Can view").tag("read")
                                    Text("Can edit").tag("write")
                                }
                                .pickerStyle(.segmented).labelsHidden().fixedSize()
                            }
                            .padding(.vertical, 8)
                            Divider()
                        }
                        Button(action: someoneNew) {
                            HStack {
                                Image(systemName: "plus.circle.fill")
                                Text("Someone new…")
                                Spacer()
                                Image(systemName: "chevron.right").foregroundStyle(.secondary)
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain).foregroundStyle(Color.accentColor).padding(.vertical, 10)
                    }
                } buttons: {
                    Button("Done") { done() }.keyboardShortcut(.defaultAction)
                }
            }
        }
        .interactiveDismissDisabled()
        .task {
            known = Set(shell.people.map(\.id))
            me = (try? await Core.call("status"))?.network.name.string ?? ""
        }
        // While a code is showing, a new person in the list is the one who used it.
        .onChange(of: shell.people.map(\.id)) {
            if step == .new, invite != nil, let joined = shell.people.first(where: { !known.contains($0.id) }) {
                invite = nil
                step = .people
                shell.show("Linked with \(joined.name.string)")
            }
            known = Set(shell.people.map(\.id))
        }
    }

    private func level(_ p: J) -> String { p.sharing.array.first { $0.container.string == container }?.access.string ?? "none" }

    private func set(_ p: J, _ v: String) {
        shell.act {
            try await Core.call("share_with", ["container": container, "person": p.id, "access": v == "none" ? nil : v])
            await shell.refresh()
        }
    }

    private func someoneNew() {
        error = nil
        if me.isEmpty { step = .name } else { step = .new; newInvite() }
    }

    /// The code both links the person and shares this with them.
    private func newInvite() {
        cancelInvite()
        Task {
            do { invite = try await Core.call("create_person_invite", ["container": container, "access": access]) } catch { self.error = error.localizedDescription }
        }
    }

    private func cancelInvite() {
        if let code = invite?.code.str { Task { try? await Core.call("cancel_invite", ["code": code]) } }
        invite = nil
    }

    private func back() {
        cancelInvite()
        error = nil
        step = .people
    }

    private func done() {
        cancelInvite()
        Task {
            try? await Core.call("answer", ["id": ask.id, "allow": true])
            await shell.loadAsks()
            shell.tick += 1
        }
    }
}
