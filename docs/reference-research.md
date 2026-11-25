# Local password manager reference

Research date: 2026-11-15. Reference: **KeePassXC 2.8.0**, official Windows x64 portable release. RustyVault remains its own product and file format. No KeePassXC source or visual assets were copied into the application.

## Why this reference

KeePassXC directly fits the chosen local desktop scope: creating, editing, organizing, and saving credentials in an encrypted file without an account or server. Bitwarden is a useful interface reference, but its [official offline documentation](https://bitwarden.com/help/using-bitwarden-offline/) states that unlocked offline apps are read-only: adding/editing items and importing require a connection. It would be a poor functional basis for an independently usable local vault.

## Artifact and evidence

Official release: <https://github.com/keepassxreboot/keepassxc/releases/tag/2.8.0>

| Artifact | Identity |
| --- | --- |
| `KeePassXC-2.8.0-x64.zip` | 70,348,471 bytes; SHA-256 `48ff4f088e9ce70e241da1270e5081528dba77c38b17dae4aa6690ffc50fc828` |
| `KeePassXC.exe` | 6,195,288 bytes; SHA-256 `72200bfe1a6c59162bee904832f3b292ede45325effcef54060c6aff836dd28d` |
| `keepassxc-cli.exe` | 2,755,672 bytes; SHA-256 `ff737226e5fac1517652781908e5fbac3e626ad45215db63b6ee86ef2aa75c37` |

The ZIP digest matches the release's `.DIGEST` file. This verifies consistency with that published digest; it is not an independent signature verification.

Actual REA 6.7.0 evidence:

* `ev_5a026332ff01c00c9d430b223c7e859b56592363f0f08a9281cea2158d815c97`, `inspect_artifact`: ZIP central-directory inventory. The retained manifest reports 145 nodes, 174 occurrences, and 173 edges. This is static package inventory, not observed execution.
* `ev_472ba816e57537a4b1d4ef348b73d8df1f2ff94c9e89c776da68f0776941a64b`, `inspect_pe_resources`: native x86_64 PE32+ resource inventory; complete for its bounded resource-table walk, 40 examined entries and 18 resources. Payloads other than icon-group relationships remain opaque. This does not recover application logic.

The active session had no REA MCP tools. `rea doctor --client codex --json` confirmed missing registration and no native analysis engine. The engine-free CLI worked; no client configuration or analysis engine was installed. `rea capture-process` explicitly rejects Windows PTY capture because descendant cleanup is unsupported. Those failures have no runtime Evidence IDs.

An ordinary PowerShell observation of the shipped CLI, separately from REA, confirmed its `help` command inventory and a generator self-check: `generate -L 24 -l -U -n -s --every-group --exclude-similar` returned 24 characters containing each requested character class. This checks one generator invocation only. No personal database or credentials were opened.

## Documented workflows to reconstruct

These behaviors come from the [official user guide](https://keepassxc.org/docs/KeePassXC_UserGuide), not native decompilation or comprehensive runtime testing.

| Area | Reference behavior | Useful RustyVault behavior |
| --- | --- | --- |
| Main window | Groups; searches/tags; entry list; interactive selected-entry preview; database tabs | Persistent navigation, searchable list, detail pane, quick copy/edit actions |
| Organization | Nested groups; multiple tags per entry; dynamic/saved searches | Groups and tags that persist in the encrypted vault; useful filters with counts |
| Entries | Title, username, password, URL, notes; custom attributes; attachments; created/modified/accessed times | Complete entry editor and selected-entry detail, with required-field validation and masked secrets |
| Search | Field-aware searches, quoted phrases, regular expressions, tag and state filters | Case-insensitive text search plus group/tag/favorite/health filters; show a clear empty result |
| Entry lifecycle | New, edit, clone, delete to recycle bin, restore, permanently delete | Recoverable trash and explicit confirmation for permanent deletion |
| History | Snapshot the previous entry before modification; read-only review, restore, delete history | Encrypted previous versions; restoring should itself preserve the current version |
| TOTP | Enter a shared secret; configure digits/period/algorithm; show current code and copy; QR export | Validate a Base32 secret or `otpauth://` URI; current code, remaining time, explicit reveal/copy |
| Password generator | Configurable length and character sets; exclusions; ensure each class; entropy estimate | Reuse cryptographic randomness; prevent an empty alphabet; clearly explain configuration |
| Passphrase generator | Word count, separator, casing, custom wordlists; EFF large list by default | A real full wordlist, random independent word selection, and entropy based on list size |
| Reports | Weak/expired password views; password analysis | Local weak, reused, and expiring-password checks; no online exposure implied |
| Saving | Atomic safe saves by default; optional pre-save backup; on-demand encrypted backup | Commit visible changes only after a successful save; keep encrypted backups and actionable errors |
| Vault security | Master-password/key-file settings; encryption/KDF settings | Change master password by writing successfully under the new key before replacing the active key |
| Locking | Manual lock; idle lock; lock on OS screen lock or user switch | Release decrypted vault/key and erase editor, generator, search/detail secrets when locked |
| Clipboard | Copy fields; automatically clear copied sensitive content | Clear after a timeout only if the clipboard still contains the value this app copied |
| Import | CSV mapping and preview; several vendor JSON/XML formats; choose new or existing database | Validate the whole import before changing the vault, merge rather than overwrite, then save atomically |
| Export | CSV/XML/HTML have explicit format limitations; plaintext export warning | Explicit plaintext confirmation; complete structured export for re-import; encrypted backup as the safer default |

The guide establishes tags and searches, but this investigation did **not** establish a separate built-in favorites feature. RustyVault may offer favorites as its own useful organizational choice.

## Concrete reference defaults

The tagged [2.8.0 configuration source](https://github.com/keepassxreboot/keepassxc/blob/2.8.0/src/core/Config.cpp) documents the following defaults. This is source evidence, separately from shipped-artifact Evidence:

* Automatic save after changes: enabled.
* Clipboard clearing: enabled, 10 seconds.
* Idle locking: enabled, 900 seconds.
* Lock on screen lock and user switch: enabled.
* Password generator: 20 characters, lowercase/uppercase/numbers/special characters, ensure each group, exclude similar characters.
* Passphrase generator: 7 words from `eff_large.wordlist`, space separator, lowercase. The EFF large list has 7,776 words; uniform word selection provides about 12.9 bits per word. A tiny decorative word list is not an equivalent generator.

Keep these configurable only where needed. Independently implement behavior; do not paste the reference's GPL source into an MIT project.

## Screens and documentation links

* [Application layout](https://keepassxc.org/docs/KeePassXC_UserGuide#_application_layout): Figure 1, main database interface.
* [Entry editor](https://keepassxc.org/docs/KeePassXC_UserGuide#_editing_an_entry).
* [TOTP setup and use](https://keepassxc.org/docs/KeePassXC_UserGuide#_adding_totp_to_an_entry).
* [History review and restoration](https://keepassxc.org/docs/KeePassXC_UserGuide#_history).
* [Encrypted backups](https://keepassxc.org/docs/KeePassXC_UserGuide#_database_backup_options).
* [Password generator](https://keepassxc.org/docs/KeePassXC_UserGuide#_password_generator).
* [Passphrase generator](https://keepassxc.org/docs/KeePassXC_UserGuide#_generating_passphrases).
* [CSV import preview](https://keepassxc.org/docs/KeePassXC_UserGuide#_importing_csv_file).
* [Export formats and limitations](https://keepassxc.org/docs/KeePassXC_UserGuide#_exporting_databases).

Official guide screenshots were extracted from its embedded PNGs into the temporary research directory: `main-interface.png`, `edit-entry.png`, `password-generator.png`, and `passphrase-generator.png`. They are published reference images, not live screenshots of this investigation.

## Reproducing the artifact inspection

All downloads and raw Evidence are retained in this session's temporary directory:

```text
C:/Users/Pranav/AppData/Local/Temp/RustyVault-KeePassXC-20dcb8dbba8c4a98b1feb121418cb591
```

Run against an independently downloaded official release:

```text
npx -y rea-agents@6.7.0 inspect-artifact <path-to-KeePassXC-2.8.0-x64.zip> --format json
npx -y rea-agents@6.7.0 inspect-pe-resources <path-to-KeePassXC.exe> --format json
```

Raw records are `rea-artifact.json` and `rea-pe-resources.json`. The executable for an optional separately controlled native UI investigation is `portable/KeePassXC-2.8.0-x64/KeePassXC.exe`. Its documented `--allow-screencapture` flag permits screenshots; screen capture is protected by default on Windows.

## Boundaries and remaining unknowns

This investigation answers the reference choice, artifact identity, documented workflows, default settings, and one shipped CLI generator check. It does **not** establish full binary logic, every workflow at runtime, or exact all-feature parity.

Full KeePassXC parity would also include KDBX interoperability and database merging, multiple database tabs, key-file and hardware/biometric unlock, global Auto-Type, SSH-agent/Secret Service integration, and passkeys. Those are separate substantial capabilities; browser integration/cloud services are outside the local scope selected for RustyVault. A local reconstruction must disclose capabilities it has not implemented instead of calling itself an exact clone.
