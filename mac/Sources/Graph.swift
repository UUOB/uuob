// The Network screen's graph. Where everything goes is worked out by UUOB
// (`graph`, crates/uuob-app/src/screens.rs), the same on every platform;
// this draws it: lines behind, a button per device, app, AI and person.

import SwiftUI

struct GraphView: View {
    @Environment(Shell.self) private var shell
    /// Groups unfolded (`apps:<device>`, `ai:<device>`, `people`).
    @State private var open: Set<String> = []
    @State private var picked: String?
    @State private var graph = J(nil)
    @State private var width: Double = 600

    private var nodes: [J] { graph.nodes.array }
    /// UUOB lays the graph out for 300 points or more; narrower, it is drawn smaller.
    private var scale: Double { graph.width.double > 0 ? min(1, width / graph.width.double) : 1 }
    /// The picked node, else this device.
    private var shown: J? { nodes.first { $0.id == picked } ?? nodes.first { $0["self"].bool } }

    private func load() async {
        if let g = try? await Core.call("graph", ["width": width, "open": Array(open)]) { graph = g }
    }

    var body: some View {
        VStack(spacing: 16) {
            GeometryReader { geo in
                ZStack(alignment: .topLeading) {
                    Canvas { ctx, _ in
                        for l in graph.lines.array {
                            var path = Path()
                            path.move(to: CGPoint(x: l.x1.double, y: l.y1.double))
                            path.addLine(to: CGPoint(x: l.x2.double, y: l.y2.double))
                            let style = l.style.string
                            let dash: [CGFloat] = style == "person" || style == "link" ? [5, 5] : []
                            ctx.stroke(path, with: .color(style == "device" ? Look.lineStrong : Look.lineStrong.opacity(0.6)),
                                       style: StrokeStyle(lineWidth: style == "device" ? 2.5 : 1.5, dash: dash))
                        }
                    }
                    ForEach(nodes) { n in
                        GraphNode(node: n, selected: n.id == shown?.id) {
                            switch n.kind.string {
                            case "bubble": open.insert(n.group.string)
                            case "fold": open.remove(n.group.string)
                            default: picked = n.id
                            }
                        }
                    }
                }
                .frame(width: graph.width.double, height: graph.height.double)
                .scaleEffect(scale, anchor: .topLeading)
                .frame(width: graph.width.double * scale, height: graph.height.double * scale, alignment: .topLeading)
                .frame(maxWidth: .infinity)
                .onAppear { width = geo.size.width }
                .onChange(of: geo.size.width) { _, w in if abs(w - width) > 20 { width = w } }
            }
            .frame(height: graph.height.double * scale)
            .background(Look.low, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 20, style: .continuous).strokeBorder(Look.line))

            if let about = shown?.about {
                VStack(alignment: .leading, spacing: 4) {
                    if !about.kind.string.isEmpty {
                        Text(about.kind.string.uppercased()).font(.system(size: 12, weight: .bold)).foregroundStyle(Look.accent)
                    }
                    Text(about.title.string).font(.system(size: 20, weight: .semibold))
                    if !about.detail.string.isEmpty { Text(about.detail.string).font(.system(size: 15)).foregroundStyle(Look.text2) }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 18).padding(.vertical, 16)
                .background(Look.surface, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(Look.line))
            }
        }
        .task(id: "\(shell.tick)|\(Int(width))|\(open.sorted())") { await load() }
    }
}

/// One node: its shape centred on (x, y), its name underneath.
private struct GraphNode: View {
    let node: J
    let selected: Bool
    let action: () -> Void

    private var size: Double { node.size.double }
    private var kind: String { node.kind.string }

    var body: some View {
        VStack(spacing: 4) {
            Button(action: action) { shape }
                .buttonStyle(.plain)
                .uuobHint(kind == "bubble" ? "Open \(node.count.int) \(node.label.string)" : kind == "fold" ? "Fold" : node.label.string)
                .accessibilityLabel(kind == "bubble" ? "Open \(node.count.int) \(node.label.string)" : kind == "fold" ? "Fold" : node.label.string)
            if kind != "fold" {
                Text(node.label.string)
                    .font(.caption.weight(node["self"].bool ? .semibold : .regular))
                    .foregroundStyle(node["self"].bool ? .primary : .secondary)
                    .lineLimit(1).frame(width: 100)
            }
        }
        // The shape's centre on (x, y): the label hangs below it.
        .position(x: node.x.double, y: node.y.double + (kind == "fold" ? 0 : 10))
    }

    @ViewBuilder private var shape: some View {
        let ring = selected && kind != "bubble" && kind != "fold"
        Group {
            switch kind {
            case "device":
                RoundedRectangle(cornerRadius: size * 0.3, style: .continuous)
                    .fill(node.dotted.bool ? AnyShapeStyle(Color.clear) : AnyShapeStyle(Look.surface2))
                    .overlay {
                        if node.dotted.bool {
                            RoundedRectangle(cornerRadius: size * 0.3, style: .continuous).strokeBorder(.secondary, style: StrokeStyle(lineWidth: 1.5, dash: [4, 3]))
                        }
                    }
                    .overlay(Ico(deviceIcon, size: size * 0.45).foregroundStyle(Look.text))
            case "app":
                AppTile(app: node.key.string.replacingOccurrences(of: "app:", with: ""), size: size)
            case "ai":
                RoundedRectangle(cornerRadius: size * 0.28, style: .continuous).fill(Color.purple)
                    .overlay(Ico("ai", size: size * 0.5).foregroundStyle(.white))
            case "person":
                Avatar(name: node.label.string, size: size)
            case "bubble":
                Circle().fill(Look.surface).overlay(Circle().strokeBorder(Look.lineStrong, style: StrokeStyle(lineWidth: 1.5, dash: [4, 3])))
                    .overlay(Text("\(node.count.int)").font(.system(size: size * 0.3, weight: .bold, design: .rounded)))
            default:
                Circle().fill(Look.surface2).overlay(Ico("close", size: size * 0.5).foregroundStyle(Look.text2))
            }
        }
        .frame(width: size, height: size)
        .overlay {
            if ring {
                RoundedRectangle(cornerRadius: kind == "person" ? size : size * 0.32, style: .continuous)
                    .strokeBorder(Look.accent, lineWidth: 3).padding(-5)
            }
        }
        .contentShape(Rectangle())
    }

    private var deviceIcon: String {
        if node.id == "unseen" { return "question" }
        if node.keeper.bool { return "server" }
        switch node.platform.string {
        case "android", "ios": return "phone"
        default: return "laptop"
        }
    }
}
