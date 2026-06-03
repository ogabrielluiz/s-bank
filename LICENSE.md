# S-Bank — Licensing

S-Bank is open source. Different parts of the repository carry different licenses
so the reusable DSP can be embedded freely while the demo plugin stays copyleft and
the **Sam-e / S-Bank** brand stays owned. When in doubt, the per-file
`SPDX-License-Identifier` header is authoritative.

## Code

| Area | License |
| --- | --- |
| **Reusable DSP** — `modules/rack/src/dsp/**` (e.g. `SBankDSP.hpp`) | **MIT OR Apache-2.0** |
| **Everything else** — the VCV plugin glue (`modules/rack/src/*.cpp`, `*.hpp`, generated `*_panel.inc`), `tools/`, `docs/` | **GPL-3.0-or-later** |

- The permissive DSP is the building block a C++ VCV developer would `#include` to make
  their own modules — open *or* commercial. Pick MIT or Apache-2.0 at your option.
- The shipped plugin (`modules/rack`) as a whole is **GPL-3.0-or-later**: it combines
  the permissive DSP (GPLv3-compatible) with GPL module glue. `plugin.json` declares
  `GPL-3.0-or-later` accordingly.

Full texts: [`LICENSE-MIT`](LICENSE-MIT), [`LICENSE-APACHE`](LICENSE-APACHE),
[`LICENSE-GPLv3.txt`](LICENSE-GPLv3.txt).

## Brand assets — © Gabriel Almeida, all rights reserved (NOT under the code licenses)

The following are **not** covered by the MIT/Apache/GPL grants above and may **not** be
reused, redistributed, or modified without permission:

- The panel designs / trade dress: `modules/rack/res/*.svg`.
- The logos / brand marks in `logos/`.
- The **Sam-e** name, the **S-Bank** name, and the **S-** mark.
- The brand living document in `site/`.

(This mirrors how Befaco and Mutable Instruments keep their FOSS code open while
retaining their panel art and names.)

## Name reservation

**Sam-e**, **S-Bank**, the **S-** mark, and the S-Bank panel designs are trademarks of
Gabriel Almeida. You are welcome to build on the code under its license, but please do
**not** use these names or the panel look on derivative or competing works, or in any
way that implies endorsement.

## Fonts

The bundled fonts — Fira Code and Space Grotesk (`tools/panelgen/fonts/`) — are licensed
under the **SIL Open Font License 1.1** (see the `*-OFL.txt` files), which permits
embedding and redistribution.
