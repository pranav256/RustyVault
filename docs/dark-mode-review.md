# Dark mode and spacing review

Windows x64, 2026-11-25. The existing Artistic design is retained. Its bundled
source supplies a light palette; the dark palette is a native adaptation, with
the same copper roles, Georgia/Inter typography and key icon.

Dark mode is the default. The footer and Settings → Appearance switch palettes
immediately. A normal session stores only `dark` or `light` in the application
config directory's `appearance.txt`; the setting works before unlocking and
survives locking. Demo sessions do not read or write that preference. Preference
write errors leave the selected appearance active and show an error. The existing
vault/backup/lock path protection also guards the preference write.

Observed spacing fixes:

- Search text is vertically centered in an actual 44px response rectangle.
- Entry rows have consistent 53px heights in both palettes and at both widths;
  long titles and usernames truncate with their full text available on hover.
- List columns shrink to 256px at the minimum size, leaving more detail space.
- Detail insets and gaps are smaller; the normal-size view now shows tags below
  the website where the previous capture stopped at the website field.
- Sidebar section gaps are smaller, retaining the separate scrolling group/tag
  area and fixed navigation footer.
- Form labels have a 4px gap to their inputs, and settings content is capped at
  720px instead of stretching fields across the entire window.
- Text inputs use 12px horizontal and 8px vertical text insets. Search, master
  password, editor, notes, settings and generator fields all share this padding.
- Custom entry rows have explicit selected accessibility metadata and a visible
  keyboard focus outline. Theme controls are disabled behind confirmation dialogs.

Eidova UI polish, responsive UX guidance and review rules were loaded. Appearance
copy passed the supplied-text comprehension check. The hosted source checker does
not accept Rust files, so its response was not counted as a source-code pass.
The Rust theme and affected callers were reviewed directly.

Native screenshots and actual instrumented egui rectangles are in
`target/ui-review-dark/`. Captures cover dark/light list/detail at 1200×800 and
900×650 logical pixels, dark settings at both sizes, and dark editor, generator,
locked screen and export confirmation at the minimum size. Windows scaling is
1.25; the minimum height rounds to 650.4 logical pixels. Content within intentional
scroll areas can extend beyond the visible portion of a screen.

Eidova's final two geometry reports across these ten states returned **0 critical,
0 serious and 102 moderate findings**. Moderate findings repeat the recommendation
for 44px touch targets on the existing 36px desktop controls. They are not 102
distinct defects. The measured sidebar is 184px, matching compact density. The
aggregate score is 0/100; this is not visual or accessibility approval. Raw results
are in `target/ui-review-dark/eidova-spacing.json`. Bounds instrumentation is
partial; off-viewport rectangles were excluded from the supplied measurements.

Automated contrast checks cover text on background/surface, selected text,
primary buttons and control/focus colors. Source-color ratios:

| Pair | Dark | Light |
| --- | ---: | ---: |
| Muted text / background | 7.87:1 | 4.65:1 |
| Secondary text / selection | 8.33:1 | 7.93:1 |
| Primary button text / accent | 8.15:1 | 5.34:1 |

These calculations do not replace a complete pixel-sampled audit.

All 16 tests passed, including palette contrast, theme switching, preference
contents/failure handling and vault path protection. Formatting passed. The final
release binary compiled with warnings denied. Its SHA-256 is
`b10163694af9855c7bc384b1c1af47752c434aae510e7837fcd4c58dc03a4ebb`.

Clippy is absent from the available Rust toolchain and was not run. Full native
keyboard/screen-reader walkthroughs, text zoom, OS theme synchronization and
Linux/macOS rendering were not verified. This review used synthetic demo data.
It did not open a personal vault or perform a security audit.

## Brave browser connection

The optional Windows MV3 extension and native messaging bridge are source-reviewed
in `browser-extension/` and `src/browser.rs`. Extension scripts require an exact
HTTPS origin and an explicit site permission, never auto-submit a login, and
stage submitted credentials in extension-process memory for at most one minute.
Filling is user-initiated; the unlocked app confirms a new or updated login before
saving. The encrypted-vault preference is off by default. The native host is bound
to IPv4 loopback, only accepts this extension's fixed origin and a random
per-unlock session token, and clears the session when locking. No browser data
leaves the machine.

Rust unit/integration checks pass, as do three Node tests for origin validation,
visible form extraction, same-origin fills and no implicit submission. The static
bridge framed-protocol check runs as part of Rust tests. The extension has not been
loaded into Brave, and a full native messaging/UI/permission walkthrough has not
been performed. These tests do not claim compatibility with every login form.
