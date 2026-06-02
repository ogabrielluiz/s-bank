"""Visual guardrails for the panel generator.

Catches the class of bug that shipped a garbled panel (malformed glyph geometry from
mishandled H/V path commands). Runs automatically at the end of generate.py, and can
be run standalone: `python3 check.py`. Exits non-zero on any failure.

Checks:
  1. samefont._placed handles every absolute command, incl. the single-axis H/V.
  2. Every baked glyph, once placed, stays within sane glyph bounds (no blow-up/swap).
  3. Every generated panel SVG is valid XML, has zero <text>, and keeps all path
     coordinates within the panel rectangle (nothing spills off the panel).
"""

from __future__ import annotations

import glob
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

from fontdata import FONTDATA
from samefont import _AXIS, _placed

_TOK = re.compile(r"[A-Za-z]|-?\d*\.?\d+(?:[eE][-+]?\d+)?")


def _points(d: str):
    """(x, y) anchor/control points of an absolute path — command-aware (mirrors the
    transform's command handling, so a wrong handler here can't hide a wrong one there)."""
    toks = _TOK.findall(d)
    pts, j, n, cmd = [], 0, len(toks), "M"
    lastx = lasty = 0.0
    while j < n:
        t = toks[j]
        if t[0].isalpha():
            cmd = t.upper()
            j += 1
            continue
        pat = _AXIS.get(cmd, "xy")
        if not pat:
            j += 1
            continue
        k = 0
        while k < len(pat) and j < n and not toks[j][0].isalpha():
            ax = pat[k]
            v = float(toks[j])
            if ax == "x":
                lastx = v
            else:
                lasty = v
            if ax == "y" or pat == "x":
                pts.append((lastx, lasty))
            j += 1
            k += 1
    return pts


def check_transform() -> list[str]:
    errs = []
    # A single H (odd coord count) must NOT shift x/y parity for the following pair —
    # this is exactly the bug that garbled the panels.
    placed = _placed("M0 0 H4 L6 -2 Z", 1.0, 10.0, 20.0)
    if "H 14.000" not in placed:
        errs.append(f"H not mapped to x: {placed}")
    if "L 16.000 18.000" not in placed:
        errs.append(f"H broke following x/y parity: {placed}")
    placed2 = _placed("M0 0 V-2 L6 -3 Z", 1.0, 10.0, 20.0)
    if "V 18.000" not in placed2 or "L 16.000 17.000" not in placed2:
        errs.append(f"V broke following x/y parity: {placed2}")
    if "H 8.000" not in _placed("M0 0 H4 Z", 2.0, 0.0, 0.0):
        errs.append("H not scaled")
    return errs


def check_glyphs() -> list[str]:
    errs = []
    for fk, fv in FONTDATA.items():
        for ch, (adv, d) in fv["glyphs"].items():
            if not d:
                continue
            pts = _points(_placed(d, 1.0, 0.0, 0.0))
            if not pts:
                continue
            xs = [p[0] for p in pts]
            ys = [p[1] for p in pts]
            # cap-normalised: x in glyph advance envelope, y above baseline (cap=-1..0)
            if min(xs) < -0.6 or max(xs) > adv + 0.6 or min(ys) < -1.6 or max(ys) > 0.6:
                errs.append(f"{fk} '{ch}' bbox out of bounds: "
                            f"x[{min(xs):.2f},{max(xs):.2f}] y[{min(ys):.2f},{max(ys):.2f}] adv={adv}")
    return errs


def check_svgs(res_dir: Path) -> list[str]:
    errs = []
    for f in sorted(glob.glob(str(res_dir / "*.svg"))):
        name = Path(f).name
        text = Path(f).read_text()
        try:
            root = ET.fromstring(text)
        except ET.ParseError as e:
            errs.append(f"{name}: invalid XML — {e}")
            continue
        if "<text" in text:
            errs.append(f"{name}: contains <text> (nanosvg won't render it)")
        m = re.search(r'viewBox="0 0 ([\d.]+) ([\d.]+)"', text)
        if not m:
            errs.append(f"{name}: no viewBox")
            continue
        w, h = float(m.group(1)), float(m.group(2))
        for d in re.findall(r'\sd="([^"]+)"', text):
            for px, py in _points(d):
                if not (-3 <= px <= w + 3 and -3 <= py <= h + 3):
                    errs.append(f"{name}: path coord ({px:.1f},{py:.1f}) outside panel {w}x{h}")
                    break
            else:
                continue
            break
    return errs


def check_all(res_dir: Path) -> bool:
    errs = check_transform() + check_glyphs() + check_svgs(res_dir)
    if errs:
        print("PANEL CHECK FAILED:", file=sys.stderr)
        for e in errs[:20]:
            print("  -", e, file=sys.stderr)
        return False
    print("panel checks passed (transform, glyphs, SVGs)")
    return True


if __name__ == "__main__":
    res = Path(__file__).resolve().parents[2] / "modules" / "rack" / "res"
    sys.exit(0 if check_all(res) else 1)
