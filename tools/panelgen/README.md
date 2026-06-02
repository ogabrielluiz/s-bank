# S-Bank panel generator — v0.4 (the Sam-e visual system, as code)

Drop-in replacement for `tools/panelgen/`. Same API, same artifacts, same paths-only
nanosvg-safe output — but the panels now actually look like Sam-e instead of the
generic purple default.

```
panels/
  sam_panel.py    # the brand kit + renderer (this is the upgrade)
  strokefont.py   # unchanged behaviour + a ':' glyph (for S: style marks)
  generate.py     # specs — now carry `style=` and `sub=`
```

## What you get
- **Environment = jet black, engraving = NASA white.** No lavender, no purple.
- **Color is state** — assigned by what a part *does*, never decoration:
  - `orange` signal/active → **OUT** jacks, the `S-` mark, status LED
  - `yellow` energy/the strike → the **openness LED + ringing-body rings**, **HIT**
  - `cyan` information/control → **CV** trims & inputs (auto-tied as pairs)
  - `electric blue` depth/unknown → the **IMPERFECTION** switch
- Instrument furniture: 270° engraved **gauges** on every knob, recessed wells,
  registration crosshairs, a mirror axis, channel **bays**, masthead + footer telemetry.

## Three intensities (per-panel `style=`)
| style | character |
|-------|-----------|
| `mk1` | **Instrument** — restraint; bracketed bays, mirror axis, big ring-out |
| `mk2` | **Telemetry** — denser; boxed bays, edge ruler, the module's own decay curve |
| `mk3` | **Signal trace** — boldest; the audio path drawn through each channel, faint `S-` watermark |

## Usage — unchanged
```python
p = Panel(module="Strike", title="STRIKE", hp=16, serial="002",
          style="mk1", sub="DUAL LOW-PASS GATE | ZERO BLEED")
p.knob(x, 24, "A_OPEN_PARAM", "OPEN")
...
p.write(RES / "Strike.svg", SRC / "Strike_panel.inc")
```
`python3 generate.py` → `res/Strike.svg` + `src/Strike_panel.inc`, in lockstep.

## Component coordinates
**Unchanged** from your spec — every knob/jack/trim/LED/switch stays exactly where it
was, so no C++ moves are needed. The art simply leaves those footprints clear and
labels them. (The interactive proof has a "component overlay" toggle that superimposes
the Rack footprints to confirm clearance.)
