use crate::vault::{Vault, VaultEntry};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
use zeroize::{Zeroize, Zeroizing};

pub const EXTENSION_ORIGIN: &str = "chrome-extension://mbogmhfmgcaindpokcpgimbaefjoppgb/";
pub const FIREFOX_EXTENSION_ID: &str = "rustyvault@pranav256.com";
const MAX_MESSAGE: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Status,
    List {
        origin: String,
    },
    Fill {
        origin: String,
        id: String,
    },
    Save {
        origin: String,
        username: String,
        password: String,
    },
}

impl Drop for Request {
    fn drop(&mut self) {
        if let Self::Save {
            username, password, ..
        } = self
        {
            username.zeroize();
            password.zeroize();
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Login {
    pub id: String,
    pub title: String,
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub logins: Vec<Login>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

impl Response {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(message.into()),
            logins: Vec::new(),
            username: None,
            password: None,
        }
    }
    pub fn success() -> Self {
        Self {
            ok: true,
            error: None,
            logins: Vec::new(),
            username: None,
            password: None,
        }
    }
}

impl Drop for Response {
    fn drop(&mut self) {
        if let Some(value) = &mut self.password {
            value.zeroize();
        }
        if let Some(value) = &mut self.username {
            value.zeroize();
        }
        for login in &mut self.logins {
            login.username.zeroize();
        }
    }
}

pub fn origin(value: &str) -> Result<String, &'static str> {
    let url = url::Url::parse(value).map_err(|_| "Choose a valid HTTPS website")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Browser integration requires an HTTPS website without embedded credentials");
    }
    Ok(url.origin().ascii_serialization())
}

fn matches(entry: &VaultEntry, site: &str) -> bool {
    entry.deleted_at.is_none()
        && entry.kind != "Secure note"
        && !entry.password.is_empty()
        && origin(&entry.url).is_ok_and(|candidate| candidate == site)
}

pub fn read_login(vault: &Vault, request: &Request) -> Response {
    let site = match request {
        Request::Status => return Response::success(),
        Request::List { origin } | Request::Fill { origin, .. } => match self::origin(origin) {
            Ok(site) if site == *origin => site,
            _ => return Response::error("A canonical HTTPS origin is required"),
        },
        _ => return Response::error("Unsupported request"),
    };
    let mut response = Response::success();
    match request {
        Request::List { .. } => {
            response.logins = vault
                .entries
                .iter()
                .filter(|e| matches(e, &site))
                .take(50)
                .map(|e| Login {
                    id: e.id.clone(),
                    title: e.service.chars().take(128).collect(),
                    username: e.username_or_email().chars().take(128).collect(),
                })
                .collect();
        }
        Request::Fill { id, .. } => {
            let Some(entry) = vault
                .entries
                .iter()
                .find(|e| e.id == *id && matches(e, &site))
            else {
                return Response::error("That login does not match this website");
            };
            response.username = Some(entry.username_or_email().into());
            response.password = Some(entry.password.clone());
        }
        _ => {}
    }
    response
}

pub fn saved_entry(vault: &Vault, request: &Request) -> Result<VaultEntry, &'static str> {
    let Request::Save {
        origin: site,
        username,
        password,
    } = request
    else {
        return Err("Unsupported request");
    };
    let canonical = origin(site)?;
    if canonical != *site || username.len() > 2048 || password.is_empty() || password.len() > 16384
    {
        return Err("A valid website and a password of at most 16384 bytes are required");
    }
    let mut entry = vault
        .entries
        .iter()
        .find(|e| matches(e, site) && e.username_or_email() == username)
        .cloned()
        .unwrap_or_else(|| {
            VaultEntry::new(
                url::Url::parse(site).unwrap().host_str().unwrap(),
                username,
                "",
                password,
            )
        });
    entry.password.clone_from(password);
    if entry.url.is_empty() {
        entry.url = site.clone();
    }
    entry
        .validate()
        .map_err(|_| "The browser login is invalid")?;
    Ok(entry)
}

