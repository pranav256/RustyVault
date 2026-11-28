use rand::{rngs::StdRng, RngCore, SeedableRng};
use rusty_vault::{
    browser::{read_frame, write_frame},
    crypto::{self, CryptoError, NONCE_LEN, SALT_LEN},
    generator::{generate_password, GeneratorConfig},
    vault::{read_csv, write_csv, EntryRevision, Vault, VaultEntry, VaultError},
};
use std::{fs, io::Cursor, panic::catch_unwind};

#[test]
fn csv_parser_handles_seeded_adversarial_bytes_without_panicking() {
    let mut rng = StdRng::seed_from_u64(0x5255_5354_5956_4155);
    let punctuation = b"abcXYZ0123,\"\r\n\0; \t";
    for case in 0..768 {
        let length = (rng.next_u32() as usize) % 2049;
        let mut input = if case % 2 == 0 {
            b"title,password,extra\r\n".to_vec()
        } else {
            Vec::new()
        };
        let start = input.len();
        input.resize(start + length, 0);
        rng.fill_bytes(&mut input[start..]);
        if case % 3 == 0 {
            for byte in &mut input[start..] {
                *byte = punctuation[*byte as usize % punctuation.len()];
            }
        }
        assert!(
            catch_unwind(|| read_csv(&input)).is_ok(),
            "CSV parser panicked on seeded case {case} ({length} bytes)"
        );
    }
}

#[test]
fn csv_roundtrips_quoted_unicode_credentials_and_omits_trash() {
    let mut entry = VaultEntry::new(
        "Example, \"日本語\"",
        "用户😀",
        "δοκιμή@example.test",
        "sëcret,\"🔐\"\r\nsecond",
    );
    entry.url = "https://example.test/路径?q=one,two".into();
    entry.notes = "first line\r\nsecond, \"quoted\" line\nآخر".into();
    entry.category = "工作,私用".into();
    entry.tags = vec!["日本語".into(), "quoted,tag".into(), "😀".into()];

    let mut trashed = VaultEntry::new("Trashed", "synthetic", "", "discard-me");
    trashed.deleted_at = Some(chrono::Utc::now());
    let exported = write_csv(&[entry.clone(), trashed]).unwrap();
    let imported = read_csv(&exported).unwrap();
    assert_eq!(imported.len(), 1);
    let actual = &imported[0];
    assert_eq!(actual.service, entry.service);
    assert_eq!(actual.username, entry.username);
    assert_eq!(actual.email, entry.email);
    assert_eq!(actual.password, entry.password);
    assert_eq!(actual.url, entry.url);
    assert_eq!(actual.notes, entry.notes);
    assert_eq!(actual.category, entry.category);
    assert_eq!(actual.tags, entry.tags);
    assert_eq!(actual.kind, entry.kind);
}

#[test]
fn csv_rejects_missing_titles_unequal_records_and_excessive_rows() {
    for input in [
        b"username,password\nuser,secret\n".as_slice(),
        b"title,password\n   ,secret\n".as_slice(),
        b"title,password,custom\nExample,secret\n".as_slice(),
        b"title,password\nExample,secret,extra\n".as_slice(),
        b"title,password\nExample,\xff\n".as_slice(),
    ] {
        assert!(read_csv(input).is_err());
    }
    let mut input = String::from("title,password\n");
    for _ in 0..10_000 {
        input.push_str("Example,synthetic-secret\n");
    }
    assert_eq!(read_csv(input.as_bytes()).unwrap().len(), 10_000);
    input.push_str("One too many,synthetic-secret\n");
    assert!(matches!(
        read_csv(input.as_bytes()),
        Err(VaultError::InvalidData(_))
    ));
}

#[test]
fn authenticated_encryption_rejects_every_byte_tamper_and_wrong_key() {
    let salt = [0x51; SALT_LEN];
    let key = crypto::derive_key("synthetic-correct-password", &salt).unwrap();
    let wrong_key = crypto::derive_key("synthetic-wrong-password", &salt).unwrap();
    let plaintext = b"synthetic credential payload";
    let envelope = crypto::encrypt(&key, plaintext).unwrap();
    assert_eq!(crypto::decrypt(&key, &envelope).unwrap(), plaintext);
    assert!(crypto::decrypt(&wrong_key, &envelope).is_err());

    for index in 0..envelope.ciphertext.len() {
        let mut tampered = envelope.clone();
        tampered.ciphertext[index] ^= 0x01;
        assert!(
            crypto::decrypt(&key, &tampered).is_err(),
            "ciphertext byte {index} was not authenticated"
        );
    }
    for index in 0..NONCE_LEN {
        let mut tampered = envelope.clone();
        tampered.nonce[index] ^= 0x80;
        assert!(crypto::decrypt(&key, &tampered).is_err());
    }
    for length in 0..envelope.ciphertext.len() {
        let mut truncated = envelope.clone();
        truncated.ciphertext.truncate(length);
        assert!(crypto::decrypt(&key, &truncated).is_err());
    }
    for length in [0, 1, NONCE_LEN - 1, NONCE_LEN + 1, 64] {
        let mut malformed = envelope.clone();
        malformed.nonce.resize(length, 0);
        assert!(matches!(
            crypto::decrypt(&key, &malformed),
            Err(CryptoError::InvalidPayload)
        ));
    }
    for version in [0, 2, u32::MAX] {
        let mut malformed = envelope.clone();
        malformed.version = version;
        assert!(matches!(
            crypto::decrypt(&key, &malformed),
            Err(CryptoError::InvalidPayload)
        ));
    }
}

