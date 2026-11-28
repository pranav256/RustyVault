use crate::crypto::{self, CryptoError, DerivedKey, EncryptedEnvelope, SALT_LEN};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use zeroize::{Zeroize, Zeroizing};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Attachment {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EntryRevision {
    pub saved_at: DateTime<Utc>,
    pub service: String,
    pub username: String,
    pub email: String,
    pub password: String,
    pub url: String,
    pub notes: String,
    // Keep legacy OTP data when loading and saving existing vaults.
    #[serde(default)]
    pub totp_secret: String,
    #[serde(default)]
    pub fields: Vec<(String, String)>,
}

impl Drop for EntryRevision {
    fn drop(&mut self) {
        self.password.zeroize();
        self.notes.zeroize();
        self.totp_secret.zeroize();
        self.username.zeroize();
        self.email.zeroize();
        for (_, value) in &mut self.fields {
            value.zeroize();
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VaultEntry {
    pub id: String,
    pub service: String,
    pub username: String,
    pub email: String,
    pub password: String,
    pub url: String,
    pub notes: String,
    pub category: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub kind: String,
    // Keep legacy OTP data when loading and saving existing vaults.
    #[serde(default)]
    pub totp_secret: String,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub history: Vec<EntryRevision>,
    #[serde(default)]
    pub fields: Vec<(String, String)>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

impl VaultEntry {
    pub fn new(
        service: impl Into<String>,
        username: impl Into<String>,
        email: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        let id = format!("{:x}", rand::random::<u128>());
        Self {
            id,
            service: service.into(),
            username: username.into(),
            email: email.into(),
            password: password.into(),
            url: String::new(),
            notes: String::new(),
            category: "General".to_string(),
            created_at: now,
            updated_at: now,
            favorite: false,
            tags: Vec::new(),
            kind: "Login".into(),
            totp_secret: String::new(),
            expires_at: None,
            deleted_at: None,
            history: Vec::new(),
            fields: Vec::new(),
            attachments: Vec::new(),
        }
    }
}

impl VaultEntry {
    pub fn username_or_email(&self) -> &str {
        if self.username.is_empty() {
            &self.email
        } else {
            &self.username
        }
    }

    pub fn is_weak(&self) -> bool {
        !self.password.is_empty()
            && u8::from(zxcvbn::zxcvbn(&self.password, &[&self.service, &self.username]).score())
                < 3
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|date| date < Utc::now())
    }

    pub fn validate(&self) -> Result<(), VaultError> {
        if self.service.trim().is_empty() {
            return Err(VaultError::InvalidData("Enter a title".into()));
        }
        if self.kind != "Secure note" && self.password.is_empty() {
            return Err(VaultError::InvalidData(
                "Enter a password or choose Secure note".into(),
            ));
        }
        if self
            .attachments
            .iter()
            .any(|a| a.bytes.len() > 5 * 1024 * 1024)
        {
            return Err(VaultError::InvalidData(
                "Attachments must be 5 MB or smaller".into(),
            ));
        }
        if !self.url.is_empty()
            && !self.url.starts_with("https://")
            && !self.url.starts_with("http://")
        {
            return Err(VaultError::InvalidData(
                "Website must start with https:// or http://".into(),
            ));
        }
        Ok(())
    }
}

impl Drop for VaultEntry {
    fn drop(&mut self) {
        self.password.zeroize();
        self.totp_secret.zeroize();
        self.notes.zeroize();
        self.username.zeroize();
        self.email.zeroize();
        for (_, value) in &mut self.fields {
            value.zeroize();
        }
        for attachment in &mut self.attachments {
            attachment.bytes.zeroize();
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VaultSettings {
    pub lock_seconds: u64,
    pub clipboard_seconds: u64,
    pub lock_on_focus_loss: bool,
    #[serde(default)]
    pub browser_enabled: bool,
}

impl Default for VaultSettings {
    fn default() -> Self {
        Self {
            lock_seconds: 300,
            clipboard_seconds: 30,
            lock_on_focus_loss: false,
            browser_enabled: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VaultData {
    pub version: u32,
    pub entries: Vec<VaultEntry>,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub settings: VaultSettings,
}

#[derive(Clone)]
pub struct Vault {
    pub salt: [u8; SALT_LEN],
    pub entries: Vec<VaultEntry>,
    pub groups: Vec<String>,
    pub settings: VaultSettings,
}

#[derive(Debug)]
pub enum VaultError {
    Crypto(CryptoError),
    Io(io::Error),
    Serialization(serde_json::Error),
    InvalidSalt,
    InvalidData(String),
    Csv(csv::Error),
}

impl From<CryptoError> for VaultError {
    fn from(e: CryptoError) -> Self {
        VaultError::Crypto(e)
    }
}

impl From<io::Error> for VaultError {
    fn from(e: io::Error) -> Self {
        VaultError::Io(e)
    }
}

impl From<serde_json::Error> for VaultError {
    fn from(e: serde_json::Error) -> Self {
        VaultError::Serialization(e)
    }
}

impl From<csv::Error> for VaultError {
    fn from(e: csv::Error) -> Self {
        Self::Csv(e)
    }
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::Crypto(e) => write!(f, "Crypto error: {}", e),
            VaultError::Io(e) => write!(f, "I/O error: {}", e),
            VaultError::Serialization(e) => write!(f, "Serialization error: {}", e),
            VaultError::InvalidSalt => write!(f, "Corrupted salt in vault file"),
            VaultError::InvalidData(message) => write!(f, "{}", message),
            VaultError::Csv(e) => write!(f, "CSV error: {}", e),
        }
    }
}

impl std::error::Error for VaultError {}

impl Vault {
    /// Create a new empty vault with a fresh random salt
    pub fn new() -> Self {
        Self {
            salt: crypto::generate_salt(),
            entries: Vec::new(),
            groups: vec!["Personal".into(), "Work".into()],
            settings: VaultSettings::default(),
        }
    }

    /// Add a new entry to the vault
    pub fn add_entry(&mut self, entry: VaultEntry) {
        self.entries.push(entry);
    }

    /// Remove an entry by ID
    pub fn remove_entry(&mut self, id: &str) -> bool {
        let initial_len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < initial_len
    }

    /// Update an existing entry
    // shortcut: history excludes attachments and organization, add full snapshots if versioning them becomes necessary.
    pub fn update_entry(&mut self, updated: VaultEntry) -> bool {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.id == updated.id) {
            let revision = EntryRevision {
                saved_at: existing.updated_at,
                service: existing.service.clone(),
                username: existing.username.clone(),
                email: existing.email.clone(),
                password: existing.password.clone(),
                url: existing.url.clone(),
                notes: existing.notes.clone(),
                totp_secret: existing.totp_secret.clone(),
                fields: existing.fields.clone(),
            };
            let mut updated = updated;
            updated.history = existing.history.clone();
            updated.history.push(revision);
            if updated.history.len() > 20 {
                updated.history.remove(0);
            }
            *existing = updated;
            existing.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Search entries case-insensitively by service, username, email, or notes
    pub fn search(&self, query: &str) -> Vec<&VaultEntry> {
        if query.trim().is_empty() {
            return self.entries.iter().collect();
        }

        let q = query.trim().to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.service.to_lowercase().contains(&q)
                    || e.username.to_lowercase().contains(&q)
                    || e.email.to_lowercase().contains(&q)
                    || e.notes.to_lowercase().contains(&q)
                    || e.url.to_lowercase().contains(&q)
                    || e.category.to_lowercase().contains(&q)
                    || e.tags.iter().any(|tag| tag.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Save the vault to an encrypted file using atomic write (tempfile -> rename)
    pub fn save_to_file(&self, path: &Path, key: &DerivedKey) -> Result<(), VaultError> {
        let vault_data = VaultData {
            version: 2,
            entries: self.entries.clone(),
            groups: self.groups.clone(),
            settings: self.settings.clone(),
        };

        let json_bytes = Zeroizing::new(serde_json::to_vec(&vault_data)?);
        let mut envelope = crypto::encrypt(key, &json_bytes)?;
        envelope.salt = self.salt.to_vec();

        let encoded = serde_json::to_vec_pretty(&envelope)?;
        if encoded.len() > 64 * 1024 * 1024 {
            return Err(VaultError::InvalidData(
                "Vault would exceed the 64 MB limit. Remove attachments before saving.".into(),
            ));
        }

        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(&encoded)?;
        temp.as_file().sync_all()?;
        if path.exists() {
            fs::copy(path, path.with_extension("enc.bak"))?;
        }
        temp.persist(path).map_err(|e| VaultError::Io(e.error))?;

        Ok(())
    }

    /// Load and decrypt the vault from disk
    pub fn load_from_file(path: &Path, password: &str) -> Result<(Self, DerivedKey), VaultError> {
        if fs::metadata(path)?.len() > 64 * 1024 * 1024 {
            return Err(VaultError::InvalidData(
                "Vault is larger than the 64 MB limit".into(),
            ));
        }
        let raw_bytes = fs::read(path)?;
        let envelope: EncryptedEnvelope = serde_json::from_slice(&raw_bytes)?;
        if envelope.version != 1 {
            return Err(VaultError::InvalidData(
                "Unsupported encryption format".into(),
            ));
        }

        if envelope.salt.len() != SALT_LEN {
            return Err(VaultError::InvalidSalt);
        }

        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&envelope.salt);

        let key = crypto::derive_key(password, &salt)?;
        let decrypted_bytes = Zeroizing::new(crypto::decrypt(&key, &envelope)?);
        let vault_data: VaultData = serde_json::from_slice(&decrypted_bytes)?;
        if !(1..=2).contains(&vault_data.version) {
            return Err(VaultError::InvalidData("Unsupported vault version".into()));
        }

        Ok((
            Self {
                salt,
                entries: vault_data.entries,
                groups: vault_data.groups,
                settings: vault_data.settings,
            },
            key,
        ))
    }

    /// Legacy importer: reads old Python emails.txt format (chr(ord(c) - 5))
    pub fn import_legacy_txt(&mut self, path: &Path) -> Result<usize, io::Error> {
        use std::io::Read;
        let mut raw = Zeroizing::new(Vec::new());
        fs::File::open(path)?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut raw)?;
        if raw.len() > 16 * 1024 * 1024 {
            return Err(io::Error::other("Import exceeds the 16 MB limit"));
        }
        let content = std::str::from_utf8(&raw)
            .map_err(|_| io::Error::other("Legacy import must be UTF-8 text"))?;
        let mut count = 0;

        for line in content.lines() {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 3 {
                let decrypt_str = |s: &str| -> String {
                    s.chars()
                        .map(|c| {
                            if c == ' ' {
                                ' '
                            } else {
                                ((c as u32).saturating_sub(5)) as u8 as char
                            }
                        })
                        .collect()
                };

                let user = decrypt_str(parts[0].trim());
                let email = decrypt_str(parts[1].trim());
                let pass = decrypt_str(parts[2].trim());

                if !user.is_empty() || !email.is_empty() || !pass.is_empty() {
                    let service = if !user.is_empty() {
                        format!("Account ({})", user)
                    } else {
                        "Imported Account".to_string()
                    };
                    self.add_entry(VaultEntry::new(service, user, email, pass));
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}

impl Default for Vault {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read_csv(bytes: &[u8]) -> Result<Vec<VaultEntry>, VaultError> {
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::Headers)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    let column = |names: &[&str]| {
        headers
            .iter()
            .position(|h| names.iter().any(|n| h.eq_ignore_ascii_case(n)))
    };
    let title = column(&["title", "name", "service"]);
    if title.is_none() {
        return Err(VaultError::InvalidData(
            "CSV needs a title, name or service column".into(),
        ));
    }
    let user = column(&["username", "login_username", "user name"]);
    let email = column(&["email"]);
    let password = column(&["password", "login_password"]);
    let url = column(&["url", "login_uri"]);
    let notes = column(&["notes"]);
    let group = column(&["group", "folder", "category"]);
    let tags = column(&["tags"]);
    let kind = column(&["type", "kind"]);
    let known = [title, user, email, password, url, notes, group, tags, kind];
    let mut entries = Vec::new();
    for row in reader.records() {
        let row = row?;
        let get = |index: Option<usize>| index.and_then(|i| row.get(i)).unwrap_or("");
        if get(title).trim().is_empty() {
            return Err(VaultError::InvalidData(format!(
                "CSV row {} has no title",
                entries.len() + 2
            )));
        }
        let mut entry = VaultEntry::new(get(title), get(user), get(email), get(password));
        entry.url = get(url).into();
        entry.notes = get(notes).into();
        if !get(group).is_empty() {
            entry.category = get(group).into();
        }
        entry.tags = get(tags)
            .split(';')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().into())
            .collect();
        if get(kind).eq_ignore_ascii_case("note")
            || get(kind) == "Secure note"
            || entry.password.is_empty()
        {
            entry.kind = "Secure note".into();
        }
        for (i, header) in headers.iter().enumerate() {
            if !known.contains(&Some(i)) && !row[i].is_empty() {
                entry.fields.push((header.into(), row[i].into()));
            }
        }
        entries.push(entry);
        if entries.len() > 10_000 {
            return Err(VaultError::InvalidData(
                "Import is limited to 10,000 entries".into(),
            ));
        }
    }
    Ok(entries)
}

pub fn write_csv(entries: &[VaultEntry]) -> Result<Vec<u8>, VaultError> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record([
        "title", "username", "email", "password", "url", "notes", "group", "tags", "type",
    ])?;
    for entry in entries.iter().filter(|e| e.deleted_at.is_none()) {
        writer.write_record([
            &entry.service,
            &entry.username,
            &entry.email,
            &entry.password,
            &entry.url,
            &entry.notes,
            &entry.category,
            &entry.tags.join(";"),
            &entry.kind,
        ])?;
    }
    writer
        .into_inner()
        .map_err(|e| VaultError::Io(e.into_error()))
}
