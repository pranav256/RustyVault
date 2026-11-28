use std::io;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BrowserKind {
    #[default]
    Brave,
    Chrome,
    Edge,
    Firefox,
}

impl BrowserKind {
    pub const ALL: [Self; 4] = [Self::Brave, Self::Chrome, Self::Edge, Self::Firefox];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Brave => "Brave",
            Self::Chrome => "Chrome",
            Self::Edge => "Edge",
            Self::Firefox => "Firefox",
        }
    }

    pub fn export_instructions(&self) -> &'static str {
        match self {
            Self::Brave => "In Brave Password Manager, open Settings. Beside Export passwords, select Download file and save the CSV. Return here and choose that file. Repeat for each browser profile you use.",
            Self::Chrome => "In Google Password Manager, open Settings. Beside Export passwords, select Download file and save the CSV. Return here and choose that file. Repeat for each browser profile you use.",
            Self::Edge => "In Edge, press Alt+F and select Passwords. Open the three-dot menu, select Export passwords, confirm and save the CSV. Return here and choose that file. Repeat for each browser profile you use.",
            Self::Firefox => "In Firefox Passwords, open the three-dot menu and select Export Passwords or Export Logins. Confirm and save the CSV. Return here and choose that file. Repeat for each browser profile you use.",
        }
    }

    #[cfg(any(windows, test))]
    fn password_manager_url(&self) -> &'static str {
        match self {
            Self::Brave => "brave://password-manager/settings",
            Self::Chrome => "chrome://password-manager/settings",
            Self::Edge => "edge://settings/autofill/passwords",
            Self::Firefox => "about:logins",
        }
    }

    pub fn open_password_manager(&self) -> io::Result<()> {
        #[cfg(windows)]
        {
            std::process::Command::new(self.executable()?)
                .arg(self.password_manager_url())
                .spawn()?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Open your browser's password manager manually, export CSV, then choose the file here.",
            ))
        }
    }

    pub fn setup_connection(&self) -> io::Result<PathBuf> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use std::process::{Command, Stdio};

            let executable = std::env::current_exe()?;
            let app_directory = executable.parent().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Cannot locate RustyVault's installation.",
                )
            })?;
            let bridge = app_directory.join("rustyvault-bridge.exe");
            let extension = app_directory.join("browser-extension");
            if !bridge.is_file() || !extension.join("manifest.json").is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Browser connection files are missing. Use the complete RustyVault installation with rustyvault-bridge.exe and browser-extension.",
                ));
            }
            let system_root = std::env::var_os("SystemRoot").ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Cannot locate Windows registry setup tools.",
                )
            })?;
            let directory = extension.join("native-hosts");
            std::fs::create_dir_all(&directory)?;
            let (registry_key, filename) = self.native_registration();
            let manifest_path = directory.join(filename);
            let manifest = serde_json::to_vec_pretty(&self.native_manifest(&bridge))?;
            std::fs::write(&manifest_path, manifest)?;
            let status = Command::new(PathBuf::from(system_root).join("System32/reg.exe"))
                .arg("add")
                .arg(registry_key)
                .args(["/ve", "/t", "REG_SZ", "/d"])
                .arg(&manifest_path)
                .arg("/f")
                .creation_flags(0x08000000)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
            if !status.success() {
                return Err(io::Error::other(format!(
                    "Could not register the RustyVault connection for {} for the current Windows user.",
                    self.label()
                )));
            }
            Ok(extension)
        }
        #[cfg(not(windows))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Automatic browser connection setup is available on Windows. Browser CSV import still works here.",
            ))
        }
    }

    #[cfg(windows)]
    fn executable(&self) -> io::Result<PathBuf> {
        let relative = match self {
            Self::Brave => "BraveSoftware/Brave-Browser/Application/brave.exe",
            Self::Chrome => "Google/Chrome/Application/chrome.exe",
            Self::Edge => "Microsoft/Edge/Application/msedge.exe",
            Self::Firefox => "Mozilla Firefox/firefox.exe",
        };
        ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(|root| PathBuf::from(root).join(relative))
            .find(|path| path.is_file())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{} was not found in its usual installation locations. Open it manually and follow the export instructions.", self.label()),
                )
            })
    }

    #[cfg(windows)]
    fn native_registration(&self) -> (&'static str, &'static str) {
        match self {
            Self::Brave => (
                r"HKCU\Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.pranav256.rustyvault",
                "chromium.json",
            ),
            Self::Chrome => (
                r"HKCU\Software\Google\Chrome\NativeMessagingHosts\com.pranav256.rustyvault",
                "chromium.json",
            ),
            Self::Edge => (
                r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.pranav256.rustyvault",
                "chromium.json",
            ),
            Self::Firefox => (
                r"HKCU\Software\Mozilla\NativeMessagingHosts\com.pranav256.rustyvault",
                "firefox.json",
            ),
        }
    }

    #[cfg(windows)]
    fn native_manifest(&self, bridge: &std::path::Path) -> serde_json::Value {
        let mut manifest = serde_json::json!({
            "name": "com.pranav256.rustyvault",
            "description": "RustyVault browser connection",
            "path": bridge,
            "type": "stdio",
        });
        if *self == Self::Firefox {
            manifest["allowed_extensions"] = serde_json::json!(["rustyvault@pranav256.com"]);
        } else {
            manifest["allowed_origins"] =
                serde_json::json!(["chrome-extension://mbogmhfmgcaindpokcpgimbaefjoppgb/"]);
        }
        manifest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_choices_default_to_brave_and_use_local_password_pages() {
        assert_eq!(BrowserKind::default(), BrowserKind::Brave);
        assert_eq!(
            BrowserKind::ALL.map(|browser| browser.label()),
            ["Brave", "Chrome", "Edge", "Firefox"]
        );
        for browser in BrowserKind::ALL {
            assert!(browser.export_instructions().contains("CSV"));
            assert!(!browser.password_manager_url().starts_with("http"));
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_manifests_keep_the_shipped_extension_allowlists() {
        let bridge = std::path::Path::new(r"C:\RustyVault\rustyvault-bridge.exe");
        for browser in BrowserKind::ALL {
            let manifest = browser.native_manifest(bridge);
            assert_eq!(manifest["name"], "com.pranav256.rustyvault");
            assert_eq!(manifest["path"], bridge.to_str().unwrap());
            assert!(browser
                .native_registration()
                .0
                .starts_with(r"HKCU\Software\"));
            if browser == BrowserKind::Firefox {
                assert_eq!(
                    manifest["allowed_extensions"],
                    serde_json::json!(["rustyvault@pranav256.com"])
                );
                assert!(manifest.get("allowed_origins").is_none());
            } else {
                assert_eq!(
                    manifest["allowed_origins"],
                    serde_json::json!(["chrome-extension://mbogmhfmgcaindpokcpgimbaefjoppgb/"])
                );
                assert!(manifest.get("allowed_extensions").is_none());
            }
        }
    }
}
