// UUOB's own controls, the same as when its screens were a web page
// (ui/tokens.css): its colours light and dark, pill buttons, groups of rows
// with a hairline edge, the green switch, the segmented control, large page
// titles and its line icons. Sizes and colours are taken from there.

import AppKit
import SwiftUI

// MARK: - Colours

extension Color {
    /// A colour for light and one for dark.
    init(light: UInt32, dark: UInt32, alpha: Double = 1) {
        func rgb(_ v: UInt32) -> NSColor {
            NSColor(srgbRed: CGFloat((v >> 16) & 0xff) / 255, green: CGFloat((v >> 8) & 0xff) / 255, blue: CGFloat(v & 0xff) / 255, alpha: alpha)
        }
        self.init(nsColor: NSColor(name: nil) { $0.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? rgb(dark) : rgb(light) })
    }
}

/// The tokens.
enum Look {
    static let bg = Color(light: 0xf5f5f7, dark: 0x0b0b0d)
    static let elevated = Color(light: 0xffffff, dark: 0x161618)
    static let surface = Color(light: 0xffffff, dark: 0x1c1c1f)
    static let surface2 = Color(light: 0xf0f0f3, dark: 0x26262a)
    static let rail = Color(light: 0xececf0, dark: 0x141416)
    static let low = Color(light: 0xfafafb, dark: 0x161618)
    static let text = Color(light: 0x1d1d1f, dark: 0xf5f5f7)
    static let text2 = Color(light: 0x1d1d1f, dark: 0xf5f5f7, alpha: 0.62)
    static let text3 = Color(light: 0x1d1d1f, dark: 0xf5f5f7, alpha: 0.38)
    static let line = Color(light: 0x000000, dark: 0xffffff, alpha: 0.08)
    static let lineStrong = Color(light: 0x000000, dark: 0xffffff, alpha: 0.14)
    static let hover = Color(light: 0x000000, dark: 0xffffff, alpha: 0.06)
    static let active = Color(light: 0x000000, dark: 0xffffff, alpha: 0.1)
    static let accent = Color(light: 0x007aff, dark: 0x0a84ff)
    static let accentSoft = Color(light: 0x007aff, dark: 0x0a84ff, alpha: 0.16)
    static let danger = Color(light: 0xff3b30, dark: 0xff453a)
    static let dangerSoft = Color(light: 0xff3b30, dark: 0xff453a, alpha: 0.14)
    static let ok = Color(light: 0x34c759, dark: 0x30d158)
    static let warn = Color(red: 1, green: 0.62, blue: 0.04)
    /// A person's face: green to blue.
    static let person = LinearGradient(colors: [Color(red: 0.19, green: 0.77, blue: 0.55), Color(red: 0.16, green: 0.62, blue: 0.96)], startPoint: .topLeading, endPoint: .bottomTrailing)
}

// MARK: - Icons

/// UUOB's line icons (24 point grid, 1.75 stroke), as ui/Icon.svelte drew them.
let iconPaths: [String: String] = [
    "plus": "M12 5v14M5 12h14",
    "trash": "M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3",
    "link": "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
    "close": "M6 6l12 12M18 6L6 18",
    "check": "M5 12l4 4 10-10",
    "copy": "M9 9h10v10a1 1 0 0 1-1 1H10a1 1 0 0 1-1-1zM5 15V5a1 1 0 0 1 1-1h9",
    "lock": "M6 11h12v9H6zM8 11V8a4 4 0 0 1 8 0v3",
    "chevron": "M9 6l6 6-6 6",
    "back": "M15 6l-6 6 6 6",
    "right": "M5 12h14M13 6l6 6-6 6",
    "edit": "M4 20h4L19 9l-4-4L4 16zM13 7l4 4",
    "laptop": "M4 6a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v9H4zM2 18h20",
    "phone": "M8 3h8a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM11 18h2",
    "server": "M5 4h14a1 1 0 0 1 1 1v5H4V5a1 1 0 0 1 1-1zM4 14h16v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1zM8 7h.01M8 17h.01",
    "question": "M9.1 9a3 3 0 0 1 5.8 1c0 2-3 2.5-3 4.5M12 18h.01",
    "settings": "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
    "people": "M9 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7zM2.5 20a6.5 6.5 0 0 1 13 0M16 4.2a3.5 3.5 0 0 1 0 6.6M18 14.3a6.5 6.5 0 0 1 3.5 5.7",
    "person": "M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4.5 21a7.5 7.5 0 0 1 15 0",
    "ai": "M12 3l1.8 4.7L18.5 9.5l-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8zM19 15l.8 2.2 2.2.8-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z",
    "apps": "M5 4h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM15 4h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM5 14h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1zM15 14h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1z",
    "network": "M14.5 5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM7.5 18a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM21.5 18a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM10.8 7.2L6.2 15.8M13.2 7.2l4.6 8.6M7.5 18h9",
    "shield": "M12 3l7 3v5c0 4.5-3 8.3-7 10-4-1.7-7-5.5-7-10V6zM9 12l2 2 4-4",
    "download": "M12 4v11M7 10l5 5 5-5M5 20h14",
]

