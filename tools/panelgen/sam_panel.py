"""S-Bank panel generator — the Sam-e visual system, as code.  (v0.4)

A panel is declared ONCE (HP + a list of components: kind, mm position, label, and
the C++ widget enum id). From that single spec this emits two artifacts that can
never drift apart:

  * ``res/<Module>.svg``        — the brand-styled panel art (labels at the exact
                                  component positions).
  * ``src/<Module>_panel.inc``  — a C++ fragment of ``addParam/addInput/...`` calls
                                  the module's Widget constructor ``#include``s.

WHAT CHANGED IN v0.4 — this is no longer the generic purple instrument. The kit now
renders the actual Sam-e system:

  * Environment = jet black (#0B080B), engraving = NASA white. No more lavender.
  * COLOR IS STATE — accents are assigned by what a component DOES, never decoration:
      orange  = signal / active    -> OUT jacks, S- mark, status LED
      yellow  = energy / the strike -> openness LED + ringing body, HIT trigger
      cyan    = information / control -> CV trims & inputs (tied as pairs)
      e.blue  = depth / unknown     -> the IMPERFECTION switch
  * Instrument language: engraved 270deg gauges round every knob, recessed wells,
    registration crosshairs, a mirror axis between channels, bracketed/boxed bays,
    masthead (S-BANK . title . HP/serial) and footer telemetry (serial . SAM-E . LED).
  * A ``style`` per panel: "mk1" (instrument), "mk2" (telemetry), "mk3" (signal trace).

Everything is still PATHS-ONLY (nanosvg ignores <text>): labels come from strokefont,
all geometry is rect / line / circle / path. No filters, no embedded fonts, no <text>.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from pathlib import Path

from samefont import text_paths, text_width  # real filled brand fonts (Fira Code / Space Grotesk)

# --- brand kit -------------------------------------------------------------------
HP_MM = 5.08
PANEL_H = 128.5

INK = "#0B080B"      # jet black   — the environment / silence / space
PAPER = "#F5F5F1"    # NASA white  — structure
GRAY = "#A6A6A6"     # mission gray — secondary type
ENGRAVE = "#ECE7DD"  # lettering (warm white, reads engraved on black)
ORANGE = "#FF5A00"   # signal / active / transmitting
YELLOW = "#FFC400"   # energy / peak / the strike
CYAN = "#19D2E5"     # information / data / flow
EBLUE = "#1C46FF"    # depth / electricity / unknown
HAIR = "#F5F5F1"     # structure lines, drawn at low opacity

DISPLAY = "Space Grotesk, Helvetica, Arial, sans-serif"  # reference only; type is outlined
MONO = "Fira Code, 'IBM Plex Mono', monospace"

# Finishes. The environment flips and the engraving flips; signal-state accents stay
# in-family but deepen on light aluminium so they still read (voltage, not a rainbow).
THEMES = {
    "black": {
        "bg": ("#100b12", "#0b070b", "#08060a"),
        "ink": "#ECE7DD", "hair": "#F5F5F1", "gray": "#A6A6A6",
        "well": "#14101a", "wellop": 0.9, "wellstroke": 0.16,
        "orange": "#FF5A00", "yellow": "#FFC400", "cyan": "#19D2E5", "eblue": "#1C46FF",
    },
    "silver": {
        "bg": ("#e2e3e6", "#d2d3d7", "#bfc0c5"),
        "ink": "#17141c", "hair": "#2b2733", "gray": "#6c6d72",
        "well": "#b4b5ba", "wellop": 0.9, "wellstroke": 0.5,
        "orange": "#E8500A", "yellow": "#A87400", "cyan": "#0E93A6", "eblue": "#1C46FF",
    },
}

# Component kind -> (Rack widget class, add-call, footprint_radius_mm).
# Footprint radii match what Rack actually draws on top, so labels clear the part.
_PARAM, _INPUT, _OUTPUT = "addParam", "addInput", "addOutput"
_KINDS = {
    "knob":    ("createParamCentered<RoundBlackKnob>",      _PARAM,  6.4),
    "knob_sm": ("createParamCentered<RoundSmallBlackKnob>", _PARAM,  4.6),
    "trim":    ("createParamCentered<Trimpot>",             _PARAM,  2.6),
    "switch2": ("createParamCentered<CKSS>",                _PARAM,  2.6),
    "switch3": ("createParamCentered<CKSSThree>",           _PARAM,  2.6),
    "in":      ("createInputCentered<PJ301MPort>",          _INPUT,  3.15),
    "out":     ("createOutputCentered<PJ301MPort>",         _OUTPUT, 3.15),
}
_SWITCH_HALF_H = 2.7  # half the CKSS body height (5.4mm tall); label clears it


# --- color = state ---------------------------------------------------------------
def accent_for(c: "_Comp", t: dict) -> str:
    """The Sam-e rule: a component's color is its signal role, never decoration."""
    if c.accent:                       # explicit override wins
        return c.accent
    if c.kind == "out":
        return t["orange"]             # signal leaving, active
    if c.kind == "light":
        return t["yellow"]             # energy / the strike
    if c.kind == "trim":
        return t["cyan"]               # control / information
    if c.kind in ("switch2", "switch3"):
        return t["eblue"]              # depth / the unknown
    if c.kind == "in":
        if "HIT" in c.label.upper():
            return t["yellow"]         # the trigger that fires the strike
        if any(k in c.label.upper() for k in ("DEC", "CTL", "CV")):
            return t["cyan"]           # CV in
    return t["ink"]                    # everything else = environment


