# RustyVault validation

This records the earlier Precision theme and data-behavior checks. The active
Artistic theme and its current build evidence are documented in `artistic-review.md`.

Checked on Windows x64 on 2026-11-15. The local project override is
`stable-x86_64-pc-windows-msvc`. This resolves the missing GNU `dlltool.exe`
build dependency. The rebuilt executable is `target/release/rusty-vault.exe`.

## Build and data behavior

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --offline --all-targets -- -D warnings`: passed.
- `cargo test --offline`: 14 tests passed, none failed.
- `cargo build --release --offline`: passed; native demo captures launched successfully.
- Final executable SHA-256:
  `f7ee15c6193d98e5fa13170d96359597d919aa92c6d9106566a778a490f3a03c`.

Tests cover encrypted round trips, wrong-password and ciphertext/version rejection,
original version-1 vault loading, credential history/TOTP/custom-field preservation,
attachments and encrypted backups, failed-backup and failed-save preservation,
CSV quoting and whole-input rejection, an RFC TOTP vector, the full EFF wordlist,
demo isolation, lock cleanup, Escape cancellation, export-path protection and
exclusive vault access between windows.

New fields use serde defaults, and the existing encryption envelope remains
version 1. The decrypted data schema is version 2. Keep an independent backup
before moving real data between application versions.

## Design and rendered evidence

The requested Eidova workflow selected Supabase / Precision, installed its actual
DTCG kit, and loaded UI and UX principles before changes. Review rules and text
comprehension checks were run. The selected-entry screen, plaintext export
confirmation and import confirmation passed the supplied-copy checks. These are
heuristics, not tests with real users.

Actual app screenshots and native widget bounds are retained in
`target/ui-review-final/`. The app's `--demo --capture <directory>` helper requests
an egui viewport screenshot and writes `screen.png` and `bounds.json`, then exits.
Capture is accepted only with synthetic demo data. No personal vault was opened.

Reviewed views: entry list/detail, editor, password generator, passphrase generator,
settings, health, locked/create forms and import/export dialogs. Main layout was
captured at 1200×800, the supported minimum of approximately 900×650, 1440×864
and 1536×864 logical pixels, with Windows scaling at 1.25. A requested 1728-wide
window was clamped to 1536 by the available desktop; this is not a 1728 capture.

Concrete findings fixed:

- Eight competing detail actions were reduced to Copy password, Edit and More.
- Sidebar text overlap and inconsistent navigation alignment were removed.
- Search now has an actual 44px target; normal desktop controls use 36px targets.
- Group creation sits beside its heading; group/tag content scrolls.
- Navigation/footer space is reserved explicitly, preventing Settings/Lock overlap
  at the minimum window size.
- Field-copy actions have normal button targets and specific labels.
- Master/editor fields have associated labels; create/unlock forms can scroll.
- Dialog backgrounds are disabled; Cancel gets initial focus, Escape cancels,
  and saved focus is returned on closing.
- Native inactive control outlines use a contrast override over the catalog tokens.

Eidova `inspect-spacing` was run with actual native geometry. Its final six-state
report returned **0 critical and 0 serious findings**. It retained 60 moderate
findings, primarily repeated recommendations for 44px touch targets on this
36px desktop interface. Its aggregate score was 0/100; it is not a visual or
accessibility approval. The raw summary is
`target/ui-review-final/eidova-spacing.json`.

The 184px sidebar matches the supplied compact-density role. Offscreen and partly
clipped navigation controls in intentional scroll areas are excluded from target
measurement. Bounds cover instrumented widgets, not every native UI element.
No DOM or computed CSS was fabricated for this egui application.

Source-color contrast calculations: muted text on the dark panel 7.00:1, primary
button text 8.99:1, and the inactive control outline against the card fill at least
3.06:1. These are calculations from the colors used by the renderer, not a full
pixel-sampled contrast audit. Disabled controls are intentionally muted.

## Verification limits

The Windows computer-use bridge failed with a native-pipe error. App-native
screenshots worked; OS pointer/keyboard walkthroughs and an accessibility-tree
or screen-reader test were not completed. Escape has a controller-level egui
input test. Source focus logic and screenshots do not establish full keyboard
or screen-reader accessibility. Text zoom, long imported data, open nested menus,
OS clipboard expiration/history and OS focus-loss behavior need manual checks.

No mobile screen or public web dashboard exists here. The 390px web capture and
public HTTPS dashboard review are not applicable to this native credential app.
Linux/macOS builds, installation, signing, independent security auditing and
complete KeePassXC feature parity were not verified. REA research limitations and
excluded reference capabilities are documented in `reference-research.md`.

Secret cleanup is best effort. Clipboard history, swap/crash dumps and transient
GUI/library copies may retain secrets. Older encrypted backups can retain deleted
data and may require an older master password. CSV export is plaintext and is not
a complete vault backup. See README for the implemented feature limits.