/// One of the icons, as a shape on the 24 point grid.
struct IconShape: Shape {
    let name: String
    func path(in rect: CGRect) -> Path {
        svgPath(iconPaths[name] ?? "").applying(CGAffineTransform(scaleX: rect.width / 24, y: rect.height / 24))
    }
}

/// An icon, drawn in the current foreground colour.
struct Ico: View {
    let name: String
    var size: CGFloat = 20
    var stroke: CGFloat = 1.75
    init(_ name: String, size: CGFloat = 20, stroke: CGFloat = 1.75) {
        self.name = name
        self.size = size
        self.stroke = stroke
    }
    var body: some View {
        IconShape(name: name)
            .stroke(style: StrokeStyle(lineWidth: stroke * size / 24, lineCap: .round, lineJoin: .round))
            .frame(width: size, height: size)
    }
}

/// A path from SVG path data: M L H V C S Q T A Z, absolute and relative
/// (what the icons use).
func svgPath(_ d: String) -> Path {
    var path = Path()
    var tokens: [String] = []
    var number = ""
    func flush() { if !number.isEmpty { tokens.append(number); number = "" } }
    for ch in d {
        if ch.isLetter && ch != "e" {
            flush(); tokens.append(String(ch))
        } else if ch == "," || ch == " " {
            flush()
        } else if ch == "-" && !number.isEmpty && number.last != "e" {
            flush(); number = "-"
        } else if ch == "." && number.contains(".") {
            flush(); number = "."
        } else {
            number.append(ch)
        }
    }
    flush()
    var i = 0
    var command = "M"
    var current = CGPoint.zero, start = CGPoint.zero, control = CGPoint.zero
    func next() -> CGFloat { defer { i += 1 }; return CGFloat(Double(tokens[i]) ?? 0) }
    func hasNumber() -> Bool { i < tokens.count && !(tokens[i].first?.isLetter ?? false) }
    while i < tokens.count {
        if tokens[i].first?.isLetter == true { command = tokens[i]; i += 1 }
        let rel = command == command.lowercased()
        let base = rel ? current : .zero
        switch command.uppercased() {
        case "M":
            current = CGPoint(x: base.x + next(), y: base.y + next())
            path.move(to: current); start = current
            command = rel ? "l" : "L"
        case "L":
            current = CGPoint(x: base.x + next(), y: base.y + next()); path.addLine(to: current)
        case "H":
            current.x = (rel ? current.x : 0) + next(); path.addLine(to: current)
        case "V":
            current.y = (rel ? current.y : 0) + next(); path.addLine(to: current)
        case "C":
            let c1 = CGPoint(x: base.x + next(), y: base.y + next())
            let c2 = CGPoint(x: base.x + next(), y: base.y + next())
            current = CGPoint(x: base.x + next(), y: base.y + next())
            path.addCurve(to: current, control1: c1, control2: c2); control = c2
        case "S":
            let c1 = CGPoint(x: 2 * current.x - control.x, y: 2 * current.y - control.y)
            let c2 = CGPoint(x: base.x + next(), y: base.y + next())
            current = CGPoint(x: base.x + next(), y: base.y + next())
            path.addCurve(to: current, control1: c1, control2: c2); control = c2
        case "Q":
            let c = CGPoint(x: base.x + next(), y: base.y + next())
            current = CGPoint(x: base.x + next(), y: base.y + next())
            path.addQuadCurve(to: current, control: c); control = c
        case "A":
            let r = next(); _ = next(); _ = next()
            let large = next() != 0, sweep = next() != 0
            let end = CGPoint(x: base.x + next(), y: base.y + next())
            arc(&path, from: current, to: end, radius: r, large: large, sweep: sweep)
            current = end
        case "Z":
            path.closeSubpath(); current = start
        default:
            i += 1
        }
        if command.uppercased() == "Z" { continue }
        if !hasNumber() { continue }
    }
    return path
}

