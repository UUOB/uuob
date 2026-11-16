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

    # In-app logo: ui/Logo.svelte, the avatar's artwork with ids unique per instance
    d, b = link("UID", "url(#UID-violet)", "url(#UID-coral)")
    art = (f'<svg viewBox="0 0 1024 1024" width={{size}} height={{size}} aria-label="UUOB" role="img" '
           f'style="border-radius:{{radius}}px">\n  <defs>{grads("UID") + d}</defs>\n'
           '  <rect width="1024" height="1024" fill="url(#UID-ink)"/>\n'
           '  <rect width="1024" height="1024" fill="url(#UID-glow)"/>\n'
           '  <g transform="translate(512 512) scale(1.1) translate(-512 -512)">\n' + b + '  </g>\n</svg>\n')
    (OUT.parent / "ui" / "Logo.svelte").write_text(
        "<!-- UUOB's logo (Link). Generated by brand/build.py: edit that, not this. -->\n"
        "<script>\n  let { size = 40, radius = Math.round(size * 0.28) } = $props();\n"
        "  const uid = $props.id();\n</script>\n\n" + art.replace("UID", "{uid}") +
        "\n<style>\n  svg { display: block; flex: none; }\n</style>\n")

    # Wordmark + lockup with the Link
    for suffix, ink in (("dark", LIGHT), ("light", INK_1)):
        wm, w = wordmark(ink, "url(#wm-coral)")
        write(f"wordmark-on-{suffix}.svg",
              svg(f'  <g transform="translate(40 40)">{wm}</g>', grads("wm"), f"0 0 {w + 80:.0f} 600"))
        wm, w = wordmark(ink, "url(#lk-coral)", x0=0)
        ld, mark = link("lk", "url(#lk-violet)", "url(#lk-coral)")
        write(f"lockup-on-{suffix}.svg",
              svg(f'  <g transform="translate(-94 -24) scale(.72)">\n{mark}  </g>\n'
                  f'  <g transform="translate(580 60)">{wm}</g>', grads("lk") + ld, f"0 0 {580 + w + 60:.0f} 640"))


if __name__ == "__main__":
    main()
