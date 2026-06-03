# S-Bank Rack plugin

This is the publishable VCV Rack plugin path. It is native C++: the DSP lives in
`src/dsp/SBankDSP.hpp` (header-only), and the Rack modules own C++ DSP objects
directly. Nothing beyond a C++ compiler and the Rack SDK is needed to build it,
which is what makes it eligible for the official VCV Library build farm.

> Status: DSP smoke-tested and golden-regression-checked locally without the Rack
> SDK (see `test/`); built and loaded in VCV Rack 2 on macOS arm64.

## Layout

- `src/dsp/SBankDSP.hpp` -- native C++ DSP for Vactrol LPG and Strike.
- `src/plugin.{hpp,cpp}` -- plugin entry points.
- `src/VactrolLPG.cpp` -- the module: owns one C++ core per polyphony channel.
- `src/Strike.cpp` -- the dual Strike module.
- `plugin.json` -- Rack plugin manifest.
- `Makefile` -- standard Rack plugin build file.
- `res/` -- panel SVGs.
- `test/` -- standalone C++ DSP tests: golden regression (`run_golden.sh`) and a
  finite/sane smoke test (`dsp_smoke.cpp`). Neither needs the Rack SDK.

## Build

```sh
make RACK_DIR=/path/to/Rack-SDK
```

## DSP smoke test

```sh
c++ -std=c++11 -Wall -Wextra -pedantic -I src test/dsp_smoke.cpp \
  -o /tmp/sbank_dsp_smoke && /tmp/sbank_dsp_smoke
```

## Notes

- The DSP sound is locked by the golden regression in `test/` — run it after any
  change to `src/dsp/SBankDSP.hpp` and re-bless only on intentional sound changes.
- Licensing: this plugin is **GPL-3.0-or-later** (`plugin.json`), built from the
  permissive `src/dsp/` core (`MIT OR Apache-2.0`) plus GPL module glue. The panel
  art in `res/` is brand-reserved, not under the code license. See the top-level
  [`LICENSE.md`](../../LICENSE.md).