#[test]
fn vault_loading_rejects_malformed_envelopes_and_inner_payloads() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("synthetic.enc");
    let password = "synthetic-load-password";
    let salt = [0x62; SALT_LEN];
    let key = crypto::derive_key(password, &salt).unwrap();

    let assert_rejected = |bytes: &[u8], label: &str| {
        fs::write(&path, bytes).unwrap();
        let result = catch_unwind(|| Vault::load_from_file(&path, password));
        assert!(result.is_ok(), "vault loading panicked for {label}");
        assert!(result.unwrap().is_err(), "vault loading accepted {label}");
    };
    for bytes in [
        b"".as_slice(),
        b"not json".as_slice(),
        b"{}".as_slice(),
        b"[]".as_slice(),
        br#"{"version":"1","salt":[],"nonce":[],"ciphertext":[]}"#.as_slice(),
        br#"{"version":1,"salt":[256],"nonce":[],"ciphertext":[]}"#.as_slice(),
    ] {
        assert_rejected(bytes, "invalid envelope JSON");
    }

    let mut envelope = crypto::encrypt(&key, br#"{"version":2,"entries":[]}"#).unwrap();
    envelope.salt = salt.to_vec();
    fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    assert!(Vault::load_from_file(&path, password).is_ok());
    assert!(Vault::load_from_file(&path, "incorrect synthetic password").is_err());

    for version in [0, 2, u32::MAX] {
        let mut invalid = envelope.clone();
        invalid.version = version;
        assert_rejected(&serde_json::to_vec(&invalid).unwrap(), "envelope version");
    }
    for length in [0, 1, SALT_LEN - 1, SALT_LEN + 1, 64] {
        let mut invalid = envelope.clone();
        invalid.salt.resize(length, 0);
        assert_rejected(&serde_json::to_vec(&invalid).unwrap(), "salt length");
    }
    let mut invalid = envelope.clone();
    invalid.nonce.truncate(NONCE_LEN - 1);
    assert_rejected(&serde_json::to_vec(&invalid).unwrap(), "nonce length");
    let mut invalid = envelope.clone();
    invalid.ciphertext[0] ^= 1;
    assert_rejected(&serde_json::to_vec(&invalid).unwrap(), "ciphertext tamper");

    for plaintext in [
        b"not json".as_slice(),
        br#"{"version":0,"entries":[]}"#.as_slice(),
        br#"{"version":3,"entries":[]}"#.as_slice(),
        br#"{"version":2,"entries":"invalid"}"#.as_slice(),
    ] {
        let mut invalid = crypto::encrypt(&key, plaintext).unwrap();
        invalid.salt = salt.to_vec();
        assert_rejected(
            &serde_json::to_vec(&invalid).unwrap(),
            "inner vault payload",
        );
    }
}

#[test]
fn vault_loading_rejects_oversized_files_before_parsing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("oversized-synthetic.enc");
    fs::File::create(&path)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert!(matches!(
        Vault::load_from_file(&path, "unused synthetic password"),
        Err(VaultError::InvalidData(_))
    ));
}

#[test]
fn native_frames_enforce_truncation_and_length_boundaries() {
    const MAX_MESSAGE: usize = 64 * 1024;
    assert!(read_frame(&mut Cursor::new([])).unwrap().is_none());
    for prefix_length in 1..4 {
        let prefix = 1_u32.to_le_bytes();
        assert!(read_frame(&mut Cursor::new(&prefix[..prefix_length])).is_err());
    }
    for length in [0, MAX_MESSAGE as u32 + 1, u32::MAX] {
        assert!(read_frame(&mut Cursor::new(length.to_le_bytes())).is_err());
    }
    for length in [1, 8, 1024, MAX_MESSAGE] {
        let payload = vec![0xa5; length];
        let mut encoded = Vec::new();
        write_frame(&mut encoded, &payload).unwrap();
        assert_eq!(&encoded[..4], &(length as u32).to_le_bytes());
        assert_eq!(encoded.len(), length + 4);
        for truncation in [0, length / 2, length - 1] {
            assert!(read_frame(&mut Cursor::new(&encoded[..4 + truncation])).is_err());
        }
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(
            read_frame(&mut cursor).unwrap().unwrap().as_slice(),
            payload
        );
        assert!(read_frame(&mut cursor).unwrap().is_none());
    }
    for payload in [Vec::new(), vec![0; MAX_MESSAGE + 1]] {
        let mut output = Vec::new();
        assert!(write_frame(&mut output, &payload).is_err());
        assert!(output.is_empty(), "invalid frame partially wrote output");
    }
    let mut consecutive = Vec::new();
    write_frame(&mut consecutive, b"first").unwrap();
    write_frame(&mut consecutive, b"second").unwrap();
    let mut cursor = Cursor::new(consecutive);
    assert_eq!(
        read_frame(&mut cursor).unwrap().unwrap().as_slice(),
        b"first"
    );
    assert_eq!(
        read_frame(&mut cursor).unwrap().unwrap().as_slice(),
        b"second"
    );
    assert!(read_frame(&mut cursor).unwrap().is_none());
}

