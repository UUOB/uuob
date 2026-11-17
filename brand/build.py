#!/usr/bin/env python3
"""Generates the UUOB logo concepts as SVG. Run: python3 brand/build.py"""
from pathlib import Path

OUT = Path(__file__).parent

# Palette (70 / 30 / 10): deep ink, violet, coral
INK_1, INK_2 = "#17132e", "#0d0b1c"
VIOLET_1, VIOLET_2 = "#a493ff", "#6d5cff"
CORAL_1, CORAL_2 = "#ffb15c", "#ff5c7a"
LIGHT = "#f5f5f7"


def svg(body, defs="", vb="0 0 1024 1024"):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}">\n'
            f'  <defs>{defs}</defs>\n{body}\n</svg>\n')


def grads(p):
    return (f'<linearGradient id="{p}-ink" x1="0" y1="0" x2="0" y2="1">'
            f'<stop offset="0" stop-color="{INK_1}"/><stop offset="1" stop-color="{INK_2}"/></linearGradient>'
            f'<radialGradient id="{p}-glow" cx=".5" cy=".42" r=".55">'
            f'<stop offset="0" stop-color="{VIOLET_2}" stop-opacity=".35"/><stop offset="1" stop-color="{VIOLET_2}" stop-opacity="0"/></radialGradient>'
            f'<linearGradient id="{p}-violet" x1="0" y1="0" x2="1" y2="1">'
            f'<stop offset="0" stop-color="{VIOLET_1}"/><stop offset="1" stop-color="{VIOLET_2}"/></linearGradient>'
            f'<linearGradient id="{p}-coral" x1="0" y1="0" x2="1" y2="1">'
            f'<stop offset="0" stop-color="{CORAL_1}"/><stop offset="1" stop-color="{CORAL_2}"/></linearGradient>')


def tile(p):
    return (f'  <rect x="64" y="64" width="896" height="896" rx="210" fill="url(#{p}-ink)"/>\n'
            f'  <rect x="64" y="64" width="896" height="896" rx="210" fill="url(#{p}-glow)"/>\n')


# ---------- Concept A: Cradle — a U holding your data (also reads as a person) ----------
def cradle(p, hold, orb):
    return (f'  <path d="M318 360 V548 A194 194 0 0 0 706 548 V360" fill="none" stroke="{hold}" '
            f'stroke-width="124" stroke-linecap="round"/>\n'
            f'  <circle cx="512" cy="350" r="94" fill="{orb}"/>\n')


# ---------- Concept B: Nest — U inside U around the orb ("UU●": your devices around your data) ----------
def nest(p, outer, inner, orb):
    return (f'  <path d="M236 300 V530 A276 276 0 0 0 788 530 V300" fill="none" stroke="{outer}" '
            f'stroke-width="88" stroke-linecap="round"/>\n'
            f'  <path d="M382 300 V530 A130 130 0 0 0 642 530 V300" fill="none" stroke="{inner}" '
            f'stroke-width="88" stroke-linecap="round"/>\n'
            f'  <circle cx="512" cy="530" r="50" fill="{orb}"/>\n')


# ---------- Concept C: Link — U and ∩ interlocked like chain links, tilted 45° ----------
# Drawn upright, then rotated. Stems sit 120 apart, so each link's stem passes through the other's
# middle and the two crossings land on the arcs' apexes (clean right angles).
U_PATH = "M332 260 V560 A120 120 0 0 0 572 560 V260"   # crosses ∩ at (572, 344): U on top
N_PATH = "M452 764 V464 A120 120 0 0 1 692 464 V764"   # crosses U at (452, 680): ∩ on top


def link(p, a, b, w=80, gap=26, angle=45):
    def cut(mid, path, cx, cy):
        h = w / 2 + gap + 8
        return (f'<mask id="{p}-{mid}" maskUnits="userSpaceOnUse" x="-512" y="-512" width="2048" height="2048">'
                f'<rect x="-512" y="-512" width="2048" height="2048" fill="#fff"/>'
                f'<path d="{path}" fill="none" stroke="#000" stroke-width="{w + 2 * gap}" clip-path="url(#{p}-{mid}c)"/></mask>'
                f'<clipPath id="{p}-{mid}c"><rect x="{cx - h}" y="{cy - h}" width="{2 * h}" height="{2 * h}"/></clipPath>')
    defs = cut("mu", N_PATH, 452, 680) + cut("mn", U_PATH, 572, 344)
    body = (f'  <g transform="translate(512 512) rotate({angle}) scale(.9) translate(-512 -512)">\n'
            f'    <path d="{U_PATH}" fill="none" stroke="{a}" stroke-width="{w}" stroke-linecap="round" mask="url(#{p}-mu)"/>\n'
            f'    <path d="{N_PATH}" fill="none" stroke="{b}" stroke-width="{w}" stroke-linecap="round" mask="url(#{p}-mn)"/>\n'
            f'  </g>\n')
    return defs, body


