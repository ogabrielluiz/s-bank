<!-- Thanks for contributing to S-Bank. Keep the summary short and the checklist honest. -->

## Summary

<!-- What does this change and why? -->

## Type of change

- [ ] Bug fix
- [ ] New feature / module
- [ ] DSP / sound change
- [ ] Panel / UI
- [ ] Docs
- [ ] CI / tooling / chore

## Checklist

- [ ] **DSP unchanged** — `modules/rack/test/run_golden.sh` passes. *(If the sound changed on purpose: re-blessed with `--bless`, and I describe the change below.)*
- [ ] **Panels** — any panel edit was made in the `tools/panelgen` spec and regenerated with `python3 tools/panelgen/generate.py` (no hand-edited SVG/`.inc`), and I checked it renders correctly.
- [ ] **Licensing** — new source files carry an `SPDX-License-Identifier` header (`MIT OR Apache-2.0` for `modules/rack/src/dsp/`, `GPL-3.0-or-later` elsewhere). I did not put brand assets (`res/*.svg`, the names/mark, `site/`) under a code license.
- [ ] **Builds** — `cd modules/rack && make RACK_DIR=/path/to/Rack-SDK` succeeds.

## Notes / screenshots

<!-- Intentional sound changes, panel before/after screenshots, anything reviewers should know. -->
