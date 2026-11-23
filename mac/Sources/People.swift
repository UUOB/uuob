import SwiftUI

private func can(_ access: String) -> String { access == "write" ? "Can edit" : "Can view" }

/// One person (Access → a person): what you share with each other.
struct PeopleView: View {
    let only: String
    @Environment(Shell.self) private var shell
    @Environment(\.dismiss) private var dismiss
    @State private var me = ""
    @State private var naming = false
    /// What to open once the name is set.
    @State private var afterName: PersonSheet?
    @State private var sheet: PersonSheet?
    @State private var removing: J?

    enum PersonSheet: String, Identifiable {
        case add, meet
        var id: String { rawValue }
    }

    private func load() async {
        me = (try? await Core.call("status"))?.network.name.string ?? ""
    }

    /// Linking needs a name for the other person to see.
    private func start(_ what: PersonSheet) {
        if me.trimmingCharacters(in: .whitespaces).isEmpty {
            afterName = what
            naming = true
        } else {
            sheet = what
        }
    }

    private func act(_ method: String, _ params: [String: Any?], toast: String? = nil) {
        shell.act {
            try await Core.call(method, params)
            if let toast { shell.show(toast) }
            await shell.refresh()
        }
    }

    var body: some View {
        Page(title: shell.people.first { $0.id == only }?.name.string ?? "", back: { dismiss() }) {
            ForEach(shell.people.filter { $0.id == only }) { p in
                Card {
                    HStack(spacing: 12) {
                        Avatar(name: p.name.string)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(p.name.string).font(.system(size: 16, weight: .semibold))
                            HStack(spacing: 6) { Dot(on: p.online.bool); Text(p.online.bool ? "Online" : "Offline").font(.system(size: 13)).foregroundStyle(Look.text2) }
                        }
                        Spacer()
                        Button(role: .destructive) { removing = p } label: { Ico("trash", size: 18) }
                            .buttonStyle(IconButtonStyle()).help("Remove \(p.name.string)")
                    }
                    let receiving = p.receiving.array
                    let sharing = p.sharing.array
                    ForEach(receiving, id: \.container.string) { s in
                        HStack(spacing: 10) {
                            AppTile(app: s.app.string, size: 28)
                            TwoLines(title: s.name.string, detail: "\(p.name.string) shares · \(s.app_name.string) · \(can(s.access.string))")
                            Spacer()
                            if s.offered.bool {
                                Button("Decline") { act("delete_container", ["id": s.container.string]) }
                                Button("Accept") {
                                    act("accept_share", ["container": s.container.string], toast: "“\(s.name.string)” was added to \(s.app_name.string)")
                                }
                                .buttonStyle(UuobButton(.primary))
                            } else {
                                Button("Leave", role: .destructive) {
                                    act("delete_container", ["id": s.container.string], toast: "You left “\(s.name.string)”")
                                }
                                .buttonStyle(UuobButton(.danger))
                            }
                        }
                    }
                    ForEach(sharing, id: \.container.string) { s in
                        HStack(spacing: 10) {
                            AppTile(app: s.app.string, size: 28)
                            TwoLines(title: s.name.string, detail: "You share · \(s.app_name.string)")
                            Spacer()
                            Segmented([("read", "Can view"), ("write", "Can edit")], selected: s.access.string) { v in
                                act("share_with", ["container": s.container.string, "person": p.id, "access": v])
                            }
                            Button { act("share_with", ["container": s.container.string, "person": p.id, "access": nil], toast: "Stopped sharing “\(s.name.string)” with \(p.name.string)") } label: {
                                Ico("close", size: 16)
                            }
                            .buttonStyle(IconButtonStyle()).help("Stop sharing")
                        }
                    }
                    if receiving.isEmpty && sharing.isEmpty {
                        Text("Nothing shared yet. Open a list in Tasks or a calendar in Calendar and choose Share.").font(.system(size: 13)).foregroundStyle(Look.text2)
                    }
                }
            }

        }
        .task(id: shell.tick) { await load() }
        .onChange(of: shell.people.contains { $0.id == only }) { _, here in if !here { dismiss() } }
        .sheet(isPresented: $naming) {
            NameSheet(current: afterName == nil ? me : "", continues: afterName != nil) { name in
                try await Core.call("set_name", ["name": name])
                me = name.trimmingCharacters(in: .whitespaces)
                let next = afterName
                afterName = nil
                naming = false
                if let next { sheet = next }
            }
        }
        .sheet(item: $sheet) { s in
            switch s {
            case .add: AddPersonSheet()
            case .meet: MeetSheet()
            }
        }
        .confirm("Remove \(removing?.name.string ?? "")?", isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
                 message: "Everything you share with each other stops. What \(removing?.name.string ?? "") shared disappears from your devices, and what you shared disappears from theirs the next time they connect.",
                 action: "Remove") {
            guard let p = removing else { return }
            act("remove_person", ["id": p.id], toast: "\(p.name.string) was removed")
        }
    }
}

/// "Your name": how you appear to people you share with.
struct NameSheet: View {
    let current: String
    let continues: Bool
    let save: (String) async throws -> Void
    /// What Cancel does, if not closing the sheet.
    var cancel: (() -> Void)? = nil

    var body: some View {
        NameEditor(placeholder: "Your name", under: "How you appear to people you share with", current: current,
                   saveLabel: continues ? "Continue" : "Save", save: save, cancel: cancel)
    }
}

/// A code for someone to link with you. The new person in the list is the one who used it.
struct AddPersonSheet: View {
    @Environment(Shell.self) private var shell
    @Environment(\.dismiss) private var dismiss
    @State private var invite: J?
    @State private var error: String?
    @State private var before: Set<String> = []

    private var linked: String? {
        guard invite != nil else { return nil }
        return shell.people.first { !before.contains($0.id) }?.name.string
    }

    var body: some View {
        Group {
            if let linked {
                Success(title: "Linked with \(linked)", text: "Nothing is shared yet. Open a list in Tasks or a calendar in Calendar and choose Share.") { dismiss() }
            } else if let invite {
                CodeCard(title: "Add a person", lead: "They choose Access → Add → A person: enter their code, and type this. It links you; nothing is shared yet.", onClose: close) {
                    CodeView(invite: invite, waiting: "Waiting for them…")
                }
            } else if let error {
                DetailSheet(title: "Couldn’t create a code", onClose: { dismiss() }, actions: { EmptyView() }) { SheetLead(error) }
            } else {
                HStack { ProgressView().controlSize(.small); Text("Creating a code…") }.padding(40)
            }
        }
        .task {
            before = Set(shell.people.map(\.id))
            do { invite = try await Core.call("create_person_invite") } catch { self.error = error.localizedDescription }
        }
    }

    private func close() {
        if let code = invite?.code.str, linked == nil { Task { try? await Core.call("cancel_invite", ["code": code]) } }
        dismiss()
    }
}

/// Type the code someone else shows.
struct MeetSheet: View {
    @Environment(Shell.self) private var shell

    var body: some View {
        CodeEntry(title: "Enter a code", lead: "They choose Access → Add → A person: show my code, and tell you what it says.",
                  note: "You see only what they share with you.", action: "Link", busyAction: "Linking…") { code in
            try await Core.call("meet", ["code": code])
            await shell.refresh()
            shell.show("Linked with \(shell.people.last?.name.string ?? "them")")
        }
    }
}
