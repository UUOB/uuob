// UUOB's core (Rust, crates/uuob-app), as the Mac app uses it. The functions
// it calls (start, call, nextEvent) are written by UniFFI into UuobCore.swift
// when the app is built (mac/build.sh). Methods and answers are JSON, the
// same that apps and AI clients use.

import Foundation

/// A JSON value from the core, read like `device.name.string` or `status.network.copies.int`.
@dynamicMemberLookup
struct J: Identifiable {
    let value: Any?

    init(_ value: Any?) { self.value = value is NSNull ? nil : value }

    subscript(dynamicMember key: String) -> J { self[key] }
    subscript(key: String) -> J { J((value as? [String: Any])?[key]) }

    var str: String? { value as? String }
    var string: String { str ?? "" }
    var int: Int { (value as? NSNumber)?.intValue ?? 0 }
    var double: Double { (value as? NSNumber)?.doubleValue ?? 0 }
    var bool: Bool { (value as? NSNumber)?.boolValue ?? false }
    var isNull: Bool { value == nil }
    var array: [J] { ((value as? [Any]) ?? []).map(J.init) }
    /// An object's entries, sorted by key (e.g. the clients by id).
    var entries: [(key: String, value: J)] {
        ((value as? [String: Any]) ?? [:]).map { (key: $0.key, value: J($0.value)) }.sorted { $0.key < $1.key }
    }
    var id: String { self["id"].string }
    /// Milliseconds since 1970, as a date.
    var date: Date { Date(timeIntervalSince1970: double / 1000) }
}

enum Core {
    /// A method that failed; the message is for the user.
    struct Failed: LocalizedError {
        let code: String
        let message: String
        var errorDescription: String? { message }
    }

    /// Where UUOB keeps its data on a Mac (the same folder as before it was native).
    static var dataDir: String {
        if let dir = ProcessInfo.processInfo.environment["UUOB_DATA_DIR"] { return dir }
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return support.appendingPathComponent("app.uuob").path
    }

    static func start() async throws {
        do {
            try await UUOB.start(dataDir: dataDir)
        } catch UuobError.Failed(let code, let detail) {
            throw Failed(code: code, message: detail)
        }
    }

    /// Runs `method`; nil values in `params` are sent as JSON null.
    @discardableResult
    static func call(_ method: String, _ params: [String: Any?] = [:]) async throws -> J {
        let body = try JSONSerialization.data(withJSONObject: params.mapValues { $0 ?? NSNull() })
        do {
            let text = try await UUOB.call(method: method, params: String(decoding: body, as: UTF8.self))
            return J(try JSONSerialization.jsonObject(with: Data(text.utf8), options: [.fragmentsAllowed]))
        } catch UuobError.Failed(let code, let detail) {
            throw Failed(code: code, message: detail)
        }
    }

    /// What changes, e.g. {"type": "device_joined", "name": "…"}. "lagged" means: reload everything.
    static func events() -> AsyncStream<J> {
        AsyncStream { continuation in
            let task = Task.detached {
                while !Task.isCancelled {
                    let text = await UUOB.nextEvent()
                    continuation.yield(J(try? JSONSerialization.jsonObject(with: Data(text.utf8))))
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }
}