# ---------- Wordmark: u u ● b, drawn from strokes ----------
def wordmark(ink, orb, x0=0, base=520, sw=64):
    xh = 300                        # x-height (outer)
    r = (xh - sw) / 2               # centreline radius
    top, bot = base - xh + sw / 2, base - sw / 2
    adv = 2 * r + sw + 56           # letter advance
    parts = []
    for i in range(2):              # u u
        x = x0 + sw / 2 + i * adv
        parts.append(f'M{x:.0f} {top:.0f} V{bot - r:.0f} A{r:.0f} {r:.0f} 0 0 0 {x + 2*r:.0f} {bot - r:.0f} '
                     f'M{x + 2*r:.0f} {top:.0f} V{bot:.0f}')
    ox = x0 + 2 * adv + sw / 2 + r  # o centre
    oy = base - xh / 2
    bx = x0 + sw / 2 + 3 * adv      # b stem
    parts.append(f'M{bx:.0f} {top - 170:.0f} V{bot:.0f}')
    ring = f'<circle cx="{bx + r:.0f}" cy="{oy:.0f}" r="{r:.0f}" fill="none" stroke="{ink}" stroke-width="{sw}"/>'
    width = bx + 2 * r + sw / 2 - x0
    body = (f'<path d="{" ".join(parts)}" fill="none" stroke="{ink}" stroke-width="{sw}" stroke-linecap="round"/>'
            f'{ring}<circle cx="{ox:.0f}" cy="{oy:.0f}" r="{xh/2:.0f}" fill="{orb}"/>')
    return body, width


# ---------- Concept D: Grid — U U over O B in a 2×2 grid ----------
def grid_letter(ch, x, y, c, w):
    """Path for one uppercase letter drawn as a stroke of width w inside the c×c cell at (x, y)."""
    xl, xr, yt, yb = x + w / 2, x + c - w / 2, y + w / 2, y + c - w / 2
    R = (xr - xl) / 2
    if ch == "U":
        return f"M{xl:.1f} {yt:.1f} V{yb - R:.1f} A{R:.1f} {R:.1f} 0 0 0 {xr:.1f} {yb - R:.1f} V{yt:.1f}"
    if ch == "B":
        ym = yt + (yb - yt) * 0.47
        ra, rb = (ym - yt) / 2, (yb - ym) / 2
        xa, xb = xr - ra - (rb - ra) * 2 - 6, xr - rb
        return (f"M{xl:.1f} {yt:.1f} V{yb:.1f} H{xb:.1f} A{rb:.1f} {rb:.1f} 0 0 0 {xb:.1f} {ym:.1f} H{xl:.1f} "
                f"M{xl:.1f} {ym:.1f} H{xa:.1f} A{ra:.1f} {ra:.1f} 0 0 0 {xa:.1f} {yt:.1f} H{xl:.1f}")
    raise ValueError(ch)


def grid(colors, w=56, gap=48, area=560, orb="disc", tiles=None, tile_r=60, pad=0, b_w=None):
    """colors: fill/stroke for U, U, O, B. tiles: optional tile fills for the four cells."""
    c = (area - gap) / 2
    x0 = (1024 - area) / 2
    cells = [(x0, x0), (x0 + c + gap, x0), (x0, x0 + c + gap), (x0 + c + gap, x0 + c + gap)]
    out = []
    for i, ((x, y), ch, col) in enumerate(zip(cells, "UUOB", colors)):
        if tiles:
            out.append(f'  <rect x="{x:.1f}" y="{y:.1f}" width="{c:.1f}" height="{c:.1f}" rx="{tile_r}" fill="{tiles[i]}"/>')
        lx, ly, lc = x + pad, y + pad, c - 2 * pad
        if ch == "O":
            if orb == "disc":
                out.append(f'  <circle cx="{lx + lc/2:.1f}" cy="{ly + lc/2:.1f}" r="{lc/2:.1f}" fill="{col}"/>')
            else:
                out.append(f'  <circle cx="{lx + lc/2:.1f}" cy="{ly + lc/2:.1f}" r="{(lc - w)/2:.1f}" fill="none" '
                           f'stroke="{col}" stroke-width="{w}"/>')
        else:
            sw = b_w if ch == "B" and b_w else w
            out.append(f'  <path d="{grid_letter(ch, lx, ly, lc, sw)}" fill="none" stroke="{col}" stroke-width="{sw}" '
                       f'stroke-linecap="round" stroke-linejoin="round"/>')
    return "\n".join(out) + "\n"


