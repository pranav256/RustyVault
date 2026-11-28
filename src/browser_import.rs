use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn brave_export_directory() -> io::Result<PathBuf> {
    directories::UserDirs::new()
        .and_then(|dirs| {
            dirs.download_dir()
                .filter(|directory| directory.is_dir())
                .map(Path::to_path_buf)
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Your Downloads folder was not found. Choose the exported CSV manually.",
            )
        })
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CsvStamp {
    length: u64,
    modified: SystemTime,
    created: Option<SystemTime>,
}

impl CsvStamp {
    fn from_metadata(metadata: &fs::Metadata) -> io::Result<Self> {
        Ok(Self {
            length: metadata.len(),
            modified: metadata.modified()?,
            created: metadata.created().ok(),
        })
    }
}

#[derive(Clone, Copy)]
enum CsvState {
    Ignored,
    Pending,
    Ready,
}

struct CsvObservation {
    stamp: CsvStamp,
    state: CsvState,
}

pub struct BraveExportWatch {
    directory: PathBuf,
    observed: HashMap<PathBuf, CsvObservation>,
}

impl BraveExportWatch {
    pub fn new(directory: PathBuf) -> io::Result<Self> {
        let observed = csv_metadata(&directory)?
            .into_iter()
            .map(|(path, stamp)| {
                (
                    path,
                    CsvObservation {
                        stamp,
                        state: CsvState::Ignored,
                    },
                )
            })
            .collect();
        Ok(Self {
            directory,
            observed,
        })
    }

    /// Finds only new or changed exports, after matching metadata on two polls.
    pub fn poll(&mut self) -> io::Result<Option<PathBuf>> {
        let mut observed = HashMap::new();
        let mut ready = Vec::new();
        let mut pending = false;
        for (path, stamp) in csv_metadata(&self.directory)? {
            let state = match self.observed.get(&path) {
                Some(previous) if previous.stamp == stamp => match previous.state {
                    CsvState::Pending => {
                        let mut file = match File::open(&path) {
                            Ok(file) => file,
                            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                            Err(error) => return Err(error),
                        };
                        let valid_header = has_brave_export_header(&mut file)?;
                        if CsvStamp::from_metadata(&file.metadata()?)? != stamp {
                            CsvState::Pending
                        } else if valid_header {
                            CsvState::Ready
                        } else {
                            CsvState::Ignored
                        }
                    }
                    state => state,
                },
                _ => CsvState::Pending,
            };
            if matches!(state, CsvState::Ready) {
                ready.push(path.clone());
            }
            pending |= matches!(state, CsvState::Pending);
            observed.insert(path, CsvObservation { stamp, state });
        }
        self.observed = observed;
        if ready.len() > 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Multiple password CSV exports appeared in the watched folder. Choose the intended CSV manually or leave only that new export in the folder.",
            ));
        }
        if pending {
            return Ok(None);
        }
        if let Some(path) = ready.pop() {
            self.observed.get_mut(&path).unwrap().state = CsvState::Ignored;
            Ok(Some(path))
        } else {
            Ok(None)
        }
    }
}

fn csv_metadata(directory: &Path) -> io::Result<HashMap<PathBuf, CsvStamp>> {
    let mut snapshot = HashMap::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
            || !entry.file_type()?.is_file()
        {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        snapshot.insert(path, CsvStamp::from_metadata(&metadata)?);
    }
    Ok(snapshot)
}