/// A circular arc from `a` to `b` (the icons' arcs are all circles).
private func arc(_ path: inout Path, from a: CGPoint, to b: CGPoint, radius: CGFloat, large: Bool, sweep: Bool) {
    let dx = (b.x - a.x) / 2, dy = (b.y - a.y) / 2
    let d = sqrt(dx * dx + dy * dy)
    guard d > 0 else { return }
    let r = max(radius, d)
    let h = sqrt(max(0, r * r - d * d))
    // The centre is on the perpendicular through the midpoint, on the side the flags choose.
    let sign: CGFloat = large == sweep ? -1 : 1
    let mid = CGPoint(x: (a.x + b.x) / 2, y: (a.y + b.y) / 2)
    let centre = CGPoint(x: mid.x + sign * h * (-dy / d), y: mid.y + sign * h * (dx / d))
    let start = atan2(a.y - centre.y, a.x - centre.x), end = atan2(b.y - centre.y, b.x - centre.x)
    // SVG's sweep 1 is clockwise on screen; SwiftUI's `clockwise` is for a flipped y.
    path.addArc(center: centre, radius: r, startAngle: .radians(start), endAngle: .radians(end), clockwise: !sweep)
}

// MARK: - Buttons

enum Kind { case plain, primary, danger, ghost }

/// `.btn`: a pill, 40 points high (`.lg`: 50), grey, blue, red or just blue text.
/// A destructive button turns red by itself.
struct UuobButton: ButtonStyle {
    var kind: Kind = .plain
    var large = false
    init(_ kind: Kind = .plain, large: Bool = false) {
        self.kind = kind
        self.large = large
    }
    @Environment(\.isEnabled) private var enabled
    /// Buttons side by side at the bottom of a sheet share its width (`.actions`).
    @Environment(\.fillButtons) private var fill
    func makeBody(configuration: Configuration) -> some View {
        let kind = configuration.role == .destructive && self.kind == .plain ? .danger : self.kind
        let (bg, fg): (Color, Color) = switch kind {
        case .plain: (Look.surface2, Look.text)
        case .primary: (Look.accent, .white)
        case .danger: (Look.dangerSoft, Look.danger)
        case .ghost: (.clear, Look.accent)
        }
        return configuration.label
            .font(.system(size: large ? 17 : 15, weight: .semibold))
            .foregroundStyle(fg)
            .padding(.horizontal, large ? 26 : 18)
            .frame(maxWidth: fill ? .infinity : nil, minHeight: large ? 50 : 40)
            .background(bg.opacity(configuration.isPressed ? 0.8 : 1), in: Capsule())
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .opacity(enabled ? 1 : 0.45)
            .contentShape(Capsule())
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
    }
}

private struct FillButtons: EnvironmentKey { static let defaultValue = false }
extension EnvironmentValues {
    var fillButtons: Bool {
        get { self[FillButtons.self] }
        set { self[FillButtons.self] = newValue }
    }
}

/// `.icon-btn`: round, 36 points, grey until hovered.
struct IconButtonStyle: ButtonStyle {
    @State private var hover = false
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(hover ? Look.text : Look.text2)
            .frame(width: 36, height: 36)
            .background(hover || configuration.isPressed ? Look.hover : .clear, in: Circle())
            .contentShape(Circle())
            .onHover { hover = $0 }
    }
}

// MARK: - Switch and segments

/// `.switch`: 51 by 31, green when on; the knob springs across.
struct UuobSwitch: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 14) {
            configuration.label
            Spacer(minLength: 0)
            Capsule().fill(configuration.isOn ? Look.ok : Look.lineStrong)
                .frame(width: 51, height: 31)
                .overlay(alignment: configuration.isOn ? .trailing : .leading) {
                    Circle().fill(.white).frame(width: 27, height: 27).padding(2).shadow(color: .black.opacity(0.3), radius: 2.5, y: 2)
                }
                .animation(.spring(response: 0.3, dampingFraction: 0.65), value: configuration.isOn)
                .onTapGesture { configuration.isOn.toggle() }
                .accessibilityElement().accessibilityAddTraits(.isButton).accessibilityValue(configuration.isOn ? "On" : "Off")
        }
    }
}