@dataclass
class _Comp:
    kind: str
    x: float
    y: float
    eid: str                      # C++ enum id, e.g. "A_OPEN_PARAM"
    label: str = ""
    accent: str | None = None     # label-colour override (else color = state)
    lo: str = ""                  # scale-end descriptor at the dial min (lower-left)
    hi: str = ""                  # scale-end descriptor at the dial max (lower-right)
    light_tpl: str = "MediumLight<YellowLight>"


@dataclass
class Style:
    """How loud the system speaks on a given panel."""
    zones: str = "brackets"   # "brackets" | "boxes" | "none"
    rail: bool = True         # mirror axis between two channel columns
    ruler: bool = False       # edge measurement ticks
    envelope: bool = False    # draw the module's own decay curve under the masthead
    trace: bool = False       # draw the audio signal path through each channel
    watermark: bool = False   # faint giant S- behind the layout
    gauges_thin: bool = False
    ping: float = 1.0         # scale of the ringing-body rings on the openness LED


STYLES: dict[str, Style] = {
    "mk1": Style(zones="brackets", rail=True, ping=0.85),
    "mk2": Style(zones="boxes", rail=True, ruler=True, envelope=True, ping=0.72),
    "mk3": Style(zones="none", rail=False, trace=True, watermark=True,
                 gauges_thin=True, ping=0.85),
}