fn has_brave_export_header(reader: impl Read) -> io::Result<bool> {
    let mut header = Vec::new();
    let mut complete = false;
    // Read one byte at a time so buffering cannot consume any credential rows.
    for byte in reader.take(1024).bytes() {
        let byte = byte?;
        if matches!(byte, b'\r' | b'\n') {
            complete = true;
            break;
        }
        header.push(byte);
    }
    if !complete {
        return Ok(false);
    }
    let mut reader = csv::ReaderBuilder::new().from_reader(header.as_slice());
    let headers = match reader.headers() {
        Ok(headers) => headers,
        Err(_) => return Ok(false),
    };
    Ok((headers.len() == 4 || headers.len() == 5)
        && ["name", "url", "username", "password"]
            .iter()
            .enumerate()
            .all(|(index, name)| headers.get(index) == Some(*name))
        && (headers.len() == 4 || headers.get(4) == Some("note")))
}

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

    const EXPORT: &[u8] = b"name,url,username,password,note\r\nExample,https://example.test,synthetic-user,synthetic-password,\r\n";

    #[test]
    fn brave_export_watch_waits_for_two_stable_polls_and_returns_once() {
        let directory = tempfile::tempdir().unwrap();
        let mut watch = BraveExportWatch::new(directory.path().to_path_buf()).unwrap();
        let path = directory.path().join("Brave Passwords.csv");
        fs::write(&path, EXPORT).unwrap();
        assert!(watch.poll().unwrap().is_none());
        assert_eq!(watch.poll().unwrap(), Some(path));
        assert!(watch.poll().unwrap().is_none());
    }

    #[test]
    fn brave_export_watch_ignores_unchanged_preexisting_exports() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("old.csv"), EXPORT).unwrap();
        let mut watch = BraveExportWatch::new(directory.path().to_path_buf()).unwrap();
        assert!(watch.poll().unwrap().is_none());
        assert!(watch.poll().unwrap().is_none());
    }

    #[test]
    fn brave_export_watch_supports_overwrite_and_restarts_stability_after_changes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Brave Passwords.csv");
        fs::write(&path, EXPORT).unwrap();
        let mut watch = BraveExportWatch::new(directory.path().to_path_buf()).unwrap();
        fs::write(&path, b"name,url,").unwrap();
        assert!(watch.poll().unwrap().is_none());
        fs::write(&path, EXPORT).unwrap();
        assert!(watch.poll().unwrap().is_none());
        assert_eq!(watch.poll().unwrap(), Some(path));
    }

    #[test]
    fn brave_export_watch_ignores_unrelated_files_and_subdirectories() {
        let directory = tempfile::tempdir().unwrap();
        let mut watch = BraveExportWatch::new(directory.path().to_path_buf()).unwrap();
        fs::write(
            directory.path().join("budget.csv"),
            b"name,total\nExample,5\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("incomplete.csv"),
            b"name,url,username,password",
        )
        .unwrap();
        fs::write(directory.path().join("export.txt"), EXPORT).unwrap();
        fs::create_dir(directory.path().join("nested")).unwrap();
        fs::write(directory.path().join("nested/export.csv"), EXPORT).unwrap();
        assert!(watch.poll().unwrap().is_none());
        assert!(watch.poll().unwrap().is_none());
        assert!(watch.poll().unwrap().is_none());
    }

    #[test]
    fn brave_export_watch_rejects_ambiguous_exports_until_one_remains() {
        let directory = tempfile::tempdir().unwrap();
        let mut watch = BraveExportWatch::new(directory.path().to_path_buf()).unwrap();
        let first = directory.path().join("first.csv");
        let second = directory.path().join("second.csv");
        fs::write(&first, EXPORT).unwrap();
        assert!(watch.poll().unwrap().is_none());
        fs::write(&second, EXPORT).unwrap();
        assert!(watch.poll().unwrap().is_none());
        assert_eq!(watch.poll().unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(watch.poll().unwrap_err().kind(), io::ErrorKind::InvalidData);
        fs::remove_file(second).unwrap();
        assert_eq!(watch.poll().unwrap(), Some(first));
    }

    #[test]
    fn brave_export_header_reads_only_the_bounded_header() {
        let mut reader = io::Cursor::new(EXPORT);
        assert!(has_brave_export_header(&mut reader).unwrap());
        assert_eq!(reader.position(), 32);
        assert!(has_brave_export_header(b"name,url,username,password\n".as_slice()).unwrap());
        assert!(!has_brave_export_header(vec![b'a'; 2048].as_slice()).unwrap());
        assert!(
            !has_brave_export_header(b"name,url,username,password,unexpected\n".as_slice())
                .unwrap()
        );
    }

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
