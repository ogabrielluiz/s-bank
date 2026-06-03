#!/usr/bin/env python3
"""Regenerate every S-Bank panel (SVG + C++ placement .inc) from its spec.

Usage:  python3 tools/panelgen/generate.py

Each panel is declared ONCE below. Edit a spec, rerun this, rebuild the plugin —
the SVG art and the widget component positions stay in lockstep by construction.

v0.4: each panel now carries a ``style`` ("mk1" instrument / "mk2" telemetry /
"mk3" signal trace) and an optional ``sub`` caption. The Sam-e visual system
(jet-black environment, color-as-state, engraved instrument furniture) lives in
sam_panel.py — these specs just say what's on the panel and where.
"""

from pathlib import Path

from sam_panel import Panel

RACK = Path(__file__).resolve().parents[2] / "modules" / "rack"
RES = RACK / "res"
SRC = RACK / "src"
# The docs site embeds the same generated panel art; emit copies here so the
# existing "panels out of sync" CI guard covers the docs too (never hand-copied).
DOCS_PANELS = Path(__file__).resolve().parents[2] / "docs" / "public" / "panels"


def build_strike() -> Panel:
    """Dual, mirrored clean EG gate. 16 HP."""
    p = Panel(module="Strike", title="STRIKE", hp=16, serial="002",
              style="mk1", sub="DUAL LOW-PASS GATE | ZERO BLEED")
    cx = [20.32, 60.96]  # two channel columns
    for ch, x in enumerate(cx):
        a = "A" if ch == 0 else "B"
        po = a + "_"
        p.note(x - 11 if ch == 0 else x + 11, 15.5, f"CH {a}", "lg", color="ink",
               anchor="start" if ch == 0 else "end")
        # --- control zone ---
        # OPEN + DECAY faders, with the openness meter rising in the gap BETWEEN them: the
        # ladder fills with gate openness and falls at the decay rate (watch it ring out).
        p.slider(x - 7.0, 31, f"{po}OPEN_PARAM", "OPEN")
        p.slider(x + 7.0, 31, f"{po}DECAY_PARAM", "DECAY")
        p.meter(x, 43, f"{a}_METER_LIGHT", n=7, pitch=3.5)
        p.knob(x, 62, f"{po}MATERIAL_PARAM", "MATERIAL", lo="HARD", hi="SOFT", prime=True)
        # --- cable field: lifted into the freed space; 12 mm rows so each label clears
        # the jack above it. Each CV attenuator stays directly over its CV input. ---
        p.trim(x - 9, 78, f"{po}DECAYCV_PARAM")
        p.trim(x + 9, 78, f"{po}CTRLCV_PARAM")
        p.input(x - 9, 90, f"{po}DECAY_INPUT", "DEC")
        p.input(x + 9, 90, f"{po}CTRL_INPUT", "CTRL")
        p.input(x - 9, 102, f"{po}IN_INPUT", "IN")
        p.input(x + 9, 102, f"{po}HIT_INPUT", "HIT")
        p.output(x, 114, f"{po}OUT_OUTPUT", "OUT")
    return p


def build_vactrol() -> Panel:
    """Single-voice vactrol 292 LPG. 6 HP."""
    p = Panel(module="VactrolLPG", title="LPG", hp=6, serial="001",
              style="mk1", sub="VACTROL 292 | SINGLE VOICE")
    x = 15.24
    p.divider(84.0)
    # knobs spaced so each label clears the gauge above/below it (no flip-collisions)
    p.knob(x, 24, "RESONANCE_PARAM", "RESO")
    p.knob(x, 44, "DRIVE_PARAM", "DRIVE")
    p.note(x, 56.5, "MODE", "sm")
    p.switch(x, 62, "MODE_PARAM", three=True)
    p.note(x, 70.5, "OS", "sm")
    p.switch(x, 76, "OVERSAMPLE_PARAM", three=True)
    p.input(x, 93, "AUDIO_INPUT", "IN")
    p.input(x, 105, "CV_INPUT", "CV")
    p.output(x, 117, "AUDIO_OUTPUT", "OUT")
    return p


def main() -> None:
    RES.mkdir(parents=True, exist_ok=True)
    DOCS_PANELS.mkdir(parents=True, exist_ok=True)
    collided = False
    for build in (build_strike, build_vactrol):
        p = build()
        print(f"{p.module} ({p.hp} HP, style={p.style}):")
        # Guardrail: a label must not land on a control/jack or another label.
        warns = p.collisions()
        for w in warns:
            print(f"  !! collision: {w}")
        collided = collided or bool(warns)
        # The C++ placement is finish-independent — write it once.
        (SRC / f"{p.module}_panel.inc").write_text(p.inc())
        # Emit both finishes so the module can toggle Black/Silver at runtime. The
        # docs embed only the default (black) finish, so copy just that one.
        for fin, suffix in (("black", ""), ("silver", "-silver")):
            p.finish = fin
            svg = p.svg()
            (RES / f"{p.module}{suffix}.svg").write_text(svg)
            if fin == "black":
                (DOCS_PANELS / f"{p.module}.svg").write_text(svg)
            print(f"  wrote res/{p.module}{suffix}.svg" + ("  (+ docs copy)" if fin == "black" else ""))
        print(f"  wrote src/{p.module}_panel.inc")
    # Guardrails: fail loudly on malformed geometry (check.py) or any label collision.
    from check import check_all
    ok = check_all(RES)
    if collided:
        print("FAILED: label collisions detected (see !! lines above).")
    if not ok or collided:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
