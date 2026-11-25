# RustyVault — Raycast

The user selected **Raycast only**. The runtime contains a single theme with
light and dark appearances. The multi-theme comparison bar, theme-ID preference
reader and alternate runtime palettes have been removed.

## Source and native mapping

Use the installed Eidova/OpenDesign source in `assets/themes/raycast/`.
Its original tokens and design notes are retained with Apache-2.0 attribution
in `assets/themes/LICENSE.txt` and `assets/NOTICE.md`. Other kits remain
archived reference assets from the earlier comparison.

Raycast uses charcoal layers, restrained coral branding and compact curves.
Dark roles: background #07080a, surface #101111, selection #20252d,
foreground #f9f9f9, secondary #cecece, muted #9c9c9d, interaction #55b3ff,
success #5fc992, brand #ff6363, border #282929 and danger #ff9696.
Primary actions use foreground with a contrasting label. Inputs and unchecked
controls use #666a73 outlines in dark mode and #85898f in light mode.
The light appearance is a native adaptation. Both are mapped in
`src/appearance.rs`; small-text colors are adjusted where needed for contrast.

Use bundled Inter for UI and display text, with the existing monospace face for
secrets and codes. Controls have 6px corners and windows 12px. Preserve the
native widgets, 4px spacing scale, 12px horizontal / 8px vertical text-input
padding, scrolling and keyboard behavior. Navigation is 192px wide, with 36px
rows and aligned native counts. Credential values share a single quiet surface
with aligned trailing copy actions. Settings use grouped sections with 16px
insets; timing preferences remain bounded native inputs.

Light/dark is the only appearance setting, stored outside the vault in
`appearance.txt`. Demo mode changes stay in memory. Stale theme-ID files are
ignored. Vault contents and browser behavior are independent of appearance.

The optional OTP feature generates a website's two-step sign-in codes.
It does not change vault unlock. Keep existing OTP secrets and code generation;
the detail label identifies these as website verification codes.

## Verification

See `docs/ui-polish-review.md` for the current visual review and
`docs/theme-browser-review.md` for the historical comparison evidence.
Source checks and contrast tests do not establish complete visual or
accessibility approval.

## Guidance attribution

Polish guidance adapted from Unslop by Damir Sokolovsky
([@heydamir](https://x.com/heydamir), [source](https://heydamir.design/unslop)),
licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
The existing Eidova/OpenDesign Raycast kit remains the visual source.
