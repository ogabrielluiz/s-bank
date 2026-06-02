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


def build_strike() -> Panel:
    """Dual, mirrored clean EG gate. 16 HP."""
    p = Panel(module="Strike", title="STRIKE", hp=16, serial="002",
              style="mk1", sub="DUAL LOW-PASS GATE | ZERO BLEED")
    cx = [20.32, 60.96]  # two channel columns
    for ch, x in enumerate(cx):
        a = "A" if ch == 0 else "B"
        po = a + "_"
        # channel header in the outer-top corner of the bay (clears the OPEN label)
        p.note(x - 11 if ch == 0 else x + 11, 14.6, f"CH {a}", "sm",
               anchor="start" if ch == 0 else "end")
        p.knob(x, 24, f"{po}OPEN_PARAM", "OPEN", lo="SHUT", hi="OPEN")
        p.knob(x, 42, f"{po}DECAY_PARAM", "DECAY", lo="FAST", hi="SLOW")
        p.knob(x, 60, f"{po}MATERIAL_PARAM", "MATERIAL", lo="HARD", hi="SOFT")
        p.trim(x - 8.5, 73, f"{po}DECAYCV_PARAM", "DEC")
        p.trim(x + 8.5, 73, f"{po}CTRLCV_PARAM", "CTRL")
        p.light(x, 82, f"{po}OPEN_LIGHT")
        p.input(x - 9, 93, f"{po}IN_INPUT", "IN")
        p.input(x + 9, 93, f"{po}HIT_INPUT", "HIT")
        p.input(x - 9, 105, f"{po}DECAY_INPUT", "DEC")
        p.input(x + 9, 105, f"{po}CTRL_INPUT", "CTRL")
        p.output(x, 117, f"{po}OUT_OUTPUT", "OUT")
    p.note(40.64, 110.0, "IMPERF", "sm", color="eblue")
    p.switch(40.64, 117, "IMPERFECTION_PARAM")
    return p


def build_vactrol() -> Panel:
    """Single-voice vactrol 292 LPG. 6 HP."""
    p = Panel(module="VactrolLPG", title="LPG", hp=6, serial="001",
              style="mk1", sub="VACTROL 292 | SINGLE VOICE")
    x = 15.24
    p.divider(82.0)
    p.knob(x, 20, "RESONANCE_PARAM", "RESO")
    p.knob(x, 40, "DRIVE_PARAM", "DRIVE")
    p.switch(x, 57, "MODE_PARAM", three=True)
    p.note(x, 50.5, "MODE", "sm")
    p.switch(x, 71, "OVERSAMPLE_PARAM", three=True)
    p.note(x, 64.5, "OS", "sm")
    p.input(x, 92, "AUDIO_INPUT", "IN")
    p.input(x, 104, "CV_INPUT", "CV")
    p.output(x, 117, "AUDIO_OUTPUT", "OUT")
    return p


def main() -> None:
    RES.mkdir(parents=True, exist_ok=True)
    for build in (build_strike, build_vactrol):
        p = build()
        print(f"{p.module} ({p.hp} HP, style={p.style}):")
        # The C++ placement is finish-independent — write it once.
        (SRC / f"{p.module}_panel.inc").write_text(p.inc())
        # Emit both finishes so the module can toggle Black/Silver at runtime.
        for fin, suffix in (("black", ""), ("silver", "-silver")):
            p.finish = fin
            (RES / f"{p.module}{suffix}.svg").write_text(p.svg())
            print(f"  wrote res/{p.module}{suffix}.svg")
        print(f"  wrote src/{p.module}_panel.inc")


if __name__ == "__main__":
    main()
