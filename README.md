# RustyVault

A local desktop password manager written in Rust. Credentials, notes, attachments,
settings and credential history are stored in an encrypted vault
using Argon2id and AES-256-GCM.

## Run it on Windows

From `D:\Projects\RustyVault`, launch the built executable:

```powershell
.\target\release\rusty-vault.exe
```

To try the redesigned interface with synthetic data:

```powershell
.\target\release\rusty-vault.exe --demo
```

Demo changes stay in memory. Import, export, attachment file operations and master
password changes are disabled in demo mode. Closing it discards changes.

Raycast is the only theme, starting in dark mode. Use **Light mode / Dark mode** in the footer
or **Settings → Appearance** to switch. Outside demo mode, the preference is
saved on this computer and also applies before unlocking. The appearance choice is stored outside the encrypted vault. Browser connection
metadata also lives outside it while the connection is active; passwords remain
in the encrypted vault until you request a fill. To preview the light demo, run
`.\target\release\rusty-vault.exe --demo --theme light`.

The theme comparison bar has been removed. Raycast retains both light and dark
appearances. The selected theme applies to the lock screen, forms and dialogs.

A Git checkout and GitHub's **Source code** archives contain source, not an EXE.
The executable above appears after a successful release build. Published archives
are available from [GitHub Releases](https://github.com/pranav256/RustyVault/releases),
but older releases may predate these features.

## Build from source

Install [Rust](https://rustup.rs/) and your platform's build tools:

- **Windows:** [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  with **Desktop development with C++**, including MSVC and a Windows SDK.
- **macOS:** `xcode-select --install`.
- **Ubuntu/Debian:** `sudo apt install build-essential pkg-config libx11-dev libxcursor-dev libxi-dev libxrandr-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev`.

On Windows, use MSVC for this project:

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup override set stable-x86_64-pc-windows-msvc
cargo run --release
```

This fixes **`error calling dlltool 'dlltool.exe': program not found`** caused by
using the Windows GNU toolchain without MinGW. The override changes this project's
toolchain; it leaves your global default alone. Reopen your terminal after installing
Rust or the C++ tools. `rustup show active-toolchain` should report MSVC here.

On other platforms, run `cargo run --release` from the directory containing
`Cargo.toml`. For a build without launching, use `cargo build --release`.
The first build downloads dependencies and can take several minutes.

The binaries are `target/release/rusty-vault.exe` on Windows and
`target/release/rusty-vault` on Linux/macOS. They open a native window and require
a graphical desktop session. Browser integration is optional and local-only; it
requires the desktop app, an explicit site permission and the unlocked vault.

## Code signing policy

The SignPath Foundation application has been submitted and is awaiting review.
No SignPath account, certificate or API token is configured, and
**v0.1.1 remains unsigned**. We intend to use free code signing from
[SignPath.io](https://signpath.io/) with a certificate from
[SignPath Foundation](https://signpath.org/), subject to approval; this is not a
claim of current sponsorship.

Committers, reviewers and signing approvers: [@pranav256](https://github.com/pranav256).
The planned signing policy requires manual approval of each signing request,
SHA-256 signatures and RFC 3161 timestamps. It covers the two Windows executables
(`rusty-vault.exe`, `rustyvault-bridge.exe`) and the two browser setup scripts
(`install-native-host.ps1`, `install-brave.ps1`). macOS signing and browser-store
signing are unchanged and are not part of this setup.

After enrollment is approved:

1. Configure SignPath's GitHub trusted build system for this repository, using
   GitHub-hosted builds and uploaded GitHub Actions artifacts.
2. Upload `.github/signpath-artifact-configuration.xml` as the project's artifact
   configuration and require manual signing approval.
3. Add the GitHub Actions secret `SIGNPATH_API_TOKEN` and repository variables
   `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`,
   `SIGNPATH_SIGNING_POLICY_SLUG` and `SIGNPATH_ARTIFACT_CONFIGURATION_SLUG`.

See the [Foundation conditions](https://signpath.org/terms) and
[GitHub trusted-build setup](https://docs.signpath.io/trusted-build-systems/github).

**Privacy:** Vault storage and password checks run locally. RustyVault has no
telemetry, breach-service lookups or cloud sync. Optional browser integration uses
native messaging and a loopback connection to the desktop app; credentials reach
an authorized HTTPS page only when you choose to fill them. Signing concerns
release artifacts and build metadata, not users' vault files or master passwords.

## First use

1. Choose and confirm a master passphrase with **at least 12 characters** and
   select **Create vault**. Existing vaults with shorter passwords remain unlockable.
2. Select **Add password** (or press **Ctrl+N**), enter the website or account name, username/email and password, then select **Save password**. If you enter an account name, add its website for browser filling or leave it blank for a local account. Optional details can be added later.
3. On later launches, enter the master password and select **Unlock vault**.

Use **Import from browser...** to bring in saved passwords from Brave, Chrome, Edge or Firefox. The guide opens the browser's password manager, explains CSV export and lets you review accounts before importing. Review shows new entries and exact duplicates that will be skipped; different passwords or details are kept separately. The CSV is unencrypted; delete it after importing.

For future saves and fills, open **Settings → Set up browser extension...**. Follow the steps to prepare the connection, add the extension and allow it while the vault is unlocked. The connection status tells you when the extension has contacted the vault.

There is no password recovery. Keep the master passphrase safe.
The locked screen offers **Open another vault…** and **Create a new vault…**.
Creating requires a new filename and never replaces an existing vault.
You can also choose a file at startup:

```powershell
.\target\release\rusty-vault.exe --vault "D:\Private\personal.enc"
```

The default vault lives separately from the executable:

| OS | Default path |
| --- | --- |
| Windows | `%LOCALAPPDATA%\pranav256\rusty-vault\data\vault.enc` |
| Linux | `~/.local/share/rusty-vault/vault.enc`, respecting `$XDG_DATA_HOME` |
| macOS | `~/Library/Application Support/com.pranav256.rusty-vault/vault.enc` |

If the OS data directory cannot be determined, the app uses `vault.enc` in the
working directory. Only one RustyVault window can unlock a given vault at a time.
The application supports desktop windows of at least 900×650 logical pixels.

## Features

- Search, favorites, persistent flat groups and tags, with a list/detail layout.
- Login entries and secure notes, custom secret fields, expiry dates and encrypted
  attachments up to **5 MB each**. The encrypted vault is capped at **64 MB**.
- Random passwords with character rules and passphrases from the full 7,776-word
  EFF list. Local weak, reused and expired password checks.
- Trash, restore and confirmed permanent deletion.
- Up to **20 credential revisions** per entry. History includes title, username,
  email, password, website, notes and custom fields. Restoring preserves the
  current credential values as another revision. Attachments, groups, tags,
  favorites, type and expiry are not versioned.
- Idle lock, optional lock on window focus loss, and timed clipboard clearing.
  Clipboard clearing only removes the value this app copied if it is still there.
- Change the master password, encrypted backups, CSV import preview and export,
  plus the existing legacy `emails.txt` importer.
- Optional Windows Firefox, Chrome, Edge and Brave save/fill through an unpacked extension. Password fills
  require an explicit login choice; submitted logins need approval in RustyVault.
  See [browser extension setup](browser-extension/README.md).

**Ctrl+F** focuses search in entry views. **Ctrl+L** locks the vault. Navigation is
paused while editing; use **Save entry** or **Cancel** before changing screens.

## Backups and portability

Successful saves replace the vault atomically and keep the previous encrypted
file alongside it: `vault.enc.bak` for the default filename. A save failure leaves
visible data and the editor draft in place. A failure to create the backup also
prevents replacement of the vault. This reduces data-loss risk; it does not replace
independent backups or guarantee recovery after every hardware/filesystem failure.

Use **Settings → Encrypted backup…** for a complete copy, including attachments,
trash, history and settings. To restore safely, close/lock the app, copy the backup
to a **new `.enc` filename**, then use **Open another vault…** to verify it before
replacing your working copy. Backups made before a master-password change still
require the old password. Previous backups can contain permanently deleted entries.

CSV import accepts common columns such as `title/name/service`,
`username/login_username`, `password/login_password`, `url/login_uri`, `notes`,
`group/folder/category`, `tags` and `type/kind`.
Tags use semicolons. Unknown columns are retained as custom fields. Imports are
limited to 16 MB and 10,000 entries; preview them before merging. Legacy import
retains its original comma-separated, character-offset behavior and is not CSV.

**CSV exports are plaintext**, including passwords. Values are preserved exactly for
migration, including values that a spreadsheet could interpret as formulas. Open
them as text; do not open them directly in Excel or another spreadsheet. They omit custom fields,
attachments, history, favorites, expiry, settings and trash, so use an encrypted
backup for a complete copy. Confirmation is required before export. Exports cannot
replace the open vault, its automatic backup or lock file. OTP data in older vaults
is preserved encrypted but is no longer shown or exported.

## Checks and reference

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The local workflow reference is KeePassXC 2.8.0; the interface uses the installed
Eidova Artistic tokens, Georgia/Inter typography and Lucide. Implementation is independent,
with no KeePassXC code or assets distributed. See [reference research](docs/reference-research.md),
[design notes](DESIGN.md), [Artistic review](docs/artistic-review.md), [data validation](docs/validation.md), and
[asset attribution](assets/NOTICE.md).

This implementation has not had an independent security audit. Best-effort secret
cleanup cannot guarantee that OS clipboard history, swap, crash dumps, or all GUI
memory copies are erased. Health checks are estimates and do not query breach
services. OS screen-lock events are not monitored; use idle/focus-loss locking.

There is no KDBX interoperability, key-file/hardware/biometric unlock, passkeys,
Auto-Type or cloud sync. Browser integration currently supports Windows and
standard HTTPS login forms; it is not full KeePassXC parity.

## Uninstall

RustyVault is portable. Lock and close it, then remove the extracted application
files. If you installed the browser connection, first follow the
[browser extension uninstall steps](browser-extension/README.md#uninstall) to
remove its native-host registrations and generated manifests.

Your encrypted vault and backups are separate from the executable by default.
Keep your `.enc` and `.enc.bak` files, including any custom vault paths; do not
delete the OS data directory listed under **First use** if you want to retain them.

## License

MIT for RustyVault. Third-party assets retain their licenses.