@dataclass
class Panel:
    module: str            # C++ module struct name, used to qualify enum ids
    title: str             # panel title (display)
    hp: int = 8
    serial: str = "001"
    style: str = "mk1"
    finish: str = "black"  # "black" (jet) | "silver" (brushed aluminium)
    sub: str = ""          # right-of-masthead caption; split halves on "|"
    comps: list[_Comp] = field(default_factory=list)
    notes: list[tuple[float, float, str, str, str, str]] = field(default_factory=list)
    dividers: list[float] = field(default_factory=list)

    @property
    def w(self) -> float:
        return self.hp * HP_MM

    # --- placement API (records a component + its label) -------------------------
    def _add(self, kind, x, y, eid, label="", accent=None, light_tpl=None, lo="", hi=""):
        c = _Comp(kind, x, y, eid, label, accent, lo, hi)
        if light_tpl:
            c.light_tpl = light_tpl
        self.comps.append(c)

    def knob(self, x, y, eid, label="", small=False, lo="", hi=""):
        self._add("knob_sm" if small else "knob", x, y, eid, label, lo=lo, hi=hi)

    def trim(self, x, y, eid, label=""):
        self._add("trim", x, y, eid, label)

    def switch(self, x, y, eid, label="", three=False):
        self._add("switch3" if three else "switch2", x, y, eid, label)

    def input(self, x, y, eid, label=""):
        self._add("in", x, y, eid, label)

    def output(self, x, y, eid, label=""):
        self._add("out", x, y, eid, label)

    def light(self, x, y, eid, tpl="MediumLight<YellowLight>"):
        self._add("light", x, y, eid, light_tpl=tpl)

    def note(self, x, y, text, cls="sm", color="gray", anchor="middle"):
        self.notes.append((x, y, text, cls, color, anchor))

    def divider(self, y):
        self.dividers.append(y)

    # --- tiny svg helpers --------------------------------------------------------
    @staticmethod
    def _line(x1, y1, x2, y2, col, w=0.25, op=1.0):
        return (f'<line x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}" '
                f'stroke="{col}" stroke-width="{w}" stroke-opacity="{op}" '
                f'stroke-linecap="round"/>')

    @staticmethod
    def _circle(cx, cy, r, fill="none", fill_op=1.0, stroke="none", w=0.25, op=1.0):
        return (f'<circle cx="{cx:.2f}" cy="{cy:.2f}" r="{r:.2f}" fill="{fill}" '
                f'fill-opacity="{fill_op}" stroke="{stroke}" stroke-width="{w}" '
                f'stroke-opacity="{op}"/>')

    @staticmethod
    def _rrect(x, y, w, h, r, fill="none", fill_op=1.0, stroke="none", sw=0.25, op=1.0):
        return (f'<rect x="{x:.2f}" y="{y:.2f}" width="{w:.2f}" height="{h:.2f}" '
                f'rx="{r:.2f}" fill="{fill}" fill-opacity="{fill_op}" stroke="{stroke}" '
                f'stroke-width="{sw}" stroke-opacity="{op}"/>')

    @staticmethod
    def _polar(cx, cy, r, deg):
        a = math.radians(deg)
        return cx + r * math.cos(a), cy + r * math.sin(a)

    def _arc(self, cx, cy, r, a0, a1, large, sweep, col, w=0.25, op=1.0):
        x0, y0 = self._polar(cx, cy, r, a0)
        x1, y1 = self._polar(cx, cy, r, a1)
        return (f'<path d="M{x0:.2f},{y0:.2f} A{r:.2f},{r:.2f} 0 {large} {sweep} '
                f'{x1:.2f},{y1:.2f}" fill="none" stroke="{col}" stroke-width="{w}" '
                f'stroke-opacity="{op}" stroke-linecap="round"/>')

    # --- composite motifs --------------------------------------------------------
    def _gauge(self, cx, cy, r, op, thin, caps=True):
        """270deg instrument dial: guard ring + graduated ticks + top index + min/max caps."""
        hair = self._t["hair"]
        a0, sweep = 135, 270
        out = [self._arc(cx, cy, r + 1.0, a0, 45, 1, 1, hair, 0.16, op * 0.5),
               self._arc(cx, cy, r, a0, 45, 1, 1, hair, 0.18 if thin else 0.22, op)]
        n = 10
        for i in range(n + 1):
            a = a0 + (i / n) * sweep
            major = (i % 5 == 0)
            ri, ro = r - (1.4 if major else 0.7), r + 0.12
            x0, y0 = self._polar(cx, cy, ri, a)
            x1, y1 = self._polar(cx, cy, ro, a)
            out.append(self._line(x0, y0, x1, y1, hair, 0.26 if major else 0.15,
                                   op + (0.18 if major else 0.04)))
        ti = self._polar(cx, cy, r - 2.1, 270)   # top index pointer (12 o'clock)
        tb = self._polar(cx, cy, r + 0.7, 270)
        out.append(self._line(ti[0], ti[1], tb[0], tb[1], hair, 0.3, op + 0.32))
        if caps:                                  # min/max caps — skipped when lo/hi text marks the ends
            for ea in (a0, 45):
                ex, ey = self._polar(cx, cy, r, ea)
                out.append(self._circle(ex, ey, 0.45, fill=hair, fill_op=op + 0.25))
        return "".join(out)

    def _well(self, cx, cy, r):
        t = self._t
        return self._circle(cx, cy, r, fill=t["well"], fill_op=t["wellop"],
                            stroke=t["hair"], w=0.2, op=t["wellstroke"])

    def _ping(self, cx, cy, scale):
        """Concentric pulses — the struck resonant body ringing out."""
        yellow = self._t["yellow"]
        out = []
        for rr, op in ((2.3, 0.5), (3.9, 0.3), (5.3, 0.17)):
            out.append(self._circle(cx, cy, rr * scale, stroke=yellow, w=0.22, op=op))
        out.append(self._circle(cx, cy, 1.25, fill=yellow, fill_op=0.85))
        return "".join(out)

    def _brackets(self, x0, y0, x1, y1, op, length=2.4):
        hair = self._t["hair"]
        out = []
        for bx, by, sx, sy in ((x0, y0, 1, 1), (x1, y0, -1, 1), (x0, y1, 1, -1), (x1, y1, -1, -1)):
            out.append(self._line(bx, by, bx + length * sx, by, hair, 0.22, op))
            out.append(self._line(bx, by, bx, by + length * sy, hair, 0.22, op))
        return "".join(out)

    def _envelope(self, x, y, w, h, col, op):
        pk = x + w * 0.16
        d = (f'M{x:.2f},{y + h:.2f} L{pk:.2f},{y:.2f} '
             f'C{pk + w * 0.18:.2f},{y + h * 0.15:.2f} {pk + w * 0.34:.2f},{y + h * 0.92:.2f} '
             f'{x + w:.2f},{y + h:.2f}')
        return (f'<path d="{d}" fill="none" stroke="{col}" stroke-width="0.3" '
                f'stroke-opacity="{op}" stroke-linecap="round" stroke-linejoin="round"/>'
                + self._line(x, y + h, x + w, y + h, col, 0.18, op * 0.6))

    # --- SVG emit ----------------------------------------------------------------
    def svg(self) -> str:
        S = STYLES.get(self.style, STYLES["mk1"])
        t = THEMES.get(self.finish, THEMES["black"])
        self._t = t                       # helpers read the active finish from here
        ink, hair, gray = t["ink"], t["hair"], t["gray"]
        orange, yellow, cyan, eblue = t["orange"], t["yellow"], t["cyan"], t["eblue"]
        w, H, mid = self.w, PANEL_H, self.w / 2
        mx = 5 if w < 50 else 7          # side margin (body)
        smx = 11.5                       # corner-row margin: clears the Rack screws
        narrow = w < 50                  # simplify masthead/footer on slim panels
        o: list[str] = ['<?xml version="1.0" encoding="UTF-8"?>',
                        f'<!-- Generated by tools/panelgen (Sam-e system v0.4). '
                        f'Edit the spec, not this file. -->',
                        f'<svg xmlns="http://www.w3.org/2000/svg" version="1.1" '
                        f'width="{w:.2f}mm" height="{H}mm" viewBox="0 0 {w:.2f} {H}">']
        o.append(f'<defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="{H}" '
                 f'gradientUnits="userSpaceOnUse">'
                 f'<stop offset="0" stop-color="{t["bg"][0]}"/>'
                 f'<stop offset="0.5" stop-color="{t["bg"][1]}"/>'
                 f'<stop offset="1" stop-color="{t["bg"][2]}"/></linearGradient></defs>')

        # 1. environment (full-bleed: no rounded border — reads weird against the
        # rails; the corners carry real Rack screws instead of drawn crosshairs)
        o.append(f'<rect x="0" y="0" width="{w:.2f}" height="{H}" fill="url(#bg)"/>')

        # channel columns (knob x positions)
        cols = sorted({c.x for c in self.comps if c.kind == "knob"})

        # 2. structure
        if S.ruler:
            for yy in range(16, 121, 5):
                major = yy % 10 == 0
                o.append(self._line(2.0, yy, 2.0 + (1.8 if major else 1.0), yy, hair, 0.18, 0.18))
                o.append(self._line(w - 2.0, yy, w - 2.0 - (1.8 if major else 1.0), yy, hair, 0.18, 0.18))
        if S.rail and len(cols) == 2:
            o.append(self._line(mid, 15.5, mid, 102, hair, 0.18, 0.16))
            for ry in range(20, 101, 10):
                o.append(self._line(mid - 0.7, ry, mid + 0.7, ry, hair, 0.18, 0.16))
            dy = 58
            o.append(f'<path d="M{mid:.2f},{dy - 1.1:.2f} L{mid + 1.1:.2f},{dy:.2f} '
                     f'L{mid:.2f},{dy + 1.1:.2f} L{mid - 1.1:.2f},{dy:.2f} Z" fill="none" '
                     f'stroke="{hair}" stroke-width="0.18" stroke-opacity="0.22"/>')
        if S.zones == "boxes":
            for cx in cols:
                o.append(self._rrect(cx - 11, 15.5, 22, 105.5, 1.4, stroke=hair, sw=0.2, op=0.13))
        elif S.zones == "brackets":
            for cx in cols:
                o.append(self._brackets(cx - 11, 15.5, cx + 11, 121, 0.3))

        # 3. masthead (width-aware) — wordmark + title in the display face. Corner
        # text uses smx so it clears the mounting screws.
        o.append(text_paths("S-", smx, 5.4, 1.9 if narrow else 2.1, orange, "start", display=True))
        o.append(text_paths("BANK", smx + text_width("S-", 1.9 if narrow else 2.1, display=True) + 0.6, 5.4,
                            1.9 if narrow else 2.1, ink, "start", display=True))
        o.append(text_paths(self.title, smx, 10.6, 3.4 if narrow else 3.6, ink, "start", display=True))
        if narrow:
            o.append(text_paths(f"{self.hp}HP", w - smx, 5.4, 1.4, gray, "end", weight=0.18))
            o.append(text_paths(f"S-{self.serial}", w - smx, 10.6, 1.3, gray, "end", weight=0.16))
        else:
            o.append(text_paths(f"{self.hp}HP / S-{self.serial}", w - smx, 5.4, 1.5, gray, "end", weight=0.18))
            if self.sub:
                halves = [s.strip() for s in self.sub.split("|")]
                o.append(text_paths(halves[0], w - smx, 8.0, 1.4, gray, "end", weight=0.16))
                if len(halves) > 1:
                    o.append(text_paths(halves[1], w - smx, 10.4, 1.4, gray, "end", weight=0.16))
        o.append(self._line(mx, 12.2, w - mx, 12.2, hair, 0.25, 0.34))
        if S.envelope:
            o.append(self._envelope(mid - 7, 14.2, 14, 3.2, cyan, 0.5))

        # user dividers
        for y in self.dividers:
            o.append(self._line(mx, y, w - mx, y, hair, 0.22, 0.2))

        # 4. signal trace (mk3): inputs converge up into the gate, signal flows down to OUT
        if S.trace and cols:
            for cx in cols:
                o.append(self._line(cx, 84, cx, 115.5, ink, 0.22, 0.34))     # gate -> OUT spine
                o.append(self._line(cx - 9, 93, cx, 85.5, ink, 0.2, 0.34))   # IN feeds the gate (left)
                o.append(self._line(cx + 9, 93, cx, 85.5, yellow, 0.22, 0.44))   # HIT fires the gate (right)
                o.append(f'<path d="M{cx - 1.0:.2f},114.0 L{cx:.2f},116.0 L{cx + 1.0:.2f},114.0" '
                         f'fill="none" stroke="{orange}" stroke-width="0.3" '
                         f'stroke-linecap="round" stroke-linejoin="round"/>')    # signal leaves
        if S.watermark:
            wm = text_paths("S-", mid, 78, 26, ink, "middle", display=True)
            o.append(wm.replace("/>", ' fill-opacity="0.05"/>'))

        # 5. CV grouping ties (cyan) for DEC/CTL trim pairs
        trims = [c for c in self.comps if c.kind == "trim"]
        for cx in cols:
            pair = [tt for tt in trims if abs(tt.y - 73) < 1 and abs(tt.x - cx) <= 9]
            if len(pair) == 2:
                o.append(self._line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, cyan, 0.18, 0.32))
                o.append(text_paths("CV", cx, 69.4, 1.4, cyan, "middle", weight=0.16))

        # 6. per-component furniture: wells, gauges, ping, accent rings
        for c in self.comps:
            acc = accent_for(c, t)
            if c.kind in ("knob", "knob_sm"):
                fr = _KINDS[c.kind][2]
                o.append(self._well(c.x, c.y, fr - 0.6))
                o.append(self._gauge(c.x, c.y, fr + 0.9, 0.28, S.gauges_thin, caps=not (c.lo or c.hi)))
                if c.lo or c.hi:                 # scale-end descriptors just outside the arc ends
                    capr = (fr + 0.9) * 0.707
                    ex, ey = capr + 0.4, c.y + capr + 1.2
                    if c.lo:
                        o.append(text_paths(c.lo, c.x - ex, ey, 1.1, gray, "end", weight=0.15))
                    if c.hi:
                        o.append(text_paths(c.hi, c.x + ex, ey, 1.1, gray, "start", weight=0.15))
            elif c.kind == "trim":
                o.append(self._well(c.x, c.y, _KINDS[c.kind][2] + 0.5))
            elif c.kind in ("in", "out"):
                fr = _KINDS[c.kind][2]
                o.append(self._well(c.x, c.y, fr + 0.5))
                if c.kind == "out":
                    o.append(self._circle(c.x, c.y, fr + 0.75, stroke=orange, w=0.28, op=0.55))
                elif acc != ink:
                    o.append(self._circle(c.x, c.y, fr + 0.7, stroke=acc, w=0.22, op=0.4))
            elif c.kind == "light":
                o.append(self._ping(c.x, c.y, S.ping))
            elif c.kind in ("switch2", "switch3"):
                o.append(self._rrect(c.x - 1.3, c.y - _SWITCH_HALF_H, 2.6, _SWITCH_HALF_H * 2, 0.8,
                                     fill=t["well"], fill_op=t["wellop"], stroke=eblue, sw=0.22, op=0.5))
                for dx, dy, op in ((-3.2, -1.0, 0.7), (-3.7, 0.8, 0.5), (3.4, 0.4, 0.6)):
                    o.append(self._circle(c.x + dx, c.y + dy, 0.26, fill=eblue, fill_op=op))

        # 7. labels (above by default; flip below if they'd hit the masthead)
        for c in self.comps:
            if not c.label:
                continue
            acc = accent_for(c, t)
            size = 1.9 if c.kind in ("knob", "knob_sm") else 1.6
            fpr = _SWITCH_HALF_H if c.kind in ("switch2", "switch3") else _KINDS[c.kind][2]
            ly = c.y - fpr - 1.4
            if ly < 14.5:                       # too close to masthead -> label below
                ly = c.y + fpr + 2.6
            o.append(text_paths(c.label, c.x, ly, size, acc, "middle", weight=0.18))

        # 8. free notes (col may be a theme key like "gray"/"eblue", or a literal colour)
        for (x, y, text, _cls, col, anchor) in self.notes:
            o.append(text_paths(text, x, y, 1.6, t.get(col, col), anchor, weight=0.16))

        # 9. footer telemetry (width-aware)
        o.append(self._line(mx, H - 6.2, w - mx, H - 6.2, hair, 0.25, 0.34))
        fb = H - 4.0
        if narrow:
            sw0 = text_width("SAM-E", 1.6, display=True)
            o.append(self._circle(mid - sw0 / 2 - 1.6, fb - 0.55, 0.8, fill=orange, fill_op=0.95))
            o.append(text_paths("SAM-E", mid, fb, 1.6, ink, "middle", display=True))
        else:
            o.append(text_paths(f"S- {self.hp}HP", smx, fb, 1.45, gray, "start", weight=0.16))
            o.append(text_paths("SAM-E", mid, fb, 1.6, ink, "middle", display=True))
            sw = text_width("SIGNAL STABLE", 1.4)
            o.append(self._circle(w - smx - sw - 2.0, fb - 0.55, 0.85, fill=orange, fill_op=0.95))
            o.append(text_paths("SIGNAL STABLE", w - smx, fb, 1.4, gray, "end", weight=0.16))

        o.append("</svg>")
        return "\n".join(o) + "\n"

    # --- C++ placement fragment emit ---------------------------------------------
    def inc(self) -> str:
        lines = [
            "// SPDX-License-Identifier: GPL-3.0-or-later",
            f"// Generated by tools/panelgen from {self.module} spec. Do not edit; regenerate.",
            f"// #include this inside the {self.module}Widget constructor.",
        ]
        for c in self.comps:
            v = f"mm2px(Vec({c.x:.3f}f, {c.y:.3f}f))"
            eid = f"{self.module}::{c.eid}"
            if c.kind == "light":
                lines.append(f"addChild(createLightCentered<{c.light_tpl}>({v}, module, {eid}));")
            else:
                ctor, add, _ = _KINDS[c.kind]
                lines.append(f"{add}({ctor}({v}, module, {eid}));")
        return "\n".join(lines) + "\n"

    def write(self, svg_path: str | Path, inc_path: str | Path):
        Path(svg_path).write_text(self.svg())
        Path(inc_path).write_text(self.inc())
        print(f"  wrote {svg_path}")
        print(f"  wrote {inc_path}")
