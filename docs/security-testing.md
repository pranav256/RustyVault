# Security regression checks — 2026-11-28

Tested the working tree on Windows x64 with synthetic credentials and temporary
vaults. No personal vaults or live websites were tested. Existing unrelated edits
were preserved.

## Verified results

| Check | Result |
| --- | --- |
| `cargo test --all-targets --locked` | 35 passed, no failures |
| Extension tests with the installed Playwright/Chromium runtime | 30 passed, no failures or skips |
| Rust formatting, JavaScript syntax and Git whitespace checks | Passed |
| OSV lookup for registry packages in `Cargo.lock` | 511 package/version pairs checked |

The Rust tests include 768 reproducible malformed-CSV inputs; quoted and Unicode
credential round trips; the 10,000-row import boundary; ciphertext-byte and nonce
tampering; truncated ciphertext; incorrect keys and passwords; invalid inner and
outer vault formats; oversized vault rejection; framing truncation and the 64 KiB
message boundary; all 16 generator class combinations with both ambiguity settings
and four lengths; and 30 updates against the 20-revision history limit.

The 22 real Chromium cases exercise the actual DOM getters and submit events,
including safe relative URLs, default ports, HTTP and foreign origins, external
form-associated controls, image submitters, named form controls, and effective
submit-button destinations. Rejected fills are checked to leave the password
field unchanged. All browser requests are intercepted with synthetic responses.

## Findings corrected

- Form controls can shadow `action`, `elements`, and `querySelectorAll` properties
  on a form. The form guards now read the native action getter and use document
  queries filtered by actual form association.
- `form.elements` omits image submitters. Destination checks now include image
  controls and external submitters associated with the form.
- A submitter's `formAction` property can resolve to the document URL even without
  an override attribute. Submission capture now checks for the attribute before
  choosing between its destination and the form's native destination.
- The password generator's all-disabled fallback now honors ambiguity exclusion.
- The earlier CSV escaping changed legitimate usernames and passwords on import.
  It was removed. Regression tests preserve leading `+`, `-`, `@`, `=`, apostrophes
  and whitespace exactly. The export dialog and README now warn that CSV values
  may be spreadsheet formulas. Open migration exports as text, never directly in
  a spreadsheet. CSV formula execution is a remaining consumer-side risk.
- Updated `url` from 2.5.8 to 2.5.9, with `form_urlencoded` 1.2.3, and raised the
  manifest's minimum `url` version. The new lockfile no longer matches
  RUSTSEC-2026-0592. RustyVault's HTTPS-only guard also rejects nested `blob:` input;
  the regression uses 10,000 prefixes. No exploit through that guard was found.

## Dependency findings remaining

The post-update OSV scan still reports RUSTSEC-2026-0194 and RUSTSEC-2026-0195 for
`quick-xml` 0.30.0. Its dependency path is through the Linux/Wayland procedural-macro
code generator `wayland-scanner` 0.31.11; the separate `quick-xml` 0.41.0 copy is
already patched. No newer stable scanner version was available when checked.
This is a build-tool dependency finding, not a demonstrated Windows runtime
exploit. Linux builds were not run.

Maintenance warnings also remain for `chrono`, `paste`, `ttf-parser`,
`zbus_names`, `zvariant`, and `zvariant_derive`. The scan does not establish an
exploitable RustyVault vulnerability in those packages. Major dependency
migrations were not attempted. Raw OSV responses and the final package inventory
are retained under the ignored `target/security/` directory.

## Rerunning

```powershell
cargo fmt --all -- --check
cargo test --all-targets --locked
npm test --prefix browser-extension
git diff --check
```

`dom.test.js` uses an already-installed `playwright` package and otherwise reports
a skip. Set `NODE_PATH` to its installed package directory if needed. If its default
browser is unavailable, set `CHROMIUM_EXECUTABLE_PATH` to an installed Chromium
executable. No project dependency was added for browser testing. Require no skips
when repeating the full browser run.

The v0.1.1 release preparation subsequently rebuilt both Windows binaries with
`cargo build --release --bins --locked`. The optimized app completed a synthetic
demo smoke test and rendered its CSV export confirmation. Release naming and
all-platform publication gating are covered by three Python regression tests.

Not performed locally: installed-extension end-to-end checks in Firefox/Edge, Linux/macOS
builds, interactive desktop workflows, memory-dump
analysis, or sustained coverage-guided fuzzing. Clippy is not installed in the
available Rust toolchain. Passing these bounded tests is not a security certification.
