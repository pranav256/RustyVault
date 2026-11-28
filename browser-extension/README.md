# RustyVault for Firefox, Chrome, Edge and Brave

The MV3 extension connects to the Windows desktop app over a localhost-only
socket using Chrome-compatible native messaging. It matches exact HTTPS origins. Logins fill
only after you select one in the popup. The extension never submits a page form.
After a user-submitted form, the extension temporarily stages the visible login
for up to 60 seconds (less if the browser suspends the background context). Saving requires a separate confirmation in the unlocked desktop
vault. Closing the tab, waiting one minute, or declining discards the staged copy.

You grant access to each exact HTTPS origin by clicking **Enable on this site**. Nothing is transmitted outside this computer. Passwords enter
the extension process only to fill the selected form or request a save. The
browser may also retain extension service-worker memory until it suspends
the worker. The extension does not read browsing history. After you enable a site, it watches
submitted login forms on that exact origin, including on future visits.

## Install

1. Extract the Windows release archive, or build both executables from source:
   `cargo build --release`.
2. Run `powershell -File .\browser-extension\install-native-host.ps1`.
   The script writes separate Firefox and Chromium manifests in `native-hosts/` and registers
   them for the current Windows user in Firefox, Chrome, Edge and Brave.
3. In Firefox, open `about:debugging#/runtime/this-firefox` and choose **Load Temporary Add-on**, selecting `manifest.json`. In Chrome, Edge or Brave, open the extensions page, turn on **Developer mode**, and select **Load unpacked**. Choose the `browser-extension` directory.
4. Unlock RustyVault (and reload the extension if it was already open). In **Settings**, enable **Connect to the
   RustyVault extension**. This opt-in setting is encrypted with the vault and is
   off by default. Demo mode never opens the browser connection.
5. Open the extension popup on a site, grant access for that HTTPS site, then
   choose a matching login. To save, submit a login on the site and choose **Save
   login** from the popup. Confirm the website and account in RustyVault.

The v0.1.1 release and source-checkout scripts are unsigned. Windows release
signing is planned after SignPath approval; see the
[Code signing policy](../README.md#code-signing-policy). For a future signed
release, verify the scripts' Authenticode signatures and use your normal
PowerShell execution policy. Unsigned source scripts require a trusted development
environment whose policy permits them; a signed release does not sign your local
checkout.

This unpacked extension has no store listing. Firefox's temporary add-on must be
loaded again after Firefox restarts. Native host and settings
files are created only when you run the installer and opt in. The extension can
be removed from the browser's extensions page; the installer creates the
manifests in `native-hosts/` and registers Firefox, Chrome, Edge and Brave.
See the native-messaging manifest and local-only session transport in this folder
and `src/browser.rs`.


Run extension checks using `npm test --prefix browser-extension`.

## Uninstall

Disable **Connect to the RustyVault extension** in the app, then lock and close
RustyVault. Remove the extension from each browser's extensions page. From the
same extracted release or source directory used for installation, run this in
PowerShell to remove the current user's native-host registrations and generated
manifests:

```powershell
foreach ($vendor in @('Mozilla', 'Google\Chrome', 'Microsoft\Edge', 'BraveSoftware\Brave-Browser')) {
    $key = "HKCU:\Software\$vendor\NativeMessagingHosts\com.pranav256.rustyvault"
    if (Test-Path -LiteralPath $key) {
        Remove-Item -LiteralPath $key
    }
}
if (Test-Path -LiteralPath '.\browser-extension\native-hosts') {
    Remove-Item -LiteralPath '.\browser-extension\native-hosts' -Recurse
}
```

You can then remove the extracted application files. These steps preserve your
encrypted vault, backups and custom vault files; do not delete those files or
RustyVault's OS data directory unless you also intend to erase your saved data.

## Browser coverage and limits

| Browser | Package | Setup |
| --- | --- | --- |
| Firefox 128+ | Shared MV3 package, background script | Temporary add-on plus Mozilla native host |
| Chrome 121+ | Shared MV3 package, service worker | Load unpacked plus Chrome native host |
| Edge 121+ | Shared MV3 package, service worker | Load unpacked plus Edge native host |
| Brave (current Chromium) | Shared MV3 package, service worker | Load unpacked plus Brave native host |

Other Chromium browsers may work with the same extension, but Opera and Vivaldi
have not been verified. Safari is not available for this Windows native host;
it requires a separate macOS Safari extension wrapper and signing.

Enabling a site records its exact HTTPS origin (scheme, hostname and port).
Browser host permissions cover all ports on a hostname; the additional exact-origin
check runs before reading submitted credentials and again before staging them.
Only approved origin strings are stored in extension storage, never passwords.
Browser autofill means selecting a matching login in the popup; it never submits
forms or fills without that selection. Browser APIs cannot silently extract the
browser's built-in password database. To migrate existing passwords, use CSV import.

The Firefox add-on ID and Chromium public key are development identities. Store
publication and signed Firefox distribution are not included. Test with synthetic
credentials before enabling on important accounts.

Implementation references: Mozilla's native-messaging and background-manifest
documentation, and Chrome/Edge native-messaging documentation:
- https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging
- https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/manifest.json/background
- https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging
- https://learn.microsoft.com/en-us/microsoft-edge/extensions-chromium/developer-guide/native-messaging
