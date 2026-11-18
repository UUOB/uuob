import SwiftUI

private func can(_ access: String) -> String { access == "write" ? "Can edit" : "Can view" }

struct PeopleView: View {
    @Environment(Shell.self) private var shell
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
        Form {
            Section {
                if !me.isEmpty {
                    HStack(spacing: 12) {
                        Avatar(name: me, mine: true)
                        TwoLines(title: me, detail: "You · the name other people see")
                        Spacer()
                        Button("Change…") { afterName = nil; naming = true }
                    }
                }
            } header: {
                Lead("Share a list or a calendar with someone who has their own UUOB. They see only what you share with them. Everything else stays private.")
            }

            if shell.people.isEmpty {
                Section {
                    ContentUnavailableView("No people yet", systemImage: "person.2",
                                           description: Text("Link with a partner, a family member or a colleague. Then share a list or a calendar with them from Tasks or Calendar."))
                }
            }

            ForEach(shell.people) { p in
                Section {
                    HStack(spacing: 12) {
                        Avatar(name: p.name.string)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(p.name.string).font(.headline)
                            HStack(spacing: 6) { Dot(on: p.online.bool); Text(p.online.bool ? "Online" : "Offline").foregroundStyle(.secondary) }
                        }
                        Spacer()
                        Button(role: .destructive) { removing = p } label: { Image(systemName: "trash") }
                            .buttonStyle(.borderless).help("Remove \(p.name.string)")
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
                                .buttonStyle(.borderedProminent)
                            } else {
                                Button("Leave", role: .destructive) {
                                    act("delete_container", ["id": s.container.string], toast: "You left “\(s.name.string)”")
                                }
                            }
                        }
                    }
                    ForEach(sharing, id: \.container.string) { s in
                        HStack(spacing: 10) {
                            AppTile(app: s.app.string, size: 28)
                            TwoLines(title: s.name.string, detail: "You share · \(s.app_name.string)")
                            Spacer()
                            Picker("", selection: Binding(get: { s.access.string }, set: { v in
                                act("share_with", ["container": s.container.string, "person": p.id, "access": v])
                            })) {
                                Text("Can view").tag("read")
                                Text("Can edit").tag("write")
                            }
                            .pickerStyle(.segmented).labelsHidden().fixedSize()
                            Button { act("share_with", ["container": s.container.string, "person": p.id, "access": nil], toast: "Stopped sharing “\(s.name.string)” with \(p.name.string)") } label: {
                                Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
                            }
                            .buttonStyle(.borderless).help("Stop sharing")
                        }
                    }
                    if receiving.isEmpty && sharing.isEmpty {
                        Text("Nothing shared yet. Open a list in Tasks or a calendar in Calendar and choose Share.").foregroundStyle(.secondary)
                    }
                }
            }

            Section {
                HStack {
                    Button("Add a Person…") { start(.add) }.buttonStyle(.borderedProminent)
                    Button("Enter a Code…") { start(.meet) }
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle("People")
        .toolbar {
            ToolbarItem {
                Button { start(.add) } label: { Label("Add a Person", systemImage: "person.badge.plus") }.help("Add a person")
            }
        }
        .task(id: shell.tick) { await load() }
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
        .alert("Remove \(removing?.name.string ?? "")?", isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } })) {
            Button("Remove", role: .destructive) {
                guard let p = removing else { return }
                act("remove_person", ["id": p.id], toast: "\(p.name.string) was removed")
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            let n = removing?.name.string ?? ""
            Text("Everything you share with each other stops. What \(n) shared disappears from your devices, and what you shared disappears from theirs the next time they connect.")
        }
    }
}

/// "Your name": how you appear to people you share with.
struct NameSheet: View {
    @Environment(\.dismiss) private var dismiss
    let current: String
    let continues: Bool
    let save: (String) async throws -> Void
    /// What Cancel does, if not closing the sheet.
    var cancel: (() -> Void)? = nil
    @State private var name = ""
    @State private var error: String?

    var body: some View {
        SheetFrame(title: "Your name", lead: "This is how you appear to people you share with.") {
            TextField("e.g. Alex", text: $name).textFieldStyle(.roundedBorder).font(.title3).onSubmit(submit)
            if let error { Text(error).foregroundStyle(.red) }
        } buttons: {
            Button("Cancel", role: .cancel) { if let cancel { cancel() } else { dismiss() } }.keyboardShortcut(.cancelAction)
            Button(continues ? "Continue" : "Save", action: submit).keyboardShortcut(.defaultAction)
                .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
        }
        .onAppear { name = current }
    }

    private func submit() {
        Task {
            do { try await save(name) } catch { self.error = error.localizedDescription }
        }
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
                SheetFrame(title: "Add a person", lead: "They open UUOB on their device, go to People → Enter a code and type this.", center: true) {
                    CodeView(invite: invite, waiting: "Waiting for them…").padding(.vertical, 8)
                    Text("This links the two of you. It doesn’t share anything, and it doesn’t add them to your devices.")
                        .font(.callout).foregroundStyle(.secondary).multilineTextAlignment(.center)
                } buttons: {
                    Button("Cancel", role: .cancel) { close() }.keyboardShortcut(.cancelAction)
                }
            } else if let error {
                SheetFrame(title: "Couldn’t create a code", lead: error) { EmptyView() } buttons: {
                    Button("Close") { dismiss() }.keyboardShortcut(.cancelAction)
                }
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
    @Environment(\.dismiss) private var dismiss
    @State private var code = ""
    @State private var busy = false
    @State private var error: String?

    var body: some View {
        SheetFrame(title: "Enter a code", lead: "Ask the other person to open People → Add a person in their UUOB (or to share something with someone new), and type the code they see.") {
            TextField("e.g. 42 amber river falcon", text: $code).textFieldStyle(.roundedBorder).font(.title3).autocorrectionDisabled().onSubmit(meet)
            if let error { Text(error).foregroundStyle(.red) }
            Text("Your devices and data stay your own. You see only what they share with you.").font(.callout).foregroundStyle(.secondary)
        } buttons: {
            Button("Cancel", role: .cancel) { dismiss() }.keyboardShortcut(.cancelAction)
            Button(busy ? "Linking…" : "Link", action: meet).keyboardShortcut(.defaultAction).disabled(busy || !complete(code))
        }
    }

    private func meet() {
        guard complete(code), !busy else { return }
        busy = true
        error = nil
        Task {
            do {
                try await Core.call("meet", ["code": code])
                await shell.refresh()
                shell.show("Linked with \(shell.people.last?.name.string ?? "them")")
                dismiss()
            } catch {
                self.error = error.localizedDescription
            }
            busy = false
        }
    }
}
