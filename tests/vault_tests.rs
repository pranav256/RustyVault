use rusty_vault::generator::{generate_password, GeneratorConfig};
use rusty_vault::vault::{Vault, VaultEntry};

#[test]
fn test_vault_crud_lifecycle() {
    let mut vault = Vault::new();
    let entry = VaultEntry::new(
        "GitHub",
        "pranav256",
        "pranav@example.com",
        "SecretPass123!",
    );
    vault.add_entry(entry.clone());
    assert_eq!(vault.entries.len(), 1);

    let search_results = vault.search("github");
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].username, "pranav256");

    let deleted = vault.remove_entry(&entry.id);
    assert!(deleted);
    assert_eq!(vault.entries.len(), 0);
}

#[test]
fn test_password_generator_constraints() {
    let config = GeneratorConfig {
        length: 24,
        uppercase: true,
        lowercase: true,
        numbers: true,
        symbols: true,
        exclude_ambiguous: true,
    };
    let password = generate_password(&config);
    assert_eq!(password.len(), 24);
}
#[test]
fn encrypted_roundtrip_history_and_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.enc");
    let mut v = Vault::new();
    let key = rusty_vault::crypto::derive_key("long test master passphrase", &v.salt).unwrap();
    let mut e = VaultEntry::new("GitHub", "alex", "", "original-password");
    e.tags = vec!["Development".into()];
    e.favorite = true;
    e.totp_secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ".into();
    e.fields = vec![("Recovery".into(), "old-recovery-code".into())];
    e.attachments.push(rusty_vault::vault::Attachment {
        name: "codes.txt".into(),
        bytes: b"recovery-secret".to_vec(),
    });
    v.add_entry(e.clone());
    v.settings.browser_enabled = true;
    v.save_to_file(&path, &key).unwrap();
    let original = std::fs::read(&path).unwrap();
    e.password = "updated-password".into();
    e.totp_secret.clear();
    e.fields[0].1 = "new-recovery-code".into();
    v.update_entry(e.clone());
    v.entries[0].deleted_at = Some(chrono::Utc::now());
    v.save_to_file(&path, &key).unwrap();
    assert_eq!(
        std::fs::read(path.with_extension("enc.bak")).unwrap(),
        original
    );
    let (loaded, _) = Vault::load_from_file(&path, "long test master passphrase").unwrap();
    assert_eq!(loaded.entries[0].history[0].password, "original-password");
    assert_eq!(
        loaded.entries[0].history[0].totp_secret,
        "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"
    );
    assert_eq!(
        loaded.entries[0].history[0].fields[0].1,
        "old-recovery-code"
    );
    assert_eq!(loaded.entries[0].attachments[0].bytes, b"recovery-secret");
    assert!(loaded.settings.browser_enabled);
    assert!(loaded.entries[0].deleted_at.is_some());
    assert_eq!(loaded.search("Development").len(), 1);
    assert!(!String::from_utf8(std::fs::read(&path).unwrap())
        .unwrap()
        .contains("updated-password"));
    assert!(Vault::load_from_file(&path, "wrong password").is_err());
}

#[test]
fn reads_original_version_one_vault() {
    use rusty_vault::crypto;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.enc");
    let salt = crypto::generate_salt();
    let key = crypto::derive_key("old-password", &salt).unwrap();
    let e = VaultEntry::new("Original", "user", "", "pass");
    let mut entry = serde_json::to_value(&e).unwrap();
    for field in [
        "favorite",
        "tags",
        "kind",
        "totp_secret",
        "expires_at",
        "deleted_at",
        "history",
        "fields",
        "attachments",
    ] {
        entry.as_object_mut().unwrap().remove(field);
    }
    let data = serde_json::json!({"version":1,"entries":[entry]});
    let mut encrypted = crypto::encrypt(&key, &serde_json::to_vec(&data).unwrap()).unwrap();
    encrypted.salt = salt.to_vec();
    std::fs::write(&path, serde_json::to_vec(&encrypted).unwrap()).unwrap();
    let (v, _) = Vault::load_from_file(&path, "old-password").unwrap();
    assert_eq!(v.entries[0].service, "Original");
    assert!(!v.entries[0].favorite);
    assert_eq!(v.settings.clipboard_seconds, 30);
    assert!(!v.settings.browser_enabled);
}

#[test]
fn failed_backup_does_not_replace_vault() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.enc");
    let mut v = Vault::new();
    let key = rusty_vault::crypto::derive_key("test-password", &v.salt).unwrap();
    v.save_to_file(&path, &key).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::create_dir(path.with_extension("enc.bak")).unwrap();
    v.add_entry(VaultEntry::new("New", "", "", "secret"));
    assert!(v.save_to_file(&path, &key).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn csv_quoted_values_and_unknown_columns_are_preserved() {
    use rusty_vault::vault::{read_csv, write_csv};
    let csv=b"title,username,password,notes,extra\n\"Example, Inc\",alex,\"quote\"\"pass\",\"line one\nline two\",custom-value\n";
    let entries = read_csv(csv).unwrap();
    assert_eq!(entries[0].service, "Example, Inc");
    assert_eq!(entries[0].password, "quote\"pass");
    assert_eq!(entries[0].notes, "line one\nline two");
    assert_eq!(entries[0].fields[0].1, "custom-value");
    let again = read_csv(&write_csv(&entries).unwrap()).unwrap();
    assert_eq!(again[0].password, entries[0].password);
    assert!(read_csv(b"username,password\nalex,secret\n").is_err());
    assert!(read_csv(b"title,username,password\n,alex,secret\n").is_err());
}

#[test]
fn totp_rfc_vector_and_invalid_secret() {
    let otp = rusty_vault::vault::parse_totp(
        "otpauth://totp/Test?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&digits=8&period=30",
    )
    .unwrap();
    assert_eq!(otp.generate(59), "94287082");
    assert!(rusty_vault::vault::parse_totp("invalid!!!").is_err());
}

#[test]
fn passphrase_uses_full_eff_wordlist() {
    let list: Vec<_> = include_str!("../assets/eff-wordlist.txt")
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .collect();
    assert_eq!(list.len(), 7776);
    let phrase = rusty_vault::generator::generate_passphrase(7, "-", false);
    let words: Vec<_> = phrase.split('-').collect();
    assert_eq!(words.len(), 7);
    assert!(words.iter().all(|word| list.contains(word)));
}
