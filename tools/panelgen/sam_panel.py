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
    (A three-colour signal system. Config switches stay neutral white — not a signal.)
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

# Rack mounting-screw keep-out. The screws are placed by the module widget, NOT
# the SVG (see modules/rack/src/plugin.hpp): one HP in from each side, at the top
# and bottom rows. Masthead/footer text must clear them, so the guardrail models
# each as a circle. Geometry mirrors plugin.hpp exactly.
MM_PER_PX = HP_MM / 15.0              # RACK_GRID_WIDTH = 15 px = 1 HP
RACK_GRID_HEIGHT_PX = 380.0          # standard 3U panel
SCREW_R_MM = 7.5 * MM_PER_PX         # half a grid cell — the screw-head keep-out

INK = "#0B080B"      # jet black   — the environment / silence / space
PAPER = "#F5F5F1"    # NASA white  — structure
GRAY = "#A6A6A6"     # mission gray — secondary type
ENGRAVE = "#ECE7DD"  # lettering (warm white, reads engraved on black)
ORANGE = "#FF5A00"   # signal / active / transmitting
YELLOW = "#FFC400"   # energy / peak / the strike
CYAN = "#19D2E5"     # information / data / flow
HAIR = "#F5F5F1"     # structure lines, drawn at low opacity

DISPLAY = "Space Grotesk, Helvetica, Arial, sans-serif"  # reference only; type is outlined
MONO = "Fira Code, 'IBM Plex Mono', monospace"

# Finishes. The environment flips and the engraving flips; signal-state accents stay
# in-family but deepen on light aluminium so they still read (voltage, not a rainbow).
THEMES = {
    "black": {
        "bg": ("#100b12", "#0b070b", "#08060a"),
        "ink": "#ECE7DD", "hair": "#F5F5F1", "gray": "#BDBDBD",
        "well": "#14101a", "wellop": 0.9, "wellstroke": 0.16,
        "orange": "#FF5A00", "yellow": "#FFC400", "cyan": "#19D2E5",
    },
    "silver": {
        "bg": ("#e2e3e6", "#d2d3d7", "#bfc0c5"),
        "ink": "#17141c", "hair": "#2b2733", "gray": "#6c6d72",
        "well": "#b4b5ba", "wellop": 0.9, "wellstroke": 0.5,
        "orange": "#E8500A", "yellow": "#A87400", "cyan": "#0E93A6",
    },
}

# Component kind -> (Rack widget class, add-call, footprint_radius_mm).
# Footprint radii match what Rack actually draws on top, so labels clear the part.
_PARAM, _INPUT, _OUTPUT = "addParam", "addInput", "addOutput"
SLIDER_W, SLIDER_H = 6.72, 25.93   # VCVSlider footprint (its px viewBox * 25.4/75)
_KINDS = {
    "knob":    ("createParamCentered<RoundBlackKnob>",      _PARAM,  6.4),
    "knob_sm": ("createParamCentered<RoundSmallBlackKnob>", _PARAM,  4.6),
    "slider":  ("createParamCentered<VCVSlider>",           _PARAM,  SLIDER_W / 2),
    "trim":    ("createParamCentered<Trimpot>",             _PARAM,  2.6),
    "switch2": ("createParamCentered<CKSS>",                _PARAM,  2.6),
    "switch3": ("createParamCentered<CKSSThree>",           _PARAM,  2.6),
    "in":      ("createInputCentered<PJ301MPort>",          _INPUT,  3.15),
    "out":     ("createOutputCentered<PJ301MPort>",         _OUTPUT, 3.15),
}
_SWITCH_HALF_H = 2.7  # half the CKSS body height (5.4mm tall); label clears it


def _rect_circle_hit(box, circ, tol):
    """True if axis-aligned box (x0,y0,x1,y1) overlaps circle (cx,cy,r) by more than tol."""
    x0, y0, x1, y1 = box
    cx, cy, r = circ
    nx = min(max(cx, x0), x1)
    ny = min(max(cy, y0), y1)
    dx, dy = cx - nx, cy - ny
    return r > tol and (dx * dx + dy * dy) < (r - tol) ** 2


