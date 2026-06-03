# S-BANK logo

The mark is **`S-BANK` set in Space Grotesk** (the brand display face), with the hyphen
carrying the signal-orange (`#FF5A00`) — color is state, even in the logo. All glyphs are
**outlined to vector paths**, so the files render anywhere with no font dependency
(GitHub, favicons, screen-print, hardware).

## Files
| File | Use |
|------|-----|
| `s-bank-wordmark.svg` / `-light.svg` | **README header / nav / docs.** Monogram + engraved BANK. (dark / light bg) |
| `s-bank-badge.svg` / `-light.svg` | **App icon / avatar / sticker.** Monogram in an instrument bezel; reads down to ~16px. |
| `s-bank-signal.svg` | **Hero / splash / motion ident.** Hyphen extends into a signal trace. |
| `s-bank-*-on-dark.svg` / `-on-light.svg` | Transparent-background variants for placing on your own color. |

For a GitHub README, `s-bank-wordmark.svg` (dark) or `-on-light.svg` (transparent) is
the usual pick:
```md
![S-BANK](logos/s-bank-wordmark.svg)
```

## Construction & rules
- **Type:** Space Grotesk Bold, outlined to paths. To edit the wordmark, reset it in
  Space Grotesk and re-outline (or open `S-Bank Logo.html` and export).
- **Colors:** ink = warm white `#F2EFE9` (on dark) or jet black `#0B080B` (on light);
  signal = orange `#FF5A00`. The orange is *only* ever the hyphen — the color-is-state rule.
- **Clear space:** keep at least the cap-height of the `S` clear on all sides.
- **Don't:** recolor the hyphen anything but the signal orange (or ink, when "off"),
  re-set it in another typeface, condense/stretch, or add effects.

## Regenerating
Preview, recolor, toggle the signal, and download any variant from `S-Bank Logo.html`
(it uses the live Space Grotesk web font). The committed SVGs here are the outlined,
font-independent exports of those same marks.
