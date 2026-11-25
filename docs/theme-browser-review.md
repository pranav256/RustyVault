# Theme previews and browser integration review

Reviewed on Windows x64, November 25, 2026. This work preserves the existing
Rust/egui application, vault format, input padding and credential flows.

## Theme selection

The complete Eidova bundled catalog was paginated: 152 entries. Its snapshot in
`assets/themes/catalog.json` retains names, categories, licenses and source URLs.
The shortlist is subjective: it favors readable credential forms, restrained
surfaces, and enough variety for comparison. It is not an objective ranking.

Ten original token sets and design notes are installed in `assets/themes/`:
Artistic, Linear (linear-app), GitHub, Raycast, Vercel, Supabase, Notion, Apple,
Minimal and Stripe. Apache-2.0 licensing and attribution are retained. These are
native adaptations, not identical recreations of the web components. Opposite
appearances, some small-text colors, opaque surfaces and fallback typography are
adapted for egui. Artistic retains serif titles; the others use bundled Inter.
Vercel uses monochrome primary buttons and blue focus/link roles.

The bottom bar previews all ten themes on every screen, with previous/next
wrapping and a direct menu. Use this theme stores the stable ID in the computer's
`theme.txt`. Restore chosen theme is in the menu. Preview does not change the
vault or save a new theme preference. Dark/light remains independent in
`appearance.txt`. Demo changes stay in memory and demo startup reads no real
appearance preference or personal vault.

Fields retain 12px horizontal and 8px vertical text padding. A real screenshot
exposed a group/navigation overlap at 900x650 after adding the bottom bar. The
group scroll area now permits a minimum below egui's default and the compact
sidebar spacing reserves a visible row above its fixed footer.

## Rendered and source evidence

Final native captures are under `target/ui-review-themes/`:
all 20 theme/appearance pairs at 900x650 logical pixels, settings at 1200x800
and 900x650, and editor, locked and confirmation states at 900x650. Windows scale
is 1.25; the minimum height rounds to 650.4 logical pixels. The contact sheet
combines the actual captured images. Source colors, focus/selection states and
control text were also checked directly.

Eidova color, components, spacing, forms and microcopy rules informed the change.
Final supplied-copy checks for the theme controls and expanded OTP explanation
have no blocking findings. The OTP field is optional and explains that it
generates a website's rotating code, rather than adding MFA to vault unlock.
It also explains the tradeoff of keeping the password and OTP secret together.

Five final geometry reports cover the 25 captures with partial instrumentation
and off-viewport scroll contents excluded. They report **0 critical, 0 serious,
344 moderate findings**. Moderate findings repeat the recommendation for 44px
touch targets on existing 36px desktop controls. Aggregate scores are 0/100;
these reports are not visual or accessibility approval. Raw findings are in
`eidova-spacing-1.json` through `eidova-spacing-5.json` beside the captures.

## Browser implementation and checks

The shared MV3 package has a service worker for Chromium browsers and module
background scripts for Firefox. Firefox has a fixed add-on ID. The bridge
accepts Chromium's caller-origin argument and Firefox's manifest-path/add-on-ID
argument order. The installer generates separate native-host manifest schemas
and registers Firefox, Chrome, Edge and Brave for the current Windows user.

The content script checks an exact-origin approval before attaching its
submission listener. The background checks that approval and browser permission
again before staging a submitted credential. Browser host permissions cover all
ports on a hostname, so browser permission alone is deliberately insufficient.
Only origin grants go into extension storage. Passwords are staged in memory
for up to a minute and can disappear earlier when a background context suspends.

Verification completed:
- 22 Rust tests, including all theme/appearance contrast pairs, ID round trips,
  preview preservation, vault protection, native framing and exact-origin access.
- Five Node extension tests, including unapproved ports, revoked permission,
  navigation during authorization, and both manifest background environments.
- A real Brave action popup in a temporary test profile listed and filled a
  synthetic login on an intercepted HTTPS fixture and sent a submitted login to
  the save-review path. The desktop/native response was mocked in this browser
  test. The real popup was opened with action.openPopup and inspected through
  its CDP target; it was not a normal browser tab.
- Both release native-bridge calling conventions returned correctly framed
  replies. Untrusted caller origins/IDs exited with no reply.
- The installer passed syntax, WhatIf, and isolated registry-mocked checks for
  all four registrations and both native-host manifest schemas.
- Formatting and git diff whitespace checks passed. Both executables built
  with warnings denied. No dependencies were added for these changes.

Firefox and Chrome are not installed on this machine. Full native-messaging
save/fill was not exercised end to end with the real desktop confirmation in
any browser. Edge was not walked through. Browser setup was not installed into
the user's normal profile and no native-host registry key was changed by tests.
Firefox's temporary extension must be reloaded after restart; signing and store
publication are not included. Opera/Vivaldi compatibility is unverified and
Safari is not implemented by this Windows connection. Clippy, screen readers,
text zoom, OS theme synchronization and macOS/Linux rendering were not checked.

Browser setup and official implementation references are in
`browser-extension/README.md`.

Final executable SHA-256:
- rusty-vault.exe: 361d8c8d4642dcd535f4063c24e62b99dbb3c0799898f6a7e8e3ddc80f359273
- rustyvault-bridge.exe: b6bf1943e4b860a5c886236e6cea1791a0ef3a60c42f8564266460ad9b7f91fd


## Earlier selection (superseded)

The user chose Stripe after the comparison. Stripe is now the default for
normal and demo startup. The ten-theme menu and light/dark switch remain.
The capture evidence and executable hashes above describe the comparison build
before this default-selection change.

## Current selection: Raycast only

The user replaced Stripe with Raycast only. Alternate runtime palettes, the
comparison bar, theme menu, demo design override and theme-ID preference reader
were removed. Raycast retains light/dark appearances, shared text-input padding,
and the existing security and browser behavior. Old catalog sources remain
archived reference assets; the comparison screenshots above are historical.

The OTP detail label now says Website verification code and explains its purpose
on hover. Existing OTP secrets and generation are preserved. The feature is
optional and does not change the master-password unlock flow.

Raycast-only checks passed: 22 Rust tests, formatting and whitespace checks,
and a release build with warnings denied. Native captures cover dark/light at
900x650, settings at 1200x800 and the light lock screen at 900x650, under
`target/ui-review-raycast-only/`. No browser UI flow was rerun for this change.