#[test]
fn browser_origins_reject_nested_blob_urls_and_foreign_sites() {
    use rusty_vault::browser::{origin, read_login, Request};
    let nested = format!("{}https://example.test", "blob:".repeat(10_000));
    assert!(origin(&nested).is_err());
    let mut vault = Vault::new();
    let mut entry = VaultEntry::new("Example", "synthetic", "", "synthetic-secret");
    entry.url = "https://example.test/login".into();
    let id = entry.id.clone();
    vault.add_entry(entry);
    for site in [
        nested.as_str(),
        "https://example.test.outside.test",
        "https://example.test@outside.test",
        "https://example.test:444",
        "http://example.test",
        "data:text/html,synthetic",
        "https://example.test/login",
    ] {
        let response = read_login(
            &vault,
            &Request::Fill {
                origin: site.into(),
                id: id.clone(),
            },
        );
        assert!(!response.ok && response.password.is_none());
    }
    let response = read_login(
        &vault,
        &Request::Fill {
            origin: "https://example.test".into(),
            id,
        },
    );
    assert_eq!(response.password.as_deref(), Some("synthetic-secret"));
}

#[test]
fn password_generator_respects_every_class_combination() {
    const CLASSES: [&str; 4] = [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "abcdefghijklmnopqrstuvwxyz",
        "0123456789",
        "!@#$%^&*()_+-=[]{}|;:,.<>?",
    ];
    const AMBIGUOUS: &str = "Il1O0o`~";
    for mask in 0_u8..16 {
        for exclude_ambiguous in [false, true] {
            for length in [0, 8, 24, 128] {
                let config = GeneratorConfig {
                    length,
                    uppercase: mask & 1 != 0,
                    lowercase: mask & 2 != 0,
                    numbers: mask & 4 != 0,
                    symbols: mask & 8 != 0,
                    exclude_ambiguous,
                };
                let password = generate_password(&config);
                assert_eq!(password.chars().count(), length.max(8));
                let enabled: Vec<&str> = CLASSES
                    .iter()
                    .enumerate()
                    .filter_map(|(index, class)| (mask & (1 << index) != 0).then_some(*class))
                    .collect();
                if enabled.is_empty() {
                    assert!(password.chars().all(|c| CLASSES[1].contains(c)));
                } else {
                    assert!(password
                        .chars()
                        .all(|c| enabled.iter().any(|class| class.contains(c))));
                    for class in &enabled {
                        assert!(password.chars().any(|c| class.contains(c)));
                    }
                }
                if exclude_ambiguous {
                    assert!(password.chars().all(|c| !AMBIGUOUS.contains(c)));
                }
            }
        }
    }
}

#[test]
fn updates_bound_history_and_ignore_caller_injected_revisions() {
    let mut vault = Vault::new();
    vault.add_entry(VaultEntry::new("Example", "synthetic", "", "password-0"));
    for version in 1..=30 {
        let mut updated = vault.entries[0].clone();
        updated.password = format!("password-{version}");
        updated.notes = format!("notes-{version}");
        updated.fields = vec![("custom".into(), format!("field-{version}"))];
        updated.history = vec![EntryRevision {
            saved_at: chrono::Utc::now(),
            service: "Injected".into(),
            username: String::new(),
            email: String::new(),
            password: "caller-controlled-history".into(),
            url: String::new(),
            notes: String::new(),
            totp_secret: String::new(),
            fields: Vec::new(),
        }];
        assert!(vault.update_entry(updated));
        assert_eq!(vault.entries[0].history.len(), version.min(20));
    }
    let entry = &vault.entries[0];
    assert_eq!(entry.password, "password-30");
    for (revision, expected_version) in entry.history.iter().zip(10..30) {
        assert_eq!(revision.service, "Example");
        assert_eq!(revision.password, format!("password-{expected_version}"));
        assert_eq!(revision.notes, format!("notes-{expected_version}"));
        assert_eq!(
            revision.fields,
            vec![("custom".into(), format!("field-{expected_version}"))]
        );
    }
    assert!(!vault.update_entry(VaultEntry::new("Missing", "", "", "")));
    assert_eq!(vault.entries[0].history.len(), 20);
}
