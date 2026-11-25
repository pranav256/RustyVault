# Artistic theme review

Windows x64, 2026-11-15. The user selected Eidova's exact **Artistic** package.
Its rendered CSS/components and structured tokens define warm paper, copper,
Georgia and Inter. The inconsistent overview is retained for traceability;
see `DESIGN.md` for the authoritative native mapping and package attribution.

Installed source checksums match the supplied resource manifests:

- `assets/artistic/tokens.css`:
  `4ea178b94f7bda9aca2d16c023f27452d54dde56099ba03cb72b31469fd7f9dc`.
- `assets/design-tokens.json`:
  `a2497ce8f138b177d2026fc0e32bd9b02e7ffa6c81254e45943081d335149d3a`.

Formatting, Clippy with warnings denied, all 14 existing tests, and the Windows
release build passed. Final executable SHA-256:
`45c48e56c1f6a4816452575f9bc84a13344efd52df10a451cb771b59e0e376e1`.
The executable is `target/release/rusty-vault.exe`.

Actual synthetic native screenshots were reviewed for list/detail, minimum-size
layout, editor, settings, generator, locked/create forms and export confirmation.
Evidence is in `target/ui-review-artistic/`: each view has `screen.png` and actual
egui `bounds.json`. The review used 1200×800 and approximately 900×650 logical
pixels at Windows scaling 1.25. Scrollable content can extend below its viewport.

UI/UX principles and review rules were loaded. Supplied-copy comprehension passed.
Eidova's six-state geometry review returned **0 critical, 0 serious, 51 moderate**
findings. The moderate findings mainly recommend 44px touch targets for existing
36px desktop controls. The aggregate geometry score was 0/100, not a visual or
accessibility approval. Summary: `target/ui-review-artistic/eidova-spacing.json`.

The first contrast review found muted text on the warm selected row at 3.96:1;
it now uses the kit's secondary text at **7.93:1**. Other source-color calculations:
muted text on paper **4.65:1**, white primary-button text **5.34:1**, and danger text
on paper **5.45:1**. Input borders and focus indications use contrasting kit roles.
These calculations are not a complete pixel-sampled audit.

Georgia is read from the installed Windows fonts; it is not redistributed.
Inter and the Libre Baskerville portable fallback are embedded under their OFL
licenses. The app and theme work offline, including when Georgia is unavailable.

Full native keyboard/screen-reader walkthroughs, text zoom, OS interaction states
and Linux/macOS rendering remain unverified. No independent security audit was
performed. Data behavior and broader limitations are in `validation.md` and README.
