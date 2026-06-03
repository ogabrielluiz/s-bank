# S-Bank

**The signal bank** — a library of analog-emulation DSP building blocks that help you
build VCV Rack modules with convincing analog behaviour and sound. Part of the
**Sam-e** signal system (`S-` is the signal). The library is the product; the modules
here are demos that use it to prove it works and show how.

The DSP is native C++ (header-only, so it tests without the Rack SDK). Two instruments
live in the bank today: a **vactrol low-pass gate** (Buchla 292 style — dirty,
resonant) and **Strike**, a clean, zero-bleed, envelope-driven low-pass gate.

## Repo layout — the library vs. the demos

- **`modules/`** — VCV Rack modules built on the library:
  - [`rack`](modules/rack) — the native C++ VCV plugin: DSP cores
    (`src/dsp/SBankDSP.hpp`), module sources, panels, and `plugin.json`.
- **`tools/`** — [`panelgen`](tools/panelgen): declarative panel generator (one spec →
  both the SVG art and the C++ widget coordinates).
- **`site/`** — the Sam-e / S- brand living document.
- **`docs/`** — design notes ([`DESIGN.md`](docs/DESIGN.md)).

## Quick start

```sh
# VCV Rack plugin (needs the Rack SDK):
cd modules/rack && make install RACK_DIR=/path/to/Rack-SDK

# DSP tests (no Rack SDK needed):
modules/rack/test/run_golden.sh                # golden regression
c++ -std=c++11 -Wall -Wextra -pedantic -I modules/rack/src \
  modules/rack/test/dsp_smoke.cpp -o /tmp/sbank_dsp_smoke && /tmp/sbank_dsp_smoke
```

## License

S-Bank is open source with a per-area split — see [`LICENSE.md`](LICENSE.md) for the
authoritative breakdown:

- **Reusable DSP** (`modules/rack/src/dsp/`) — **MIT OR Apache-2.0**. Embed it in your
  own modules, open or commercial.
- **The demo plugin** and everything else — **GPL-3.0-or-later**.
- **Brand assets** — the panel designs (`res/*.svg`), the **Sam-e** / **S-Bank** names,
  the **S-** mark, and `site/` are © Gabriel Almeida, all rights reserved, and are **not**
  covered by the code licenses. Please don't reuse the names or the panel look on
  derivative works.
- Bundled fonts (Fira Code, Space Grotesk) are under the SIL Open Font License 1.1.
