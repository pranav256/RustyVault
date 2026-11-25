# Raycast UI polish

Reviewed on Windows x64, November 25, 2026.

The user requested a fresher UI and had already selected Raycast only. This
pass retains its charcoal surfaces, Inter and coral key mark. Blue now indicates
interaction and focus; green indicates healthy passwords. Neutral selection,
compact credential rows, aligned native navigation counts and trailing copy
actions replace the former pervasive red outlines and large field slabs.
Settings keep their existing order and operations in consistently padded groups.
Inputs retain 12px horizontal / 8px vertical padding and 44px minimum height.
Native checkbox outlines and the standalone unlock input are visible in both
appearances. No data format or browser protocol changed.

Rendered synthetic demo captures and measured egui bounds are in
`target/ui-review-polish/`. Dark detail and settings were captured at 900x650
and 1200x800 logical pixels, with Windows scale 1.25. Light detail, editor,
generator and unlock were also reviewed at 900x650. The export confirmation
was reviewed in the first rendering round; its content and behavior are retained.
Long forms and sidebar groups scroll within their existing viewports.

The first rendering round exposed misaligned copy controls, faint unchecked
controls and a crowded unlock form. The second round confirms their correction.
Copy username and copy website share the same right edge at both tested widths.
The minimum-size sidebar displays the full Password health label.

Eidova received six states of measured native geometry, excluding hidden
off-viewport items. It returned **0 critical, 0 serious and 60 moderate** findings,
all recommending 44px touch targets for compact 32-40px desktop controls.
These were retained for desktop density and native keyboard access. The geometry
score is 0/100, not an accessibility or visual approval. Its mobile-width
suggestion is outside this native app's 900px minimum width; larger suggested
widths were not checked. Evidence: `target/ui-review-polish/eidova-spacing.json`.
Instrumentation covers selected controls, not every visible element.

All 23 Rust tests pass, including semantic contrast in both appearances and a
native egui click test verifying navigation changes views while an open editor
prevents navigation and preserves its draft. `cargo fmt --all`, `git diff --check`
and a warnings-denied release build pass. Real-vault workflows and live browser
save/autofill were not exercised during this visual pass. No claim of full
accessibility approval or touch optimization is made.
