// UUOB's sheets in the frames every UUOB app shares (sdk/kit/mac/Scaffold.swift):
// the editor for a name, the detail card for codes and choices.

import SwiftUI

/// A grey line under a sheet's title.
struct SheetLead: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text).font(.system(size: 15)).foregroundStyle(Look.text2)
            .fixedSize(horizontal: false, vertical: true).padding(.top, 6)
    }
}

/// A name in the editor: big at the top, with Save (or Continue, Create) at the bottom.
struct NameEditor: View {
    @Environment(\.dismiss) private var dismiss
    let placeholder: String
    /// A grey line under the name: what it is for.
    var under = ""
    var current = ""
    var isNew = false
    var saveLabel: String? = nil
    let save: (String) async throws -> Void
    /// What Cancel does, if not closing the sheet.
    var cancel: (() -> Void)? = nil
    @State private var name = ""
    @State private var error: String?

    private var trimmed: String { name.trimmingCharacters(in: .whitespaces) }

    var body: some View {
        EditSheet(placeholder: placeholder, title: $name, isNew: isNew, canSave: !trimmed.isEmpty, saveLabel: saveLabel,
                  onCancel: { if let cancel { cancel() } else { dismiss() } }, onSave: submit) {
            if !under.isEmpty { Text(under).font(.system(size: 13)).foregroundStyle(Look.text2) }
        } content: {
            if let error { Text(error).font(.system(size: 13)).foregroundStyle(Look.danger).padding(.top, 4) }
        }
        .onAppear { name = current }
    }

    private func submit() {
        guard !trimmed.isEmpty else { return }
        Task {
            do { try await save(name) } catch { self.error = error.localizedDescription }
        }
    }
}

/// A code another device shows, typed here: the field, a short note, and Join (or Link).
struct CodeEntry: View {
    @Environment(\.dismiss) private var dismiss
    let title: String
    let lead: String
    let note: String
    let action: String
    let busyAction: String
    let perform: (String) async throws -> Void
    @State private var code = ""
    @State private var busy = false
    @State private var error: String?

    var body: some View {
        DetailSheet(title: title, onClose: { dismiss() }, actions: { EmptyView() }) {
            SheetLead(lead)
            TextField("e.g. 42 amber river falcon", text: $code)
                .textFieldStyle(UuobField()).font(.title3).autocorrectionDisabled()
                .onSubmit(go).padding(.top, 16)
            if let error { Text(error).font(.system(size: 13)).foregroundStyle(Look.danger).padding(.top, 8) }
            Text(note).font(.system(size: 13)).foregroundStyle(Look.text2).padding(.top, 10)
        } footer: {
            HStack {
                Spacer()
                Button(busy ? busyAction : action, action: go)
                    .buttonStyle(UuobButton(.primary, small: true)).keyboardShortcut(.defaultAction)
                    .disabled(busy || !complete(code))
            }
            .padding(.top, 18)
        }
    }

    private func go() {
        guard complete(code), !busy else { return }
        busy = true
        error = nil
        Task {
            do {
                try await perform(code)
                dismiss()
            } catch {
                self.error = error.localizedDescription
            }
            busy = false
        }
    }
}

/// A code to type on the other device, in the middle of a detail card.
struct CodeCard<Code: View>: View {
    let title: String
    let lead: String
    let onClose: () -> Void
    @ViewBuilder var code: () -> Code

    var body: some View {
        DetailSheet(title: title, onClose: onClose, actions: { EmptyView() }) {
            SheetLead(lead)
            code().frame(maxWidth: .infinity).padding(.top, 18).padding(.bottom, 4)
        }
    }
}

/// "Add": an app, a person (your code or theirs) or an AI. The + in Access and ⌘N.
struct AddChooser: View {
    @Environment(\.dismiss) private var dismiss
    let pick: (AccessView.Adding) -> Void

    var body: some View {
        DetailSheet(title: "Add", onClose: { dismiss() }, actions: { EmptyView() }) {
            VStack(spacing: 0) {
                choice("apps", "An app, with a code", .app)
                choice("person", "A person: show my code", .showCode)
                choice("link", "A person: enter their code", .enterCode)
                choice("ai", "An AI", .ai, last: true)
            }
            .padding(.top, 10)
        }
    }

    private func choice(_ icon: String, _ title: String, _ what: AccessView.Adding, last: Bool = false) -> some View {
        Button { pick(what) } label: {
            IconRow(icon, last: last) {
                Text(title).font(.system(size: 15))
                Spacer()
                Ico("chevron", size: 16).foregroundStyle(Look.text3)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
