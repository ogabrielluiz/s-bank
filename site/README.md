# Sam-e / S- — Signal System (brand living document)

A self-contained static site implementing the **Sam-e** brand system: a single-page
"living document" of the `S-` signal identity (boot-log intro, oscilloscope canvases,
the interactive *color = state* panel, the `S-` mode grid, voice posters, and a live
telemetry dashboard).

Implemented from a Claude Design handoff bundle (`SAM-e Signal System.html`) — see the
brand notes in the source CSS/JS.

## Run

No build step. Serve the folder and open it:

```sh
cd site
python3 -m http.server 8080
# open http://localhost:8080
```

(Fira Code + Space Grotesk load from Google Fonts, so the page wants a network
connection for type; everything else is local.)

## Layout

- `index.html` — the page (sections: Intro, Identity, S- System, Color, Type, Balance, Voice, Live)
- `css/system.css` — tokens (palette, type, layout shell). **Color = state**, never decoration.
- `css/components.css` — nav, dark frame, marks, cards, scroll-reveal
- `css/sections.css` — section-specific layout
- `js/signal.js` — the signal engine: scroll reveal, dark-frame nav flip, `S-` hyphen
  mode cycling, the oscilloscope canvas renderer, the boot intro, and the
  `color = state` interaction

## Brand rules baked in

- **Color is state, not decoration** — Orange = signal/active, Yellow = energy/peak,
  Cyan = information/data, Electric Blue = depth/atmosphere. Black & white are the
  environment. "Use color like a synth uses voltage, never as a rainbow."
- **The `S-` mark is one glyph in many modes** (`S- S~ S> S: S•`) — texture, "a faction,
  not a feature." Commit to one mode per context.
- **70 / 20 / 10** — electronic artist / technical systems / color emotion.
