# S-Bank Rack plugin

This is the publishable VCV Rack plugin path. It is native C++: the DSP lives in
`src/dsp/SBankDSP.hpp`, and the Rack modules own C++ DSP objects directly. No
Rust staticlib, Cargo step, or generated C ABI header is required to build the
plugin.

> Status: C++ DSP smoke-tested locally without the Rack SDK. A full Rack SDK build
> and an in-Rack audio smoke test still need to be run on the target platforms.

## Layout

- `src/dsp/SBankDSP.hpp` -- native C++ DSP for Vactrol LPG and Strike.
- `src/plugin.{hpp,cpp}` -- plugin entry points.
- `src/VactrolLPG.cpp` -- the module: owns one C++ core per polyphony channel.
- `src/Strike.cpp` -- the dual Strike module.
- `plugin.json` -- Rack plugin manifest.
- `Makefile` -- standard Rack plugin build file.
- `res/` -- panel SVGs.
- `test/dsp_smoke.cpp` -- standalone C++ DSP compile/runtime smoke test.

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

- The Rust crates under `components/` remain reference implementations for now.
  They are not part of the Rack plugin build.
- The C++ port should be compared against the existing Rust goldens before the
  Rust reference code is deleted.
- Licensing: the clean-room core is `MIT OR Apache-2.0`. Shipping inside Cardinal
  or as a non-exception VCV plugin would make the whole plugin GPLv3; choose
  deliberately before release.