/// `.segmented`: a grey track, the chosen one raised.
struct Segmented: View {
    let options: [(value: String, label: String)]
    @Binding var selection: String
    var stretch = false

    var body: some View {
        HStack(spacing: 2) {
            ForEach(options, id: \.value) { o in
                let on = o.value == selection
                Button { selection = o.value } label: {
                    Text(o.label).font(.system(size: 14, weight: .medium)).lineLimit(1)
                        .foregroundStyle(on ? Look.text : Look.text2)
                        .padding(.horizontal, 14).padding(.vertical, 5)
                        .frame(maxWidth: stretch ? .infinity : nil)
                        .background(on ? Look.elevated : .clear, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                        .shadow(color: on ? .black.opacity(0.25) : .clear, radius: 1.5, y: 1)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(3)
        .background(Look.surface2, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .fixedSize(horizontal: !stretch, vertical: true)
    }
}

extension Segmented {
    /// A segmented choice that runs `pick` when changed (for values kept elsewhere).
    init(_ options: [(String, String)], selected: String, stretch: Bool = false, pick: @escaping (String) -> Void) {
        self.options = options.map { (value: $0.0, label: $0.1) }
        self._selection = Binding(get: { selected }, set: pick)
        self.stretch = stretch
    }
}

// MARK: - Pages, groups, rows

/// A page as UUOB's always were: a large title, a grey line about it, the
/// content in a column no wider than 680 points. With `back`, a link above
/// the title goes back.
struct Page<Content: View, Trailing: View>: View {
    let title: String
    var lead: String = ""
    var back: (() -> Void)? = nil
    @ViewBuilder var trailing: Trailing
    @ViewBuilder var content: Content

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if let back {
                    Button(action: back) { HStack(spacing: 4) { Ico("back", size: 18); Text("Back") } }
                        .buttonStyle(UuobButton(.ghost)).padding(.leading, -18).padding(.bottom, 12)
                }
                HStack(alignment: .firstTextBaseline) {
                    Text(title).font(.system(size: 34, weight: .bold)).tracking(-0.85).foregroundStyle(Look.text)
                    Spacer()
                    trailing
                }
                if !lead.isEmpty {
                    Text(lead).font(.system(size: 17)).foregroundStyle(Look.text2).padding(.top, 10).frame(maxWidth: 560, alignment: .leading)
                }
                content.padding(.top, 14)
            }
            .frame(maxWidth: 680)
            .padding(.horizontal, 32).padding(.top, 56).padding(.bottom, 48)
            .frame(maxWidth: .infinity)
        }
        .background(Look.bg)
        .buttonStyle(UuobButton())
        .toggleStyle(UuobSwitch())
        .foregroundStyle(Look.text)
        .navigationBarBackButtonHidden(true)
    }
}

extension Page where Trailing == EmptyView {
    init(title: String, lead: String = "", back: (() -> Void)? = nil, @ViewBuilder content: () -> Content) {
        self.title = title
        self.lead = lead
        self.back = back
        self.trailing = EmptyView()
        self.content = content()
    }
}

/// `.section-label`: small capitals, grey.
struct SectionLabel: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text.uppercased()).font(.system(size: 13, weight: .semibold)).tracking(0.5).foregroundStyle(Look.text3)
    }
}

/// `.group`: rows on one card with a hairline edge, a line between rows;
/// a label above it, a note under it. (Like Form's Section, which it replaces.)
struct Card<Content: View, Header: View, Footer: View>: View {
    @ViewBuilder var content: Content
    @ViewBuilder var header: Header
    @ViewBuilder var footer: Footer

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            header.padding(.leading, 4)
            _VariadicView.Tree(Rows()) { content }
                .background(Look.surface, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(Look.line))
            footer.font(.system(size: 13)).foregroundStyle(Look.text2).padding(.horizontal, 4).padding(.top, 2)
        }
        .padding(.top, 28)
    }

    /// Each child a row: 14 by 18 points of room, at least 56 high, a hairline above all but the first.
    struct Rows: _VariadicView_MultiViewRoot {
        func body(children: _VariadicView.Children) -> some View {
            VStack(spacing: 0) {
                ForEach(children) { child in
                    if child.id != children.first?.id { Rectangle().fill(Look.line).frame(height: 1) }
                    child.padding(.horizontal, 18).padding(.vertical, 14).frame(maxWidth: .infinity, minHeight: 56, alignment: .leading)
                }
            }
        }
    }
}

