# Third-party assets

- `design-tokens.json` and `artistic/`: the Artistic rendered-fixture kit from
  [OpenDesign](https://github.com/nexu-io/open-design/tree/b4ec310921f49ae512de1f0a1f3a4e7ce3d05f2f/design-systems/artistic),
  supplied by Eidova. Copyright 2026 Open Design contributors,
  [Apache-2.0](artistic/LICENSE.txt). Source files are retained; native theme mapping
  is adapted for RustyVault. No React component source is distributed.
- `Inter.ttf`: Inter by the Inter Project Authors,
  [SIL Open Font License 1.1](Inter-LICENSE.txt), from Google Fonts.
- `LibreBaskerville.ttf`: Libre Baskerville by the Libre Baskerville Project
  Authors, [SIL Open Font License 1.1](LibreBaskerville-LICENSE.txt), from Google Fonts.
  It is the portable display fallback; installed Windows Georgia is used when
  available and is not redistributed.
- `icons/*.svg`: [Lucide](https://lucide.dev), ISC license;
  see `icons/LICENSE.txt`. Added navigation icons use Lucide 0.468.0 source
  from the official repository, with white strokes for native runtime tinting.
- `eff-wordlist.txt`: Electronic Frontier Foundation's
  [large passphrase wordlist](https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt),
  7,776 words, unchanged. Attribution: Electronic Frontier Foundation,
  [EFF Dice](https://www.eff.org/dice),
  [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) under [EFF copyright terms](https://www.eff.org/copyright).

Unslop guidance attribution is in `../DESIGN.md`.

- `themes/`: ten Eidova/OpenDesign token kits and original design notes, under
  Apache-2.0 (`themes/LICENSE.txt`). Each catalog entry in `themes/catalog.json`
  retains its source URL and license. Native adaptations, opposite appearances,
  contrast adjustments and fallback typography are in `src/appearance.rs`.