pub fn read_frame(reader: &mut impl Read) -> io::Result<Option<Zeroizing<Vec<u8>>>> {
    let mut length = [0; 4];
    match reader.read(&mut length[..1])? {
        0 => return Ok(None),
        _ => reader.read_exact(&mut length[1..])?,
    }
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_MESSAGE {
        return Err(io::Error::other("Invalid message size"));
    }
    let mut bytes = Zeroizing::new(vec![0; length]);
    reader.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

pub fn write_frame(writer: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() || bytes.len() > MAX_MESSAGE {
        return Err(io::Error::other("Invalid message size"));
    }
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(bytes)?;
    writer.flush()
}

pub fn session_path() -> io::Result<PathBuf> {
    directories::ProjectDirs::from("com", "pranav256", "rusty-vault")
        .map(|d| d.config_dir().join("browser-session.json"))
        .ok_or_else(|| io::Error::other("No private configuration directory is available"))
}

#[derive(Serialize, Deserialize)]
struct Session {
    port: u16,
    token: String,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}

#[derive(Serialize, Deserialize)]
struct Authenticated {
    token: String,
    request: Request,
}
impl Drop for Authenticated {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}

pub struct Incoming {
    pub request: Request,
    pub reply: mpsc::SyncSender<Response>,
}

pub struct Server {
    receiver: mpsc::Receiver<Incoming>,
    stop: Arc<AtomicBool>,
    path: PathBuf,
    token: Zeroizing<String>,
}

impl Server {
    pub fn start(wake: impl Fn() + Send + 'static) -> io::Result<Self> {
        Self::start_at(session_path()?, wake)
    }

    fn start_at(path: PathBuf, wake: impl Fn() + Send + 'static) -> io::Result<Self> {
        fs::create_dir_all(path.parent().unwrap())?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))?;
        lock.try_lock()
            .map_err(|_| io::Error::other("Another vault already has a browser connection"))?;
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let token = Zeroizing::new(format!(
            "{:032x}{:032x}",
            rand::random::<u128>(),
            rand::random::<u128>()
        ));
        let session = Session {
            port: listener.local_addr()?.port(),
            token: token.to_string(),
        };
        let bytes = Zeroizing::new(serde_json::to_vec(&session)?);
        let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(&bytes)?;
        file.persist(&path).map_err(|e| e.error)?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        std::thread::spawn(move || {
            let _lock: File = lock;
            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let result = (|| -> io::Result<()> {
                            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                            stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                            let bytes = read_frame(&mut stream)?
                                .ok_or_else(|| io::Error::other("Missing request"))?;
                            let mut message: Authenticated = serde_json::from_slice(&bytes)?;
                            let equal = message.token.len() == session.token.len()
                                && message
                                    .token
                                    .bytes()
                                    .zip(session.token.bytes())
                                    .fold(0, |diff, (a, b)| diff | (a ^ b))
                                    == 0;
                            if !equal {
                                return Err(io::Error::other("Invalid connection token"));
                            }
                            let (reply, answer) = mpsc::sync_channel(1);
                            let request = std::mem::replace(&mut message.request, Request::Status);
                            sender
                                .try_send(Incoming { request, reply })
                                .map_err(|_| io::Error::other("Vault is busy"))?;
                            wake();
                            let response = answer
                                .recv_timeout(Duration::from_secs(10))
                                .map_err(|_| io::Error::other("Vault did not respond"))?;
                            let bytes = Zeroizing::new(serde_json::to_vec(&response)?);
                            write_frame(&mut stream, &bytes)
                        })();
                        if result.is_err() {
                            let _ = write_frame(&mut stream, br#"{"ok":false,"error":"Browser connection unavailable; unlock the vault and try again"}"#);
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(50))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            receiver,
            stop,
            path,
            token,
        })
    }
    pub fn try_recv(&self) -> Option<Incoming> {
        self.receiver.try_recv().ok()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Session>(&bytes).ok())
            .is_some_and(|s| s.token == *self.token)
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub fn native_host(origin: &str, input: &mut impl Read, output: &mut impl Write) -> io::Result<()> {
    if origin != EXTENSION_ORIGIN && origin != FIREFOX_EXTENSION_ID {
        return Err(io::Error::other("Untrusted extension"));
    }
    let Some(bytes) = read_frame(input)? else {
        return Ok(());
    };
    let request: Request = serde_json::from_slice(&bytes)?;
    let result = (|| -> io::Result<Zeroizing<Vec<u8>>> {
        let path = session_path()?;
        let session: Session = serde_json::from_slice(&fs::read(path)?)?;
        let mut stream = TcpStream::connect_timeout(
            &SocketAddrV4::new(Ipv4Addr::LOCALHOST, session.port).into(),
            Duration::from_secs(2),
        )?;
        stream.set_read_timeout(Some(Duration::from_secs(12)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let message = Authenticated {
            token: session.token.clone(),
            request,
        };
        let bytes = Zeroizing::new(serde_json::to_vec(&message)?);
        write_frame(&mut stream, &bytes)?;
        read_frame(&mut stream)?.ok_or_else(|| io::Error::other("Vault disconnected"))
    })();
    match result {
        Ok(bytes) => write_frame(output, &bytes),
        Err(_) => write_frame(output, br#"{"ok":false,"error":"Open RustyVault, unlock your vault and enable the browser connection in Settings"}"#),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_rejects_truncation_and_oversized_messages() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, b"{} ").unwrap();
        assert_eq!(
            read_frame(&mut bytes.as_slice())
                .unwrap()
                .unwrap()
                .as_slice(),
            b"{} "
        );
        assert!(read_frame(&mut &bytes[..5]).is_err());
        assert!(read_frame(&mut &[1u8, 0, 0][..]).is_err());
        assert!(read_frame(&mut &u32::MAX.to_le_bytes()[..]).is_err());
        assert!(native_host("chrome-extension://evil/", &mut &bytes[..], &mut Vec::new()).is_err());
        let mut status = Vec::new();
        write_frame(&mut status, br#"{"action":"status"}"#).unwrap();
        assert!(native_host(
            FIREFOX_EXTENSION_ID,
            &mut status.as_slice(),
            &mut Vec::new()
        )
        .is_ok());
    }
    #[test]
    fn browser_access_requires_an_exact_https_origin() {
        let mut vault = Vault::new();
        let mut entry = VaultEntry::new("Example", "alex", "", "original");
        entry.url = "https://example.com/login".into();
        vault.add_entry(entry.clone());
        for site in [
            "http://example.com",
            "https://sub.example.com",
            "https://example.com.evil.test",
            "https://example.com:444",
            "https://example.com/path",
            "https://user@example.com",
        ] {
            let reply = read_login(
                &vault,
                &Request::Fill {
                    origin: site.into(),
                    id: entry.id.clone(),
                },
            );
            assert!(!reply.ok && reply.password.is_none(), "Accepted {site}");
        }
        let request = Request::Fill {
            origin: "https://example.com".into(),
            id: entry.id.clone(),
        };
        assert_eq!(
            read_login(&vault, &request).password.as_deref(),
            Some("original")
        );
        vault.entries[0].deleted_at = Some(chrono::Utc::now());
        assert!(!read_login(&vault, &request).ok);
    }
    #[test]
    fn browser_save_updates_history_and_keeps_metadata() {
        let mut vault = Vault::new();
        let save = |password: &str| Request::Save {
            origin: "https://example.com".into(),
            username: "alex".into(),
            password: password.into(),
        };
        let mut entry = saved_entry(&vault, &save("original")).unwrap();
        entry.notes = "keep me".into();
        assert_eq!(entry.url, "https://example.com");
        vault.add_entry(entry);
        let updated = saved_entry(&vault, &save("replacement")).unwrap();
        assert_eq!(updated.notes, "keep me");
        assert!(vault.update_entry(updated));
        assert_eq!(vault.entries.len(), 1);
        assert_eq!(vault.entries[0].history[0].password, "original");
        assert!(!read_login(
            &vault,
            &Request::List {
                origin: "https://example.com".into()
            }
        )
        .logins
        .is_empty());
        assert!(saved_entry(&vault, &save("")).is_err());
    }

    #[test]
    fn socket_requires_token_and_disconnects_when_stopped() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        let (wake, ready) = mpsc::channel();
        let server = Server::start_at(path.clone(), move || {
            let _ = wake.send(());
        })
        .unwrap();
        assert!(Server::start_at(path.clone(), || {}).is_err());
        let session: Session = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let connect = || {
            let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, session.port)).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream
        };
        let mut rejected = connect();
        write_frame(
            &mut rejected,
            br#"{"token":"wrong","request":{"action":"status"}}"#,
        )
        .unwrap();
        let bytes = read_frame(&mut rejected).unwrap().unwrap();
        let response: Response = serde_json::from_slice(&bytes).unwrap();
        assert!(!response.ok);
        assert!(server.try_recv().is_none());
        let mut accepted = connect();
        let message = Authenticated {
            token: session.token.clone(),
            request: Request::Status,
        };
        let bytes = Zeroizing::new(serde_json::to_vec(&message).unwrap());
        write_frame(&mut accepted, &bytes).unwrap();
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        let incoming = server.try_recv().unwrap();
        assert!(matches!(incoming.request, Request::Status));
        incoming.reply.send(Response::success()).unwrap();
        let bytes = read_frame(&mut accepted).unwrap().unwrap();
        assert!(serde_json::from_slice::<Response>(&bytes).unwrap().ok);
        drop(server);
        assert!(!path.exists());
    }
}