extension Card where Header == EmptyView, Footer == EmptyView {
    init(@ViewBuilder content: () -> Content) {
        self.content = content(); self.header = EmptyView(); self.footer = EmptyView()
    }
}

extension Card where Header == SectionLabel, Footer == EmptyView {
    init(_ title: String, @ViewBuilder content: () -> Content) {
        self.content = content(); self.header = SectionLabel(title); self.footer = EmptyView()
    }
}

extension Card where Footer == EmptyView {
    init(@ViewBuilder content: () -> Content, @ViewBuilder header: () -> Header) {
        self.content = content(); self.header = header(); self.footer = EmptyView()
    }
}

extension Card {
    init(@ViewBuilder content: () -> Content, @ViewBuilder header: () -> Header, @ViewBuilder footer: () -> Footer) {
        self.content = content(); self.header = header(); self.footer = footer()
    }
}

/// `.tag`: "This device", small and blue in a pill.
struct Tag: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text).font(.system(size: 12, weight: .semibold)).foregroundStyle(Look.accent)
            .padding(.horizontal, 8).padding(.vertical, 2).background(Look.accentSoft, in: Capsule())
    }
}

// MARK: - Rows, fields, confirmations

/// A row: what it is on the left, a control on the right (Form's LabeledContent, outside a Form).
struct Trailing<Label: View, Content: View>: View {
    @ViewBuilder var content: Content
    @ViewBuilder var label: Label
    var body: some View {
        HStack(spacing: 14) {
            label
            Spacer(minLength: 0)
            content
        }
    }
}

extension Trailing where Label == Text {
    init(_ title: String, @ViewBuilder content: () -> Content) {
        self.content = content()
        self.label = Text(title).font(.system(size: 16, weight: .semibold))
    }
}

/// `.field`: 46 high, rounded, on grey; a blue ring while typing.
struct UuobField: TextFieldStyle {
    @FocusState private var focused: Bool
    func _body(configuration: TextField<Self._Label>) -> some View {
        configuration
            .textFieldStyle(.plain)
            .font(.system(size: 17))
            .focused($focused)
            .padding(.horizontal, 14)
            .frame(minHeight: 46)
            .background(focused ? Look.surface : Look.surface2, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(focused ? Look.accent : Look.line))
            .overlay(RoundedRectangle(cornerRadius: 15, style: .continuous).stroke(focused ? Look.accentSoft : .clear, lineWidth: 4).padding(-3))
    }
}

/// A question before something that can't be undone, as one of UUOB's sheets.
struct ConfirmSheet: View {
    @Environment(\.dismiss) private var dismiss
    let title: String
    let message: String
    let action: String
    let perform: () -> Void

    var body: some View {
        SheetFrame(title: title, lead: message) {
            EmptyView()
        } buttons: {
            Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
            Button(action, role: .destructive) {
                perform()
                dismiss()
            }
            .buttonStyle(UuobButton(.danger, large: true)).keyboardShortcut(.defaultAction)
        }
    }
}

/// A sheet asking for one line of text (a device's name).
struct TextSheet: View {
    @Environment(\.dismiss) private var dismiss
    let title: String
    @State var text: String
    let save: (String) -> Void

    var body: some View {
        SheetFrame(title: title) {
            TextField("Name", text: $text).textFieldStyle(UuobField()).onSubmit(done)
        } buttons: {
            Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
            Button("Save", action: done).buttonStyle(UuobButton(.primary, large: true)).keyboardShortcut(.defaultAction)
                .disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
        }
    }

    private func done() {
        guard !text.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        save(text)
        dismiss()
    }
}

extension View {
    /// Asks `title` (with `message`) in a sheet; `perform` runs on `action`.
    func confirm(_ title: String, isPresented: Binding<Bool>, message: String, action: String, perform: @escaping () -> Void) -> some View {
        sheet(isPresented: isPresented) { ConfirmSheet(title: title, message: message, action: action, perform: perform) }
    }
}