# ---------- Concept E: Network — your computer, phone and server, linked directly ----------
import math

NODES = {"laptop": (512, 296), "phone": (284, 716), "server": (740, 716)}
CENTRE = (512, 576)


def device(kind, cx, cy, col, w=26, k=1.0):
    """A device glyph drawn as strokes, about 200 px wide at k=1, centred on (cx, cy)."""
    def r(x, y, ww, hh, rx):
        return (f'<rect x="{cx + x*k:.1f}" y="{cy + y*k:.1f}" width="{ww*k:.1f}" height="{hh*k:.1f}" rx="{rx*k:.1f}" '
                f'fill="none" stroke="{col}" stroke-width="{w}"/>')
    def l(x1, y1, x2, y2):
        return (f'<path d="M{cx + x1*k:.1f} {cy + y1*k:.1f} L{cx + x2*k:.1f} {cy + y2*k:.1f}" stroke="{col}" '
                f'stroke-width="{w}" stroke-linecap="round"/>')
    def dot(x, y, rr):
        return f'<circle cx="{cx + x*k:.1f}" cy="{cy + y*k:.1f}" r="{rr*k:.1f}" fill="{col}"/>'
    if kind == "laptop":
        return r(-72, -62, 144, 96, 14) + l(-98, 58, 98, 58)
    if kind == "phone":
        return r(-44, -80, 88, 160, 20) + dot(0, 46, 11)
    if kind == "server":
        return r(-76, -72, 152, 62, 14) + r(-76, 10, 152, 62, 14) + dot(42, -41, 9) + dot(42, 41, 9)
    raise ValueError(kind)


def trimmed(a, b, ta, tb):
    (x1, y1), (x2, y2) = a, b
    d = math.hypot(x2 - x1, y2 - y1); ux, uy = (x2 - x1) / d, (y2 - y1) / d
    return x1 + ux * ta, y1 + uy * ta, x2 - ux * tb, y2 - uy * tb


def network(take, ink, line, orb, disc=None, size=1.0, spread=1.0):
    """size scales the device icons; spread pushes the devices away from the centre."""
    n = {k: (CENTRE[0] + (x - CENTRE[0]) * spread, CENTRE[1] + (y - CENTRE[1]) * spread) for k, (x, y) in NODES.items()}
    trim = {"laptop": 122 * size, "phone": 118 * size, "server": 122 * size}
    pairs = [("laptop", "phone"), ("laptop", "server"), ("phone", "server")]
    out = []
    if take == "mesh":            # devices linked to each other, data (coral) on every link
        for a, b in pairs:
            x1, y1, x2, y2 = trimmed(n[a], n[b], trim[a], trim[b])
            out.append(f'<path d="M{x1:.1f} {y1:.1f} L{x2:.1f} {y2:.1f}" stroke="{line}" stroke-width="22" stroke-linecap="round"/>')
            out.append(f'<circle cx="{(x1+x2)/2:.1f}" cy="{(y1+y2)/2:.1f}" r="34" fill="{orb}"/>')
        for k, (x, y) in n.items():
            out.append(device(k, x, y, ink, w=26 * min(size, 1.15), k=size))
    elif take == "hub":           # devices in discs around your data
        c = CENTRE
        for k in n:
            x1, y1, x2, y2 = trimmed(n[k], c, 100, 60)
            out.append(f'<path d="M{x1:.1f} {y1:.1f} L{x2:.1f} {y2:.1f}" stroke="{line}" stroke-width="22" stroke-linecap="round"/>')
        out.append(f'<circle cx="{c[0]}" cy="{c[1]}" r="64" fill="{orb}"/>')
        for k, (x, y) in n.items():
            out.append(f'<circle cx="{x}" cy="{y}" r="114" fill="{disc}"/>')
            out.append(device(k, x, y, ink, w=22, k=.72))
    elif take == "ring":          # devices sitting on one ring: one network
        cx, cy = CENTRE
        R = math.hypot(n["laptop"][0] - cx, n["laptop"][1] - cy)
        out.append(f'<circle cx="{cx}" cy="{cy}" r="{R:.1f}" fill="none" stroke="{line}" stroke-width="22"/>')
        out.append(f'<circle cx="{cx}" cy="{cy}" r="58" fill="{orb}"/>')
        for k, (x, y) in n.items():
            out.append(f'<circle cx="{x}" cy="{y}" r="122" fill="{disc}"/>')
            out.append(device(k, x, y, ink, w=24, k=.86))
    return "\n".join("  " + o for o in out) + "\n"