def _rect_rect_hit(a, b, tol):
    return (a[0] < b[2] - tol and b[0] < a[2] - tol and
            a[1] < b[3] - tol and b[1] < a[3] - tol)


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
        return t["ink"]                # mode/config selector — neutral, not a signal accent
    if c.kind == "in":
        if "HIT" in c.label.upper():
            return t["yellow"]         # the trigger that fires the strike
        if any(k in c.label.upper() for k in ("DEC", "CTRL", "CV")):
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
    prime: bool = False           # primary control — gets a heavier gauge arc
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
    def _add(self, kind, x, y, eid, label="", accent=None, light_tpl=None, lo="", hi="", prime=False):
        c = _Comp(kind, x, y, eid, label, accent, lo, hi)
        c.prime = prime
        if light_tpl:
            c.light_tpl = light_tpl
        self.comps.append(c)

    def knob(self, x, y, eid, label="", small=False, lo="", hi="", prime=False):
        self._add("knob_sm" if small else "knob", x, y, eid, label, lo=lo, hi=hi, prime=prime)

    def slider(self, x, y, eid, label="", lo="", hi=""):
        self._add("slider", x, y, eid, label, lo=lo, hi=hi)

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

    def meter(self, x, y_bottom, base, n=5, pitch=2.6, tpl="TinyLight<YellowLight>"):
        """A vertical LED ladder (openness meter): n segments stacked upward from
        y_bottom. Segment index 0 is the bottom (it lights first). eids are
        `<base> + <i>` so the C++ enum can be a 5-wide block."""
        for i in range(n):
            self._add("seg", x, y_bottom - i * pitch, f"{base} + {i}", light_tpl=tpl)

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
    def _gauge(self, cx, cy, r, op, thin, caps=True, prime=False):
        """270deg instrument dial: a single graduated arc + sparse ticks + top index.
        Deliberately quiet — one concentric layer, 5 marks. A ``prime`` control gets a
        heavier arc so it reads as primary instead of competing with its neighbours."""
        hair = self._t["hair"]
        a0, sweep = 135, 270
        arc_w = 0.16 if thin else (0.36 if prime else 0.18)
        out = [self._arc(cx, cy, r, a0, 45, 1, 1, hair, arc_w, op + (0.22 if prime else 0.0))]
        n = 4                                     # 5 marks: ends + centre + the two quarters
        for i in range(n + 1):
            a = a0 + (i / n) * sweep
            major = (i % 2 == 0)                  # ends + centre are major
            ri, ro = r - (1.2 if major else 0.6), r + 0.1
            x0, y0 = self._polar(cx, cy, ri, a)
            x1, y1 = self._polar(cx, cy, ro, a)
            out.append(self._line(x0, y0, x1, y1, hair, 0.2 if major else 0.12,
                                   op + (0.1 if major else 0.0)))
        ti = self._polar(cx, cy, r - 1.9, 270)    # top index pointer (12 o'clock)
        tb = self._polar(cx, cy, r + 0.6, 270)
        out.append(self._line(ti[0], ti[1], tb[0], tb[1], hair, 0.28, op + 0.3))
        if caps:                                  # min/max caps — skipped when lo/hi text marks the ends
            for ea in (a0, 45):
                ex, ey = self._polar(cx, cy, r, ea)
                out.append(self._circle(ex, ey, 0.4, fill=hair, fill_op=op + 0.2))
        return "".join(out)

    def _well(self, cx, cy, r):
        t = self._t
        return self._circle(cx, cy, r, fill=t["well"], fill_op=t["wellop"],
                            stroke=t["hair"], w=0.2, op=t["wellstroke"])

    def _ping(self, cx, cy, scale):
        """The openness indicator — one ring + the lit core. Yellow = the strike, so it
        reads clearly (it's the module's identity state), while staying a single ring."""
        yellow = self._t["yellow"]
        return (self._circle(cx, cy, 2.8 * scale, stroke=yellow, w=0.26, op=0.5)
                + self._circle(cx, cy, 1.2, fill=yellow, fill_op=0.95))

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
        orange, yellow, cyan = t["orange"], t["yellow"], t["cyan"]
        w, H, mid = self.w, PANEL_H, self.w / 2
        mx = 5 if w < 50 else 7          # side margin (body)
        smx = 11.5                       # corner-row margin: clears the Rack screws
        narrow = w < 50                  # simplify masthead/footer on slim panels
        mast_div = 12.8 if narrow else 12.2  # masthead rule sits below the (lowered) slim masthead
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
            # a small registration dot — not a diamond (which read as a central function)
            o.append(self._circle(mid, 58, 0.5, fill=hair, fill_op=0.2))
        if S.zones == "boxes":
            for cx in cols:
                o.append(self._rrect(cx - 11, 15.5, 22, 105.5, 1.4, stroke=hair, sw=0.2, op=0.13))
        elif S.zones == "brackets":
            sliders = [c for c in self.comps if c.kind == "slider"]
            for cx in cols:
                grp = [c for c in sliders if abs(c.x - cx) <= 8]
                if grp:                       # frame just the fader pair — the "envelope" group
                    x0 = min(c.x for c in grp) - SLIDER_W / 2 - 1.6
                    x1 = max(c.x for c in grp) + SLIDER_W / 2 + 1.6
                    y0 = min(c.y for c in grp) - SLIDER_H / 2 - 1.6
                    y1 = max(c.y for c in grp) + SLIDER_H / 2 + 5.6
                    o.append(self._brackets(x0, y0, x1, y1, 0.42, length=2.8))
                else:
                    o.append(self._brackets(cx - 11, 15.5, cx + 11, 121, 0.3))

        # 3. masthead — wordmark + title + corner stamp, laid out by _furniture()
        # so the art and the collision guardrail share one source of truth.
        roles = {"orange": orange, "ink": ink, "gray": gray}
        for r in self._furniture():
            if r["zone"] == "mast":
                o.append(text_paths(r["t"], r["x"], r["y"], r["s"], roles[r["role"]],
                                    r["a"], display=r["disp"]))
        o.append(self._line(mx, mast_div, w - mx, mast_div, hair, 0.25, 0.34))
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

        # 5. CV grouping: each attenuverter links DOWN to the CV input it scales, so the
        # relationship reads at a glance (no floating "CV" that looks like a third control).
        trims = [c for c in self.comps if c.kind == "trim"]
        ins = [c for c in self.comps if c.kind == "in"]
        for tr in trims:
            below = [j for j in ins if abs(j.x - tr.x) < 0.6 and 0 < (j.y - tr.y) < 16]
            if below:
                j = min(below, key=lambda j: j.y)
                o.append(self._line(tr.x, tr.y + 2.4, j.x, j.y - 3.4, cyan, 0.18, 0.4))

        # 6. per-component furniture: wells, gauges, ping, accent rings
        for c in self.comps:
            acc = accent_for(c, t)
            if c.kind in ("knob", "knob_sm"):
                fr = _KINDS[c.kind][2]
                o.append(self._well(c.x, c.y, fr - 0.6))
                o.append(self._gauge(c.x, c.y, fr + 0.9, 0.28, S.gauges_thin,
                                     caps=not (c.lo or c.hi), prime=c.prime))
                if c.prime:                      # hero knob — a faint halo the faders lack
                    o.append(self._circle(c.x, c.y, fr + 1.9, stroke=hair, w=0.3, op=0.4))
                if c.lo or c.hi:                 # scale-end descriptors just outside the arc ends
                    capr = (fr + 0.9) * 0.707
                    ex, ey = capr + 0.4, c.y + capr + 1.2
                    if c.lo:
                        o.append(text_paths(c.lo, c.x - ex, ey, 1.3, gray, "end", weight=0.15))
                    if c.hi:
                        o.append(text_paths(c.hi, c.x + ex, ey, 1.3, gray, "start", weight=0.15))
            elif c.kind == "slider":
                # Recessed engraved channel; VCVSlider draws the track + cap on top, so a
                # thin rim of well shows around it.
                o.append(self._rrect(c.x - SLIDER_W / 2 - 0.6, c.y - SLIDER_H / 2 - 0.6,
                                     SLIDER_W + 1.2, SLIDER_H + 1.2, 1.3,
                                     fill=t["well"], fill_op=t["wellop"],
                                     stroke=hair, sw=0.2, op=t["wellstroke"]))
                # graduated travel scale flanking the fader (like a mixer fader): 9 marks,
                # majors at the ends + centre. Faint — reads as a scale, not a tick ring.
                h2 = SLIDER_H / 2 - 1.6
                n = 8
                for i in range(n + 1):
                    yy = c.y - h2 + (i / n) * (2 * h2)
                    major = (i % 4 == 0)
                    ln = 1.5 if major else 0.9
                    for s in (-1, 1):
                        x0 = c.x + s * (SLIDER_W / 2 + 0.5)
                        o.append(self._line(x0, yy, x0 + s * ln, yy, hair,
                                            0.18 if major else 0.12, 0.4 if major else 0.22))
            elif c.kind == "trim":
                fr = _KINDS[c.kind][2]
                o.append(self._well(c.x, c.y, fr + 0.3))
                if acc != ink:            # subordinate cyan ring — the CV JACK below owns the bold ring
                    o.append(self._circle(c.x, c.y, fr + 0.5, stroke=acc, w=0.16, op=0.3))
                # 12-o'clock detent mark — signals a bipolar (±) attenuverter, centre = off
                o.append(self._line(c.x, c.y - fr - 0.4, c.x, c.y - fr - 1.3, hair, 0.2, 0.55))
            elif c.kind in ("in", "out"):
                fr = _KINDS[c.kind][2]
                o.append(self._well(c.x, c.y, fr + 0.5))
                if c.kind == "out":
                    o.append(self._circle(c.x, c.y, fr + 0.75, stroke=orange, w=0.28, op=0.55))
                elif acc != ink:
                    o.append(self._circle(c.x, c.y, fr + 0.7, stroke=acc, w=0.22, op=0.4))
            elif c.kind == "light":
                o.append(self._ping(c.x, c.y, S.ping))
            elif c.kind == "seg":     # openness-meter segment: small recessed LED well
                o.append(self._circle(c.x, c.y, 1.45, fill=t["well"], fill_op=t["wellop"],
                                      stroke=hair, w=0.16, op=t["wellstroke"]))
            elif c.kind in ("switch2", "switch3"):
                o.append(self._rrect(c.x - 1.3, c.y - _SWITCH_HALF_H, 2.6, _SWITCH_HALF_H * 2, 0.8,
                                     fill=t["well"], fill_op=t["wellop"], stroke=ink, sw=0.22, op=0.5))
                for dx, dy, op in ((-3.2, -1.0, 0.7), (-3.7, 0.8, 0.5), (3.4, 0.4, 0.6)):
                    o.append(self._circle(c.x + dx, c.y + dy, 0.26, fill=ink, fill_op=op))

        # 7. labels — placement via _label_place (shared with the collision guardrail)
        for c in self.comps:
            if not c.label:
                continue
            acc = accent_for(c, t)
            size, ly = self._label_place(c)
            o.append(text_paths(c.label, c.x, ly, size, acc, "middle", weight=0.18))

        # 8. free notes (col may be a theme key like "gray"/"cyan", or a literal colour)
        for (x, y, text, _cls, col, anchor) in self.notes:
            sz = 2.4 if _cls == "lg" else 1.6   # "lg" = structural header (e.g. CH A/B)
            o.append(text_paths(text, x, y, sz, t.get(col, col), anchor,
                                weight=0.2 if _cls == "lg" else 0.16))

        # 9. footer telemetry — text from _furniture(); the status dots stay inline.
        foot = [r for r in self._furniture() if r["zone"] == "foot"]
        o.append(self._line(mx, H - 6.2, w - mx, H - 6.2, hair, 0.25, 0.34))
        fb = H - 4.0
        if narrow:
            sw0 = text_width("SAM-E", 1.6, display=True)
            o.append(self._circle(mid - sw0 / 2 - 1.6, fb - 0.55, 0.8, fill=orange, fill_op=0.95))
            for r in foot:                                # [SAM-E]
                o.append(text_paths(r["t"], r["x"], r["y"], r["s"], roles[r["role"]],
                                    r["a"], display=r["disp"]))
        else:
            o.append(text_paths(foot[0]["t"], foot[0]["x"], foot[0]["y"], foot[0]["s"], gray, "start"))
            o.append(text_paths(foot[1]["t"], foot[1]["x"], foot[1]["y"], foot[1]["s"], ink, "middle", display=True))
            sw = text_width("SIGNAL STABLE", 1.4)
            # neutral status dot — orange is reserved for OUT/identity, not decorative telemetry
            o.append(self._circle(w - smx - sw - 2.0, fb - 0.55, 0.7, fill=gray, fill_op=0.7))
            o.append(text_paths(foot[2]["t"], foot[2]["x"], foot[2]["y"], foot[2]["s"], gray, "end"))

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
            if c.kind in ("light", "seg"):
                lines.append(f"addChild(createLightCentered<{c.light_tpl}>({v}, module, {eid}));")
            else:
                ctor, add, _ = _KINDS[c.kind]
                lines.append(f"{add}({ctor}({v}, module, {eid}));")
        return "\n".join(lines) + "\n"

    # --- label placement: single source of truth for svg() AND collisions() ------
    def _label_place(self, c):
        """(size_mm, baseline_y) for a component's main label."""
        if c.kind == "slider":                   # name sits clearly below the fader well
            return 1.8, c.y + SLIDER_H / 2 + 3.8
        if c.kind in ("knob", "knob_sm"):
            size = 2.3 if c.prime else 1.9
            fpr = _KINDS[c.kind][2] + (1.9 if c.prime else 0.9)   # clear the gauge / prime halo
        elif c.kind in ("switch2", "switch3"):
            size, fpr = 1.6, _SWITCH_HALF_H
        else:
            size, fpr = 1.6, _KINDS[c.kind][2]
        ly = c.y - fpr - 1.4
        if ly < 14.5:                            # too close to masthead -> flip below
            ly = c.y + fpr + 2.6
        return size, ly

    # --- collision guardrail: warn when a label overlaps furniture or another label
    def _footprint(self, c):
        """Bounding circle (cx, cy, r) of a component's drawn furniture, or None."""
        if c.kind in ("knob", "knob_sm"):
            return (c.x, c.y, _KINDS[c.kind][2] + (1.9 if c.prime else 0.9))
        if c.kind == "trim":
            return (c.x, c.y, _KINDS[c.kind][2] + 0.8)
        if c.kind in ("in", "out"):
            return (c.x, c.y, _KINDS[c.kind][2] + 0.9)
        if c.kind == "light":
            return (c.x, c.y, 2.8 * STYLES.get(self.style, STYLES["mk1"]).ping)
        if c.kind == "seg":
            return (c.x, c.y, 1.6)
        return None

    def _footprint_rect(self, c):
        if c.kind == "slider":
            return (c.x - SLIDER_W / 2 - 0.6, c.y - SLIDER_H / 2 - 0.6,
                    c.x + SLIDER_W / 2 + 0.6, c.y + SLIDER_H / 2 + 0.6)
        if c.kind in ("switch2", "switch3"):
            return (c.x - 1.6, c.y - _SWITCH_HALF_H, c.x + 1.6, c.y + _SWITCH_HALF_H)
        return None

    def _label_box(self, c):
        if not c.label:
            return None
        size, ly = self._label_place(c)
        w = text_width(c.label, size)
        x0 = c.x - w / 2.0                        # component labels are middle-anchored
        return (x0, ly - size, x0 + w, ly + 0.12 * size)

    def _furniture(self):
        """Masthead + footer text, laid out ONCE so svg() draws exactly what
        collisions() audits — no drift between the art and the guardrail. Each
        record: {t, x, y, s(ize), role, a(nchor), disp(lay), zone}.

        On slim (<50 mm) panels the Rack screws occupy all four corners, so the
        masthead drops below the top screws (left-aligned, HP/serial tucked to the
        right of the title) and the footer stays in the clear central band."""
        w, mid, mx = self.w, self.w / 2, (5 if self.w < 50 else 7)
        smx = 11.5
        narrow = self.w < 50
        T = []

        def add(t, x, y, s, role, a="start", disp=False, zone="mast"):
            if t:
                T.append({"t": t, "x": x, "y": y, "s": s, "role": role,
                          "a": a, "disp": disp, "zone": zone})

        big = 1.9 if narrow else 2.1
        if narrow:
            # 6 HP: the Rack screws own the top corners and the first knob's label
            # sits high, so the masthead is a tight wordmark+title lockup below the
            # screws. There is no room for a corner HP/serial stamp — omit it.
            add("S-", mx, 8.0, big, "orange", "start", True)
            add("BANK", mx + text_width("S-", big, display=True) + 0.6, 8.0, big, "ink", "start", True)
            add(self.title, mx, 11.9, 3.4, "ink", "start", True)
        else:
            add("S-", smx, 5.4, big, "orange", "start", True)
            add("BANK", smx + text_width("S-", big, display=True) + 0.6, 5.4, big, "ink", "start", True)
            add(self.title, smx, 10.6, 3.6, "ink", "start", True)
            add(f"{self.hp}HP / S-{self.serial}", w - smx, 5.4, 1.5, "gray", "end")
            if self.sub:
                halves = [s.strip() for s in self.sub.split("|")]
                add(halves[0], w - smx, 8.0, 1.4, "gray", "end")
                if len(halves) > 1:
                    add(halves[1], w - smx, 10.4, 1.4, "gray", "end")
        fb = PANEL_H - 4.0
        if narrow:
            add("SAM-E", mid, fb, 1.6, "ink", "middle", True, zone="foot")
        else:
            add(f"S- {self.hp}HP", smx, fb, 1.45, "gray", "start", zone="foot")
            add("SAM-E", mid, fb, 1.6, "ink", "middle", True, zone="foot")
            add("SIGNAL STABLE", w - smx, fb, 1.4, "gray", "end", zone="foot")
        return T

    def _furniture_box(self, r):
        """Bounding box (x0, y0, x1, y1) of a _furniture() text record."""
        wd = text_width(r["t"], r["s"], display=r["disp"])
        a = r["a"]
        x0 = r["x"] if a == "start" else (r["x"] - wd if a == "end" else r["x"] - wd / 2.0)
        return (x0, r["y"] - r["s"], x0 + wd, r["y"] + 0.12 * r["s"])

    def _screws(self):
        """The four Rack mounting screws as keep-out circles (cx, cy, r), mm.
        Mirrors the placement in modules/rack/src/plugin.hpp."""
        w = self.w
        xl = HP_MM + SCREW_R_MM                 # one HP in, then half a cell to the centre
        xr = w - 2 * HP_MM + SCREW_R_MM
        yt = SCREW_R_MM
        yb = (RACK_GRID_HEIGHT_PX - 15.0) * MM_PER_PX + SCREW_R_MM
        return [(xl, yt, SCREW_R_MM), (xr, yt, SCREW_R_MM),
                (xl, yb, SCREW_R_MM), (xr, yb, SCREW_R_MM)]

    def collisions(self, clearance=1.0):
        """Human-readable warnings where any text (a component label, a note, or
        masthead/footer furniture) overlaps OR comes within `clearance` mm of a
        component footprint, a mounting screw, or other text. Catches crowding, not
        just hard overlaps. Used as a generate-time guardrail.

        Furniture text is intentionally tight (the wordmark halves, stacked corner
        captions), so furniture-vs-furniture flags only real OVERLAPS; everything
        else is held to the full `clearance`. Likewise a label sits adjacent to its
        OWN control by design, so against its owner only a real overlap is flagged."""
        near = -clearance   # expand shapes -> flag near-misses (gap < clearance)
        # items: (owner_comp_or_None, name, box, group)
        items = []
        for c in self.comps:
            b = self._label_box(c)
            if b:
                items.append((c, c.label, b, "label"))
        for (x, y, text, cls, _col, anchor) in self.notes:
            sz = 2.4 if cls == "lg" else 1.6
            w = text_width(text, sz)
            x0 = x if anchor == "start" else (x - w if anchor == "end" else x - w / 2.0)
            items.append((None, text, (x0, y - sz, x0 + w, y + 0.12 * sz), "note"))
        for r in self._furniture():
            items.append((None, r["t"], self._furniture_box(r), "furniture"))
        warns = []
        # text vs component footprints
        for owner, name, box, _grp in items:
            for c in self.comps:
                t = 0.25 if c is owner else near   # own control: overlap only; others: clearance
                circ, rect = self._footprint(c), self._footprint_rect(c)
                if (circ and _rect_circle_hit(box, circ, t)) or \
                   (rect and _rect_rect_hit(box, rect, t)):
                    how = "overlaps" if c is owner else "too close to"
                    warns.append(f"text '{name}' {how} {c.kind} {c.eid} @ ({c.x:.1f},{c.y:.1f})")
        # text vs text
        for i in range(len(items)):
            for j in range(i + 1, len(items)):
                both_furn = items[i][3] == "furniture" and items[j][3] == "furniture"
                t = 0.25 if both_furn else near    # furniture is tight by design: overlap only
                if _rect_rect_hit(items[i][2], items[j][2], t):
                    warns.append(f"text '{items[i][1]}' / '{items[j][1]}' too close")
        # text vs mounting screws
        for _owner, name, box, _grp in items:
            for s in self._screws():
                if _rect_circle_hit(box, s, -0.6):
                    warns.append(f"text '{name}' hits a mounting screw @ ({s[0]:.1f},{s[1]:.1f})")
        return warns

    def write(self, svg_path: str | Path, inc_path: str | Path):
        Path(svg_path).write_text(self.svg())
        Path(inc_path).write_text(self.inc())
        print(f"  wrote {svg_path}")
        print(f"  wrote {inc_path}")
