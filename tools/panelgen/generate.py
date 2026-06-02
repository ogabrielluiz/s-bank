#!/usr/bin/env python3
"""Regenerate every S-Bank panel (SVG + C++ placement .inc) from its spec.

Usage:  python3 tools/panelgen/generate.py

Each panel is declared ONCE below. Edit a spec, rerun this, rebuild the plugin —
the SVG art and the widget component positions stay in lockstep by construction.
"""

from pathlib import Path

from sam_panel import Panel

RACK = Path(__file__).resolve().parents[2] / "modules" / "rack"
RES = RACK / "res"
SRC = RACK / "src"


def build_strike() -> Panel:
    """Dual, mirrored clean EG gate. 16 HP."""
    p = Panel(module="Strike", title="STRIKE", hp=16, serial="002")
    cx = [20.32, 60.96]  # two channel columns
    p.divider(87.0)
    for ch, x in enumerate(cx):
        a = "A" if ch == 0 else "B"
        po = a + "_"
        p.note(x, 13.0, f"CH {a}", "sm")
        p.knob(x, 24, f"{po}OPEN_PARAM", "OPEN")
        p.knob(x, 42, f"{po}DECAY_PARAM", "DECAY")
        p.knob(x, 60, f"{po}MATERIAL_PARAM", "MATERIAL")
        p.trim(x - 8.5, 73, f"{po}DECAYCV_PARAM", "DEC")
        p.trim(x + 8.5, 73, f"{po}CTRLCV_PARAM", "CTL")
        p.light(x, 82, f"{po}OPEN_LIGHT")
        p.input(x - 9, 93, f"{po}IN_INPUT", "IN")
        p.input(x + 9, 93, f"{po}HIT_INPUT", "HIT")
        p.input(x - 9, 105, f"{po}DECAY_INPUT", "DEC")
        p.input(x + 9, 105, f"{po}CTRL_INPUT", "CTL")
        p.output(x, 117, f"{po}OUT_OUTPUT", "OUT")
    p.note(40.64, 110.5, "IMPERF", "sm")
    p.switch(40.64, 117, "IMPERFECTION_PARAM")
    return p


def build_vactrol() -> Panel:
    """Single-voice vactrol 292 LPG. 6 HP."""
    p = Panel(module="VactrolLPG", title="LPG", hp=6, serial="001")
    x = 15.24
    p.divider(82.0)
    p.knob(x, 20, "RESONANCE_PARAM", "RESO")
    p.knob(x, 40, "DRIVE_PARAM", "DRIVE")
    p.switch(x, 57, "MODE_PARAM", three=True)
    p.note(x, 51.5, "MODE", "sm")
    p.switch(x, 71, "OVERSAMPLE_PARAM", three=True)
    p.note(x, 65.5, "OS", "sm")
    p.input(x, 92, "AUDIO_INPUT", "IN")
    p.input(x, 104, "CV_INPUT", "CV")
    p.output(x, 117, "AUDIO_OUTPUT", "OUT")
    return p


def main() -> None:
    RES.mkdir(parents=True, exist_ok=True)
    for build in (build_strike, build_vactrol):
        p = build()
        print(f"{p.module} ({p.hp} HP):")
        p.write(RES / f"{p.module}.svg", SRC / f"{p.module}_panel.inc")


if __name__ == "__main__":
    main()
