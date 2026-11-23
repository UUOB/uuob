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
                Ico("right", size: 22).foregroundStyle(Look.text3)
                if connect {
                    Image(nsImage: NSApp.applicationIconImage).resizable().frame(width: 64, height: 64)
                } else {
                    AppTile(app: ask.container.string, size: 58)
                }
            }
            Group {
                if connect {
                    Text("“\(ask.name.string)” wants to use your network").font(.system(size: 22, weight: .semibold))
                    Text("It gets its own place for its data, synced to all your devices. It can’t see anything else unless you allow it.").font(.system(size: 17)).foregroundStyle(Look.text2)
                } else {
                    Text("“\(ask.app_name.string)” wants to \(ask.access.string == "write" ? "read and change" : "read") your \(ask.container_name.string) data").font(.system(size: 22, weight: .semibold))
                    Text("You can change this any time in Access.").font(.system(size: 17)).foregroundStyle(Look.text2)
                }
            }
            .multilineTextAlignment(.center)
            HStack(spacing: 10) {
                Button("Don’t Allow") { answer(false) }.keyboardShortcut(.cancelAction)
                Button("Allow") { answer(true) }.buttonStyle(UuobButton(.primary, large: true)).keyboardShortcut(.defaultAction)
            }
            .environment(\.fillButtons, true)
            .padding(.top, 8)
        }
        .padding(28)
        .frame(width: 460)
        .background(Look.elevated)
        .foregroundStyle(Look.text)
        .buttonStyle(UuobButton(large: true))
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
                Ico("right", size: 22).foregroundStyle(Look.text3)
                AppTile(app: offer.app.string, size: 58)
            }
            Text("\(person.name.string) shared “\(offer.name.string)” with you").font(.system(size: 22, weight: .semibold)).multilineTextAlignment(.center)
            Text("It will appear in \(offer.app_name.string) on all your devices. \(offer.access.string == "write" ? "You can both change it." : "You can view it, but not change it.")")
                .font(.system(size: 17)).foregroundStyle(Look.text2).multilineTextAlignment(.center)
            HStack(spacing: 10) {
                Button("Decline") { answer(false) }
                Button("Accept") { answer(true) }.buttonStyle(UuobButton(.primary, large: true)).keyboardShortcut(.defaultAction)
            }
            .environment(\.fillButtons, true)
            .padding(.top, 8)
            Button("Later") { shell.later.insert(offer.container.string) }.buttonStyle(UuobButton(.ghost)).keyboardShortcut(.cancelAction)
        }
        .padding(28)
        .frame(width: 460)
        .background(Look.elevated)
        .foregroundStyle(Look.text)
        .buttonStyle(UuobButton(large: true))
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
                CodeCard(title: "Share with someone new", lead: "They choose Access → Add → A person: enter their code, and type this.", onClose: back) {
                    VStack(spacing: 18) {
                        Segmented([("read", "Can view"), ("write", "Can edit")], selected: access) { access = $0; newInvite() }
                        if let invite { CodeView(invite: invite, waiting: "Waiting for them…") }
                        else if let error { Text(error).foregroundStyle(Look.danger) }
                        else { ProgressView() }
                    }
                }
            case .people:
                DetailSheet(title: "Share “\(ask.container_name.string)”", onClose: done, actions: { EmptyView() }) {
                    SheetLead("From \(ask.app_name.string). People you choose get this and nothing else.")
                    VStack(spacing: 0) {
                        ForEach(shell.people) { p in
                            HStack(spacing: 10) {
                                Avatar(name: p.name.string, size: 30)
                                Text(p.name.string)
                                Spacer()
                                Segmented([("none", "No access"), ("read", "Can view"), ("write", "Can edit")], selected: level(p)) { v in set(p, v) }
                            }
                            .padding(.vertical, 8)
                            Rectangle().fill(Look.line).frame(height: 1)
                        }
                        Button(action: someoneNew) {
                            HStack {
                                Ico("plus", size: 20)
                                Text("Someone new…")
                                Spacer()
                                Ico("chevron", size: 18).foregroundStyle(Look.text3)
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain).foregroundStyle(Look.accent).padding(.vertical, 10)
                    }
                    .padding(.top, 12)
                } footer: {
                    HStack {
                        Spacer()
                        Button("Done") { done() }.buttonStyle(UuobButton(.primary, small: true)).keyboardShortcut(.defaultAction)
                    }
                    .padding(.top, 18)
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