def write(name, text):
    (OUT / name).write_text(text)


def main():
    # A — Cradle
    write("a-cradle-icon.svg", svg(tile("ai") + cradle("ai", f"url(#ai-violet)", f"url(#ai-coral)"), grads("ai")))
    write("a-cradle-mark.svg", svg(cradle("am", f"url(#am-violet)", f"url(#am-coral)"), grads("am")))
    write("a-cradle-mono.svg", svg(cradle("ao", "#000", "#000")))

    # B — Nest
    write("b-nest-icon.svg", svg(tile("bi") + nest("bi", "url(#bi-violet)", LIGHT, "url(#bi-coral)"), grads("bi")))
    write("b-nest-mark.svg", svg(nest("bm", "url(#bm-violet)", "url(#bm-violet)", "url(#bm-coral)"), grads("bm")))
    write("b-nest-mono.svg", svg(nest("bo", "#000", "#000", "#000")))

    # C — Link
    d, b = link("ci", "url(#ci-violet)", "url(#ci-coral)")
    write("c-link-icon.svg", svg(tile("ci") + b, grads("ci") + d))
    d, b = link("cm", "url(#cm-violet)", "url(#cm-coral)")
    write("c-link-mark.svg", svg(b, grads("cm") + d))
    d, b = link("co", "#000", "#000")
    write("c-link-mono.svg", svg(b, d))
    # Android: round launcher icon, and the adaptive-icon foreground (mark inside the central 66%)
    d, b = link("cr", "url(#cr-violet)", "url(#cr-coral)")
    write("c-link-round.svg", svg('  <circle cx="512" cy="512" r="512" fill="url(#cr-ink)"/>\n'
                                  '  <circle cx="512" cy="512" r="512" fill="url(#cr-glow)"/>\n'
                                  '  <g transform="translate(512 512) scale(1.1) translate(-512 -512)">\n' + b + '  </g>\n',
                                  grads("cr") + d))
    d, b = link("cf", "url(#cf-violet)", "url(#cf-coral)")
    write("c-link-foreground.svg", svg('  <g transform="translate(512 512) scale(.8) translate(-512 -512)">\n' + b + '  </g>\n',
                                       grads("cf") + d))

    # Avatar (GitHub organization, social): full-bleed square, the site rounds or crops it
    d, b = link("ca", "url(#ca-violet)", "url(#ca-coral)")
    write("c-link-avatar.svg", svg('  <rect width="1024" height="1024" fill="url(#ca-ink)"/>\n'
                                   '  <rect width="1024" height="1024" fill="url(#ca-glow)"/>\n'
                                   '  <g transform="translate(512 512) scale(1.1) translate(-512 -512)">\n' + b + '  </g>\n',
                                   grads("ca") + d))

    # D — Grid (three takes)
    V, C = "url(#{p}-violet)", "url(#{p}-coral)"
    def g(p, **kw):
        cols = [x.format(p=p) for x in kw.pop("cols")]
        if "tiles" in kw:
            kw["tiles"] = [x.format(p=p) for x in kw["tiles"]]
        return grid(cols, **kw)
    takes = {
        "d1-grid": dict(cols=[LIGHT, LIGHT, C, LIGHT]),
        "d2-tiles": dict(cols=[INK_1, INK_1, INK_1, INK_1], tiles=[V, V, C, LIGHT], w=40, pad=50, gap=28, area=600,
                         orb="ring"),
        "d3-bold": dict(cols=[V, V, C, V], w=84, gap=28, area=600, b_w=66),
    }
    for name, kw in takes.items():
        write(f"{name}-icon.svg", svg(tile("di") + g("di", **dict(kw)), grads("di")))
        mono = dict(kw, cols=["#000"] * 4)
        if "tiles" in mono:
            mono["tiles"] = ["#000"] * 4
            mono["cols"] = ["#fff"] * 4
        write(f"{name}-mono.svg", svg(g("do", **mono)))

    # E — Network (three takes)
    for name, take, disc in (("e1-mesh", "mesh", None), ("e2-hub", "hub", "url(#ei-violet)"), ("e3-ring", "ring", "url(#ei-ink)")):
        write(f"{name}-icon.svg", svg(tile("ei") + network(take, LIGHT, "#8b7bff", "url(#ei-coral)", disc), grads("ei")))
        mono_ink = "#fff" if take == "hub" else "#000"
        mono_disc = "#000" if take == "hub" else "#fff"
        write(f"{name}-mono.svg", svg(network(take, mono_ink, "#000", "#000", mono_disc)))

    # E in black and white: white on black, black on white
    for name, take in (("e1-mesh", "mesh"), ("e2-hub", "hub"), ("e3-ring", "ring")):
        for mode, bg, fg in (("dark", "#000", "#fff"), ("light", "#fff", "#000")):
            ink, disc = (bg, fg) if take == "hub" else (fg, bg)
            write(f"{name}-bw-{mode}.svg",
                  svg(f'  <rect x="64" y="64" width="896" height="896" rx="210" fill="{bg}"/>\n'
                      + network(take, ink, fg, fg, disc)))

    # E1 Mesh, dark, with large device icons
    write("e1-mesh-large-dark.svg",
          svg('  <rect x="64" y="64" width="896" height="896" rx="210" fill="#000"/>\n'
              + network("mesh", "#fff", "#fff", "#fff", size=1.4, spread=1.06)))

    # ===== THE LOGO: Mesh, white on black. export.sh renders the app icons from the logo-*.svg files. =====
    art = network("mesh", "#fff", "#fff", "#fff", size=1.4, spread=1.06)
    def scaled(k, body):
        return f'  <g transform="translate(512 512) scale({k}) translate(-512 -512)">\n{body}  </g>\n'
    write("logo-icon.svg", svg('  <rect x="64" y="64" width="896" height="896" rx="210" fill="#000"/>\n' + art))
    write("logo-round.svg", svg('  <circle cx="512" cy="512" r="512" fill="#000"/>\n' + scaled(.92, art)))
    write("logo-foreground.svg", svg(scaled(.72, art)))           # Android adaptive icon: central 66%
    write("logo-avatar.svg", svg('  <rect width="1024" height="1024" fill="#000"/>\n' + scaled(1.08, art)))
    write("logo-mono.svg", svg(network("mesh", "#000", "#000", "#000", size=1.4, spread=1.06)))
    (OUT.parent / "ui" / "Logo.svelte").write_text(
        "<!-- UUOB's logo (Mesh). Generated by brand/build.py: edit that, not this. -->\n"
        "<script>\n  let { size = 40, radius = Math.round(size * 0.28) } = $props();\n</script>\n\n"
        '<svg viewBox="0 0 1024 1024" width={size} height={size} aria-label="UUOB" role="img" '
        'style="border-radius:{radius}px">\n  <rect width="1024" height="1024" fill="#000"/>\n'
        + scaled(1.08, art) + "</svg>\n\n<style>\n  svg { display: block; flex: none; }\n</style>\n")

    # Wordmark and lockup, black and white
    for suffix, ink in (("dark", "#fff"), ("light", "#000")):
        wm, w = wordmark(ink, ink)
        write(f"wordmark-on-{suffix}.svg", svg(f'  <g transform="translate(40 40)">{wm}</g>', "", f"0 0 {w + 80:.0f} 600"))
        mark = network("mesh", ink, ink, ink, size=1.4, spread=1.06)
        write(f"lockup-on-{suffix}.svg",
              svg(f'  <g transform="translate(-72 -20) scale(.7)">\n{mark}  </g>\n'
                  f'  <g transform="translate(680 60)">{wm}</g>', "", f"0 0 {680 + w + 60:.0f} 640"))

if __name__ == "__main__":
    main()
