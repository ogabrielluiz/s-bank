# Vendored brand fonts (OFL)

Panel lettering is rendered as **filled vector paths** outlined from these fonts
(Rack's nanosvg can't draw `<text>`). `bake_font.py` reads them once and writes
`../fontdata.py` (pure-Python glyph outlines); the generator uses that — the fonts are
**not** needed at generation time, only to re-bake.

| File | Font | Use | License |
|---|---|---|---|
| `FiraCode-600.ttf` | [Fira Code](https://github.com/tonsky/FiraCode) (SemiBold) | labels | OFL 1.1 — `FiraCode-OFL.txt` |
| `SpaceGrotesk-700.ttf` | [Space Grotesk](https://github.com/floriankarsten/space-grotesk) (Bold) | masthead / title | OFL 1.1 — `SpaceGrotesk-OFL.txt` |

Both are the brand's chosen faces (Sam-e = Space Grotesk + Fira Code). Static TTFs
fetched from the [Fontsource](https://fontsource.org) CDN. Re-bake:

```sh
/tmp/fontenv/bin/python bake_font.py    # needs fonttools (pip install fonttools)
```
