use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CONFIG_FILE: &str = "splinterparty.conf";
const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const LARGE_FILE_PART_SIZE: u64 = 100 * 1024 * 1024;
const READ_BUF_SIZE: usize = 64 * 1024;
const SHARE_FILE: &str = ".splinterparty.share";
const UPLOAD_FILE: &str = ".splinterparty.upload";
const REMOTE_LINK_SUFFIX: &str = ".splinterparty.remote";
const PIN_COOKIE: &str = "sp_pin";
const DIRECTORY_CSS: &str = r#"
:root { color-scheme: light dark; font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #f6f7f9; color: #171a1f; }
* { box-sizing: border-box; }
body { margin: 0; min-height: 100vh; background: #f6f7f9; }
main { width: min(1120px, calc(100vw - 32px)); margin: 0 auto; padding: 32px 0; }
header { display: flex; align-items: flex-end; justify-content: space-between; gap: 20px; margin-bottom: 18px; }
.eyebrow { margin: 0 0 6px; font-size: 12px; font-weight: 700; letter-spacing: 0; text-transform: uppercase; color: #53606f; }
h1 { margin: 0; font-size: 30px; line-height: 1.2; overflow-wrap: anywhere; }
.summary { min-width: 88px; padding: 12px 14px; border: 1px solid #d9dee7; border-radius: 8px; background: #ffffff; text-align: right; }
.summary span { display: block; font-size: 24px; font-weight: 750; }
.summary small { color: #657386; }
.remote-card { display: grid; gap: 6px; margin: 0 0 14px; padding: 14px 16px; border: 1px solid #d9dee7; border-radius: 8px; background: #ffffff; }
.remote-card strong { font-size: 14px; }
.remote-card code { display: inline-block; width: fit-content; max-width: 100%; padding: 4px 7px; border-radius: 6px; background: #eef2ff; color: #1e3a8a; overflow-wrap: anywhere; }
.remote-card small { color: #657386; }
nav { display: flex; flex-wrap: wrap; gap: 8px; margin: 0 0 12px; }
a { color: #155eef; text-decoration: none; }
a:hover { text-decoration: underline; }
.up { display: inline-flex; align-items: center; min-height: 34px; padding: 0 12px; border: 1px solid #cbd5e1; border-radius: 8px; background: #ffffff; color: #1f2937; font-weight: 650; }
.browser { overflow: hidden; border: 1px solid #d9dee7; border-radius: 8px; background: #ffffff; }
.row { display: grid; grid-template-columns: minmax(220px, 1fr) 120px 110px 160px; gap: 14px; align-items: center; min-height: 52px; padding: 0 16px; border-top: 1px solid #edf0f4; }
.row:first-child { border-top: 0; }
.row.head { min-height: 38px; background: #f1f4f8; color: #53606f; font-size: 12px; font-weight: 750; text-transform: uppercase; }
.name { display: inline-flex; align-items: center; gap: 10px; min-width: 0; color: #111827; font-weight: 650; overflow-wrap: anywhere; }
.icon { flex: 0 0 auto; display: inline-flex; align-items: center; justify-content: center; width: 42px; height: 24px; border-radius: 6px; background: #e7eefc; color: #174ea6; font-size: 10px; font-weight: 800; }
.type { color: #53606f; }
.type.large { color: #9a3412; font-weight: 750; }
.actions { text-align: right; }
.actions a { font-weight: 650; }
.row.item { cursor: context-menu; }
.context-menu { position: fixed; z-index: 1000; min-width: 190px; display: none; padding: 6px; border: 1px solid #cbd5e1; border-radius: 10px; background: #ffffff; box-shadow: 0 12px 32px rgba(15, 23, 42, .18); }
.context-menu button, .context-menu a { display: block; width: 100%; padding: 9px 10px; border: 0; border-radius: 7px; background: transparent; color: #111827; font: inherit; font-weight: 650; text-align: left; text-decoration: none; cursor: pointer; }
.context-menu button:hover, .context-menu a:hover { background: #eef2ff; text-decoration: none; }
.context-menu .danger { color: #991b1b; }
.row.item[draggable="true"] { user-select: none; }
.row.item.dragging { opacity: .55; }
.row.item.drop-target { outline: 2px solid #155eef; outline-offset: -2px; background: #eef2ff; }
.empty { padding: 28px 16px; color: #657386; }
.upload-panel { margin-top: 18px; border: 1px solid #d9dee7; border-radius: 8px; background: #ffffff; padding: 16px; }
.upload-panel h2 { margin: 0 0 12px; font-size: 18px; }
.upload-panel form { display: grid; grid-template-columns: minmax(180px, 240px) 1fr minmax(150px, 220px) auto; gap: 14px; align-items: end; }
.upload-panel label { display: grid; gap: 6px; color: #1f2937; font-weight: 700; }
.upload-panel input, textarea { width: 100%; min-height: 40px; border: 1px solid #cbd5e1; border-radius: 8px; padding: 8px 10px; font: inherit; }
.upload-panel button { min-height: 42px; border: 0; border-radius: 8px; background: #155eef; color: #ffffff; font: inherit; font-weight: 750; cursor: pointer; }
textarea { min-height: 120px; resize: vertical; }
@media (max-width: 760px) {
  main { width: min(100vw - 20px, 1120px); padding: 18px 0; }
  header { align-items: stretch; flex-direction: column; }
  .summary { text-align: left; }
  .row { grid-template-columns: 1fr; gap: 4px; align-items: start; padding: 12px; }
  .row.head { display: none; }
  .actions { text-align: left; }
  .upload-panel form { grid-template-columns: 1fr; }
}
"#;
const FORM_CSS: &str = r#"
.form-page { min-height: 100vh; display: grid; place-items: center; }
.panel { width: min(460px, 100%); border: 1px solid #d9dee7; border-radius: 8px; background: #ffffff; padding: 22px; }
.panel h1 { margin-bottom: 12px; font-size: 24px; }
form { display: grid; gap: 14px; margin-top: 16px; }
label { display: grid; gap: 6px; color: #1f2937; font-weight: 700; }
label span, .muted { color: #657386; font-size: 13px; font-weight: 500; }
input, select { width: 100%; min-height: 40px; border: 1px solid #cbd5e1; border-radius: 8px; padding: 8px 10px; font: inherit; }
button { min-height: 42px; border: 0; border-radius: 8px; background: #155eef; color: #ffffff; font: inherit; font-weight: 750; cursor: pointer; }
.error { border: 1px solid #fecaca; border-radius: 8px; background: #fff1f2; color: #991b1b; padding: 10px 12px; }
"#;

fn main() -> io::Result<()> {
    let config = match Command::from_env()? {
        Command::Help => return print_help(),
        Command::Config => return print_config(),
        Command::Hash(path) => return print_file_hash(&path),
        Command::Dedup(root) => return print_duplicates(root.as_deref()),
        Command::SplitLarge(path) => return split_large_path(&path),
        Command::Reassemble(path) => return reassemble_from_manifest(&path),
        Command::Setup => return run_setup(),
        Command::Serve(config) => config,
    };

    if config.port_forward {
        match PortForwarder::new(config.port()).and_then(|forwarder| forwarder.add_mapping()) {
            Ok(mapping) => println!(
                "port forwarding configured: {}:{} -> {}:{}",
                mapping.gateway_name,
                mapping.external_port,
                mapping.local_addr,
                mapping.internal_port
            ),
            Err(error) => eprintln!("port forwarding skipped: {error}"),
        }
    }

    let listener = TcpListener::bind(&config.bind_addr)?;

    println!(
        "serving {} on http://{}",
        config.root.display(),
        config.bind_addr
    );
    if !directory_writable(&config.root) {
        eprintln!(
            "warning: served directory is not writable; uploads and folder creation will fail until OS permissions are fixed"
        );
    }

    let config = Arc::new(config);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let config = Arc::clone(&config);
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, &config) {
                        eprintln!("request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("connection failed: {error}"),
        }
    }

    Ok(())
}

struct Config {
    bind_addr: String,
    root: PathBuf,
    port_forward: bool,
    auth: Option<AuthConfig>,
}

impl Config {
    fn new(
        root: PathBuf,
        bind_addr: String,
        port_forward: bool,
        auth: Option<AuthConfig>,
    ) -> io::Result<Self> {
        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "server root must be a directory",
            ));
        }

        Ok(Self {
            bind_addr,
            root,
            port_forward,
            auth,
        })
    }

    fn from_file() -> io::Result<Option<Self>> {
        let path = Path::new(CONFIG_FILE);
        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(path)?;
        let mut root = None;
        let mut bind_addr = None;
        let mut port_forward = None;
        let mut auth_username = None;
        let mut auth_password = None;

        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            match key.trim() {
                "root" => root = Some(PathBuf::from(value.trim())),
                "bind_addr" => bind_addr = Some(value.trim().to_string()),
                "port_forward" => port_forward = Some(parse_bool(value.trim())),
                "auth_username" => auth_username = Some(value.trim().to_string()),
                "auth_password" => auth_password = Some(value.trim().to_string()),
                _ => {}
            }
        }

        let Some(root) = root else {
            return Ok(None);
        };

        Self::new(
            root,
            bind_addr.unwrap_or_else(|| DEFAULT_BIND_ADDR.to_string()),
            port_forward.unwrap_or(false),
            AuthConfig::from_parts(auth_username, auth_password),
        )
        .map(Some)
    }

    fn save(&self) -> io::Result<()> {
        let mut contents = format!(
            "root={}\nbind_addr={}\nport_forward={}\n",
            self.root.display(),
            self.bind_addr,
            self.port_forward
        );
        if let Some(auth) = &self.auth {
            contents.push_str(&format!(
                "auth_username={}\nauth_password={}\n",
                auth.username, auth.password
            ));
        }
        fs::write(CONFIG_FILE, contents)
    }

    fn port(&self) -> u16 {
        self.bind_addr
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse().ok())
            .unwrap_or(8080)
    }
}

#[derive(Clone)]
struct AuthConfig {
    username: String,
    password: String,
}

impl AuthConfig {
    fn from_parts(username: Option<String>, password: Option<String>) -> Option<Self> {
        match (username, password) {
            (Some(username), Some(password)) if !username.is_empty() && !password.is_empty() => {
                Some(Self { username, password })
            }
            _ => None,
        }
    }
}

enum Command {
    Help,
    Config,
    Hash(PathBuf),
    Dedup(Option<PathBuf>),
    SplitLarge(PathBuf),
    Reassemble(PathBuf),
    Setup,
    Serve(Config),
}

impl Command {
    fn from_env() -> io::Result<Self> {
        let args = env::args().skip(1).collect::<Vec<_>>();
        match args.first().map(String::as_str) {
            Some("-h" | "--help" | "help") => return Ok(Self::Help),
            Some("config") => return Ok(Self::Config),
            Some("hash") => {
                let Some(path) = args.get(1) else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "usage: cargo run -- hash <file>",
                    ));
                };
                return Ok(Self::Hash(PathBuf::from(path)));
            }
            Some("dedup") => return Ok(Self::Dedup(args.get(1).map(PathBuf::from))),
            Some("split-large") => {
                let Some(path) = args.get(1) else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "usage: cargo run -- split-large <file-or-directory>",
                    ));
                };
                return Ok(Self::SplitLarge(PathBuf::from(path)));
            }
            Some("reassemble") => {
                let Some(path) = args.get(1) else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "usage: cargo run -- reassemble <manifest>",
                    ));
                };
                return Ok(Self::Reassemble(PathBuf::from(path)));
            }
            Some("setup") => return Ok(Self::Setup),
            _ => {}
        }

        let config = if args.is_empty() {
            Config::from_file()?.unwrap_or(Config::new(
                env::current_dir()?,
                DEFAULT_BIND_ADDR.to_string(),
                false,
                None,
            )?)
        } else {
            let root = PathBuf::from(&args[0]);
            let bind_addr = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| DEFAULT_BIND_ADDR.to_string());
            let port_forward = args.iter().any(|arg| arg == "--port-forward");
            Config::new(root, bind_addr, port_forward, None)?
        };

        Ok(Self::Serve(config))
    }
}

fn print_help() -> io::Result<()> {
    println!(
        "Splinterparty fileserver\n\n\
         Usage:\n\
           cargo run -- setup              Run interactive setup\n\
           cargo run                       Serve using splinterparty.conf, or current directory if no config exists\n\
           cargo run -- <root> [bind]      Serve a directory directly\n\
           cargo run -- config             Show current saved config\n\
           cargo run -- hash <file>        Print a file SHA-256 hash\n\
           cargo run -- dedup [root]       Find duplicate files by SHA-256\n\
           cargo run -- split-large <path> Split files over 100 MiB into hashed parts\n\
           cargo run -- reassemble <file>  Reassemble a split-large manifest\n\
           cargo run -- --help             Show this help\n\n\
         Examples:\n\
           cargo run -- setup\n\
           cargo run\n\
           cargo run -- /mnt/storage 0.0.0.0:8080 --port-forward\n\
           cargo run -- hash /mnt/storage/photo.jpg\n\
           cargo run -- dedup /mnt/storage\n\
           cargo run -- split-large /mnt/storage/video.mp4\n\
           cargo run -- reassemble /mnt/storage/video.mp4.parts/manifest.txt"
    );
    Ok(())
}

fn print_config() -> io::Result<()> {
    match Config::from_file()? {
        Some(config) => {
            println!("config file: {CONFIG_FILE}");
            println!("root: {}", config.root.display());
            println!("bind address: {}", config.bind_addr);
            println!("port forwarding: {}", enabled_label(config.port_forward));
            match &config.auth {
                Some(auth) => {
                    println!("auth: enabled");
                    println!("auth username: {}", auth.username);
                    println!("auth password: hidden");
                }
                None => println!("auth: disabled"),
            }
        }
        None => {
            println!("no {CONFIG_FILE} found");
            println!("run `cargo run -- setup` to create one");
        }
    }

    Ok(())
}

fn enabled_label(enabled: bool) -> &'static str {
    if enabled { "enabled" } else { "disabled" }
}

fn print_file_hash(path: &Path) -> io::Result<()> {
    let hash = hash_file(path)?;
    println!("{}  {}", hash, path.display());
    Ok(())
}

fn print_duplicates(root: Option<&Path>) -> io::Result<()> {
    let root = match root {
        Some(root) => fs::canonicalize(root)?,
        None => Config::from_file()?
            .map(|config| config.root)
            .unwrap_or(env::current_dir()?),
    };

    let report = find_duplicates(&root)?;
    if report.groups.is_empty() {
        println!("no duplicate files found under {}", root.display());
        return Ok(());
    }

    println!(
        "found {} duplicate groups under {}",
        report.groups.len(),
        root.display()
    );
    println!("duplicate bytes: {}", human_bytes(report.duplicate_bytes));

    for (index, group) in report.groups.iter().enumerate() {
        println!(
            "\n{}. {} files, {}, sha256 {}",
            index + 1,
            group.paths.len(),
            human_bytes(group.size),
            group.hash
        );
        for path in &group.paths {
            println!("   {}", path.display());
        }
    }

    Ok(())
}

fn split_large_path(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if metadata.is_file() {
        split_large_file(path)?;
        return Ok(());
    }

    if metadata.is_dir() {
        let mut files_by_size = BTreeMap::<u64, Vec<PathBuf>>::new();
        collect_files_by_size(path, &mut files_by_size)?;
        let mut split_count = 0_u64;

        for (size, paths) in files_by_size {
            if size <= LARGE_FILE_PART_SIZE {
                continue;
            }

            for path in paths {
                split_large_file(&path)?;
                split_count += 1;
            }
        }

        println!("split {split_count} large files under {}", path.display());
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "split-large path must be a file or directory",
    ))
}

fn split_large_file(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if metadata.len() <= LARGE_FILE_PART_SIZE {
        println!(
            "not a large file: {} is {}",
            path.display(),
            human_bytes(metadata.len())
        );
        return Ok(());
    }

    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid file name"))?;
    let parts_dir = path.with_file_name(format!("{file_name}.parts"));
    fs::create_dir_all(&parts_dir)?;

    let mut source = File::open(path)?;
    let mut manifest = SplitManifest {
        original_name: file_name.to_string(),
        original_size: metadata.len(),
        part_size: LARGE_FILE_PART_SIZE,
        parts: Vec::new(),
    };

    let mut index = 0_u64;
    loop {
        let part_path = parts_dir.join(format!("{index:08}.part"));
        let mut part_file = File::create(&part_path)?;
        let mut hasher = Sha256::new();
        let mut written = 0_u64;
        let mut buffer = [0_u8; READ_BUF_SIZE];

        while written < LARGE_FILE_PART_SIZE {
            let read_len = buffer.len().min((LARGE_FILE_PART_SIZE - written) as usize);
            let bytes_read = source.read(&mut buffer[..read_len])?;
            if bytes_read == 0 {
                break;
            }

            part_file.write_all(&buffer[..bytes_read])?;
            hasher.update(&buffer[..bytes_read]);
            written += bytes_read as u64;
        }

        if written == 0 {
            let _ = fs::remove_file(part_path);
            break;
        }

        manifest.parts.push(SplitPart {
            index,
            file_name: format!("{index:08}.part"),
            size: written,
            sha256: hex_bytes(&hasher.finish()),
        });
        index += 1;
    }

    let manifest_path = parts_dir.join("manifest.txt");
    fs::write(&manifest_path, manifest.to_text())?;

    println!(
        "split {} into {} parts at {}",
        path.display(),
        manifest.parts.len(),
        parts_dir.display()
    );
    println!("manifest: {}", manifest_path.display());
    Ok(())
}

#[derive(Debug)]
struct SplitManifest {
    original_name: String,
    original_size: u64,
    part_size: u64,
    parts: Vec<SplitPart>,
}

#[derive(Debug)]
struct SplitPart {
    index: u64,
    file_name: String,
    size: u64,
    sha256: String,
}

impl SplitManifest {
    fn to_text(&self) -> String {
        let mut output = String::new();
        let _ = writeln!(output, "version=1");
        let _ = writeln!(output, "original_name={}", self.original_name);
        let _ = writeln!(output, "original_size={}", self.original_size);
        let _ = writeln!(output, "part_size={}", self.part_size);
        for part in &self.parts {
            let _ = writeln!(
                output,
                "part={},{},{},{}",
                part.index, part.file_name, part.size, part.sha256
            );
        }
        output
    }

    fn from_text(text: &str) -> io::Result<Self> {
        let mut original_name = None;
        let mut original_size = None;
        let mut part_size = None;
        let mut parts = Vec::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "version" => {
                    if value != "1" {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "unsupported split manifest version",
                        ));
                    }
                }
                "original_name" => original_name = Some(value.to_string()),
                "original_size" => original_size = value.parse().ok(),
                "part_size" => part_size = value.parse().ok(),
                "part" => {
                    let fields = value.split(',').collect::<Vec<_>>();
                    if fields.len() != 4 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "invalid part manifest line",
                        ));
                    }
                    parts.push(SplitPart {
                        index: fields[0].parse().map_err(|_| {
                            io::Error::new(io::ErrorKind::InvalidData, "invalid part index")
                        })?,
                        file_name: fields[1].to_string(),
                        size: fields[2].parse().map_err(|_| {
                            io::Error::new(io::ErrorKind::InvalidData, "invalid part size")
                        })?,
                        sha256: fields[3].to_string(),
                    });
                }
                _ => {}
            }
        }

        Ok(Self {
            original_name: original_name.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "manifest missing original_name")
            })?,
            original_size: original_size.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "manifest missing original_size")
            })?,
            part_size: part_size.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "manifest missing part_size")
            })?,
            parts,
        })
    }
}

fn reassemble_from_manifest(manifest_path: &Path) -> io::Result<()> {
    let manifest_text = fs::read_to_string(manifest_path)?;
    let manifest = SplitManifest::from_text(&manifest_text)?;
    let parts_dir = manifest_path.parent().unwrap_or(Path::new("."));
    let output_path = parts_dir.with_file_name(&manifest.original_name);
    let temp_output_path =
        parts_dir.with_file_name(format!("{}.reassembling", manifest.original_name));

    let mut output = File::create(&temp_output_path)?;
    let mut total_written = 0_u64;

    for part in &manifest.parts {
        let part_path = parts_dir.join(&part.file_name);
        let actual_hash = hash_file(&part_path)?;
        if actual_hash != part.sha256 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "part {} hash mismatch: expected {}, got {}",
                    part.file_name, part.sha256, actual_hash
                ),
            ));
        }

        let metadata = fs::metadata(&part_path)?;
        if metadata.len() != part.size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "part {} size mismatch: expected {}, got {}",
                    part.file_name,
                    part.size,
                    metadata.len()
                ),
            ));
        }

        let mut input = File::open(&part_path)?;
        io::copy(&mut input, &mut output)?;
        total_written += part.size;
    }

    if total_written != manifest.original_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "reassembled size mismatch: expected {}, got {}",
                manifest.original_size, total_written
            ),
        ));
    }

    fs::rename(&temp_output_path, &output_path)?;
    println!("reassembled {}", output_path.display());
    Ok(())
}

struct DedupReport {
    groups: Vec<DuplicateGroup>,
    duplicate_bytes: u64,
}

struct DuplicateGroup {
    size: u64,
    hash: String,
    paths: Vec<PathBuf>,
}

fn find_duplicates(root: &Path) -> io::Result<DedupReport> {
    let mut files_by_size = BTreeMap::<u64, Vec<PathBuf>>::new();
    collect_files_by_size(root, &mut files_by_size)?;

    let mut groups = Vec::new();
    for (size, paths) in files_by_size {
        if paths.len() < 2 {
            continue;
        }

        let mut paths_by_hash = BTreeMap::<String, Vec<PathBuf>>::new();
        for path in paths {
            let hash = hash_file(&path)?;
            paths_by_hash.entry(hash).or_default().push(path);
        }

        for (hash, paths) in paths_by_hash {
            if paths.len() > 1 {
                groups.push(DuplicateGroup { size, hash, paths });
            }
        }
    }

    let duplicate_bytes = groups
        .iter()
        .map(|group| group.size * group.paths.len().saturating_sub(1) as u64)
        .sum();

    Ok(DedupReport {
        groups,
        duplicate_bytes,
    })
}

fn collect_files_by_size(
    root: &Path,
    files_by_size: &mut BTreeMap<u64, Vec<PathBuf>>,
) -> io::Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }

    if metadata.is_file() {
        files_by_size
            .entry(metadata.len())
            .or_default()
            .push(root.to_path_buf());
        return Ok(());
    }

    if metadata.is_dir() {
        for entry in fs::read_dir(root)? {
            collect_files_by_size(&entry?.path(), files_by_size)?;
        }
    }

    Ok(())
}

fn hash_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; READ_BUF_SIZE];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex_bytes(&hasher.finish()))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_bytes(&hasher.finish())
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

fn system_time_label(time: SystemTime) -> Option<String> {
    let seconds = time.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(format_unix_seconds(seconds))
}

fn format_unix_seconds(seconds: u64) -> String {
    let days = seconds / 86_400;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };

    (year, month as u32, day as u32)
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    message_len: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffer_len: 0,
            message_len: 0,
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        self.message_len = self.message_len.wrapping_add(input.len() as u64);

        if self.buffer_len > 0 {
            let fill = (64 - self.buffer_len).min(input.len());
            self.buffer[self.buffer_len..self.buffer_len + fill].copy_from_slice(&input[..fill]);
            self.buffer_len += fill;
            input = &input[fill..];

            if self.buffer_len == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffer_len = 0;
            }
        }

        while input.len() >= 64 {
            self.compress(&input[..64]);
            input = &input[64..];
        }

        if !input.is_empty() {
            self.buffer[..input.len()].copy_from_slice(input);
            self.buffer_len = input.len();
        }
    }

    fn finish(mut self) -> [u8; 32] {
        let bit_len = self.message_len.wrapping_mul(8);

        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        if self.buffer_len > 56 {
            self.buffer[self.buffer_len..].fill(0);
            let block = self.buffer;
            self.compress(&block);
            self.buffer_len = 0;
        }

        self.buffer[self.buffer_len..56].fill(0);
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        let mut output = [0_u8; 32];
        for (index, word) in self.state.iter().enumerate() {
            output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        output
    }

    fn compress(&mut self, block: &[u8]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];

        let mut w = [0_u32; 64];
        for index in 0..16 {
            let offset = index * 4;
            w[index] = u32::from_be_bytes([
                block[offset],
                block[offset + 1],
                block[offset + 2],
                block[offset + 3],
            ]);
        }

        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

fn run_setup() -> io::Result<()> {
    println!("Splinterparty setup");

    let root = prompt_served_root()?;
    let default_bind = DEFAULT_BIND_ADDR.to_string();
    let bind_addr = prompt_string("Bind address", &default_bind)?;
    let port_forward = prompt_bool("Configure router port forwarding with UPnP", true)?;
    let auth = prompt_auth_config()?;

    let config = Config::new(root, bind_addr, port_forward, auth)?;
    config.save()?;

    println!("saved {CONFIG_FILE}");
    if let Some(auth) = &config.auth {
        println!("auth username: {}", auth.username);
        println!("auth password: {}", auth.password);
    }

    if config.port_forward {
        match PortForwarder::new(config.port()).and_then(|forwarder| forwarder.add_mapping()) {
            Ok(mapping) => println!(
                "port forwarding configured: {}:{} -> {}:{}",
                mapping.gateway_name,
                mapping.external_port,
                mapping.local_addr,
                mapping.internal_port
            ),
            Err(error) => eprintln!("port forwarding failed: {error}"),
        }
    }

    println!("run `cargo run` to start serving {}", config.root.display());
    Ok(())
}

fn prompt_auth_config() -> io::Result<Option<AuthConfig>> {
    if !prompt_bool("Require username and password", true)? {
        return Ok(None);
    }

    let username = prompt_string("Username", "admin")?;
    let password = prompt_string("Password", "admin")?;

    Ok(Some(AuthConfig { username, password }))
}

fn prompt_served_root() -> io::Result<PathBuf> {
    let default_root = env::current_dir()?;
    let value = prompt_string("Directory to serve", &default_root.display().to_string())?;
    Ok(PathBuf::from(value))
}

fn prompt_string(label: &str, default: &str) -> io::Result<String> {
    if default.is_empty() {
        print!("{label}: ");
    } else {
        print!("{label} [{default}]: ");
    }
    io::stdout().flush()?;

    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    let value = value.trim();

    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value.to_string())
    }
}

fn prompt_bool(label: &str, default: bool) -> io::Result<bool> {
    let hint = if default { "Y/n" } else { "y/N" };
    loop {
        print!("{label} ({hint}): ");
        io::stdout().flush()?;

        let mut value = String::new();
        io::stdin().read_line(&mut value)?;
        let value = value.trim();

        if value.is_empty() {
            return Ok(default);
        }

        match value.to_ascii_lowercase().as_str() {
            "y" | "yes" | "true" | "1" => return Ok(true),
            "n" | "no" | "false" | "0" => return Ok(false),
            _ => println!("please answer yes or no"),
        }
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "yes" | "1" | "on"
    )
}

fn base64_decode(value: &str) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut chunk = [0_u8; 4];
    let mut chunk_len = 0;

    for byte in value.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        let decoded = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 64,
            _ => return None,
        };

        chunk[chunk_len] = decoded;
        chunk_len += 1;

        if chunk_len == 4 {
            if chunk[0] == 64 || chunk[1] == 64 {
                return None;
            }

            output.push((chunk[0] << 2) | (chunk[1] >> 4));
            if chunk[2] != 64 {
                output.push((chunk[1] << 4) | (chunk[2] >> 2));
            }
            if chunk[3] != 64 {
                output.push((chunk[2] << 6) | chunk[3]);
            }

            chunk_len = 0;
        }
    }

    if chunk_len == 0 { Some(output) } else { None }
}

struct PortForwarder {
    internal_port: u16,
    external_port: u16,
    local_addr: String,
}

impl PortForwarder {
    fn new(port: u16) -> io::Result<Self> {
        Ok(Self {
            internal_port: port,
            external_port: port,
            local_addr: local_lan_addr()?,
        })
    }

    fn add_mapping(&self) -> io::Result<PortMapping> {
        let gateway = discover_gateway()?;
        let description = http_get(&gateway.description_url)?;
        let service = find_wan_service(&description, &gateway.description_url)?;

        let body = format!(
            "<?xml version=\"1.0\"?>\
             <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
             <s:Body>\
             <u:AddPortMapping xmlns:u=\"{}\">\
             <NewRemoteHost></NewRemoteHost>\
             <NewExternalPort>{}</NewExternalPort>\
             <NewProtocol>TCP</NewProtocol>\
             <NewInternalPort>{}</NewInternalPort>\
             <NewInternalClient>{}</NewInternalClient>\
             <NewEnabled>1</NewEnabled>\
             <NewPortMappingDescription>splinterparty fileserver</NewPortMappingDescription>\
             <NewLeaseDuration>0</NewLeaseDuration>\
             </u:AddPortMapping>\
             </s:Body>\
             </s:Envelope>",
            service.service_type, self.external_port, self.internal_port, self.local_addr
        );

        let response = http_post(
            &service.control_url,
            &format!("{}#AddPortMapping", service.service_type),
            &body,
        )?;

        if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
            let status = response.lines().next().unwrap_or("unknown response");
            return Err(io::Error::other(format!(
                "gateway rejected AddPortMapping: {status}"
            )));
        }

        Ok(PortMapping {
            gateway_name: gateway.name,
            external_port: self.external_port,
            internal_port: self.internal_port,
            local_addr: self.local_addr.clone(),
        })
    }
}

struct PortMapping {
    gateway_name: String,
    external_port: u16,
    internal_port: u16,
    local_addr: String,
}

struct Gateway {
    name: String,
    description_url: String,
}

struct WanService {
    service_type: String,
    control_url: String,
}

struct HttpUrl {
    host: String,
    port: u16,
    path: String,
}

fn local_lan_addr() -> io::Result<String> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;
    Ok(socket.local_addr()?.ip().to_string())
}

fn tailscale_remote_url(port: u16) -> Option<String> {
    let output = std::process::Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let ip = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?
        .to_string();

    Some(format!("http://{ip}:{port}"))
}

fn discover_gateway() -> io::Result<Gateway> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::from_secs(3)))?;

    for search_target in [
        "urn:schemas-upnp-org:device:InternetGatewayDevice:1",
        "urn:schemas-upnp-org:service:WANIPConnection:1",
        "urn:schemas-upnp-org:service:WANPPPConnection:1",
    ] {
        let message = format!(
            "M-SEARCH * HTTP/1.1\r\n\
             HOST: 239.255.255.250:1900\r\n\
             MAN: \"ssdp:discover\"\r\n\
             MX: 2\r\n\
             ST: {search_target}\r\n\r\n"
        );
        socket.send_to(message.as_bytes(), "239.255.255.250:1900")?;
    }

    let mut buffer = [0_u8; 8192];
    loop {
        match socket.recv_from(&mut buffer) {
            Ok((len, source)) => {
                let response = String::from_utf8_lossy(&buffer[..len]);
                if let Some(location) = header_value(&response, "LOCATION") {
                    return Ok(Gateway {
                        name: source.to_string(),
                        description_url: location,
                    });
                }
            }
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::TimedOut =>
            {
                return Err(io::Error::other("no UPnP gateway responded"));
            }
            Err(error) => return Err(error),
        }
    }
}

fn find_wan_service(description: &str, description_url: &str) -> io::Result<WanService> {
    for service in service_blocks(description) {
        let Some(service_type) = tag_value(service, "serviceType") else {
            continue;
        };
        if !service_type.contains("WANIPConnection") && !service_type.contains("WANPPPConnection") {
            continue;
        }

        let Some(control_url) = tag_value(service, "controlURL") else {
            continue;
        };

        return Ok(WanService {
            service_type,
            control_url: absolutize_url(description_url, &control_url)?,
        });
    }

    Err(io::Error::other(
        "gateway description did not include a WAN connection service",
    ))
}

fn service_blocks(description: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut rest = description;

    while let Some(start) = rest.find("<service>") {
        rest = &rest[start + "<service>".len()..];
        let Some(end) = rest.find("</service>") else {
            break;
        };
        blocks.push(&rest[..end]);
        rest = &rest[end + "</service>".len()..];
    }

    blocks
}

fn tag_value(input: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let start = input.find(&start_tag)? + start_tag.len();
    let end = input[start..].find(&end_tag)? + start;
    Some(input[start..end].trim().to_string())
}

fn header_value(input: &str, name: &str) -> Option<String> {
    input.lines().find_map(|line| {
        let (header_name, value) = line.split_once(':')?;
        if header_name.eq_ignore_ascii_case(name) {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

fn http_get(url: &str) -> io::Result<String> {
    let url = parse_http_url(url)?;
    let mut stream = TcpStream::connect((&*url.host, url.port))?;
    write!(
        stream,
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        url.path, url.host
    )?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn http_post(url: &str, soap_action: &str, body: &str) -> io::Result<String> {
    let url = parse_http_url(url)?;
    let mut stream = TcpStream::connect((&*url.host, url.port))?;
    write!(
        stream,
        "POST {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: text/xml; charset=\"utf-8\"\r\n\
         SOAPAction: \"{}\"\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{}",
        url.path,
        url.host,
        soap_action,
        body.len(),
        body
    )?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn parse_http_url(url: &str) -> io::Result<HttpUrl> {
    let Some(rest) = url.strip_prefix("http://") else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "only http:// UPnP URLs are supported",
        ));
    };

    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host.to_string(), port.parse().unwrap_or(80)),
        None => (authority.to_string(), 80),
    };

    Ok(HttpUrl {
        host,
        port,
        path: format!("/{path}"),
    })
}

fn absolutize_url(base: &str, value: &str) -> io::Result<String> {
    if value.starts_with("http://") {
        return Ok(value.to_string());
    }

    let base = parse_http_url(base)?;
    let path = if value.starts_with('/') {
        value.to_string()
    } else {
        let parent = base.path.rsplit_once('/').map_or("/", |(parent, _)| parent);
        format!("{parent}/{value}")
    };

    Ok(format!("http://{}:{}{}", base.host, base.port, path))
}

fn handle_connection(mut stream: TcpStream, config: &Config) -> io::Result<()> {
    let peer = stream
        .peer_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let request = match read_request(&mut stream)? {
        Some(request) => request,
        None => return Ok(()),
    };

    if request.target.starts_with("/__pin") {
        log_request(&peer, &request, "302");
        return handle_pin_route(&mut stream, &request);
    }

    if request.target.starts_with("/__share") {
        log_request(&peer, &request, "200");
        return handle_share_route(&mut stream, config, &request);
    }

    if request.target.starts_with("/__folder") {
        log_request(&peer, &request, "200");
        return handle_folder_route(&mut stream, config, &request);
    }

    if request.target.starts_with("/__remote") {
        log_request(&peer, &request, "200");
        return handle_remote_route(&mut stream, config, &request);
    }

    if request.target.starts_with("/__symlink") {
        log_request(&peer, &request, "200");
        return handle_symlink_route(&mut stream, config, &request);
    }

    if request.target.starts_with("/__upload") {
        log_request(&peer, &request, "200");
        return handle_upload_route(&mut stream, config, &request);
    }

    if request.target.starts_with("/__chunk") {
        log_request(&peer, &request, "200");
        return handle_chunk_route(&mut stream, config, &request);
    }

    if request.target.starts_with("/__delete") {
        log_request(&peer, &request, "200");
        return handle_delete_route(&mut stream, config, &request);
    }

    if request.target.starts_with("/__move") {
        log_request(&peer, &request, "200");
        return handle_move_route(&mut stream, config, &request);
    }

    if request.target.starts_with("/__copy") {
        log_request(&peer, &request, "200");
        return handle_copy_route(&mut stream, config, &request);
    }

    if request.method != "GET" && request.method != "HEAD" {
        log_request(&peer, &request, "405");
        return write_text_response(
            &mut stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "GET, HEAD")],
            request.method == "HEAD",
        );
    }

    if !is_authorized(&request, config.auth.as_ref()) {
        log_request(&peer, &request, "401");
        return write_text_response(
            &mut stream,
            "401 Unauthorized",
            "Authentication required\n",
            &[("WWW-Authenticate", "Basic realm=\"Splinterparty\"")],
            request.method == "HEAD",
        );
    }

    let requested_path = match path_for_request(&config.root, &request.target) {
        Some(path) => path,
        None => {
            log_request(&peer, &request, "400");
            return write_text_response(
                &mut stream,
                "400 Bad Request",
                "Bad request path\n",
                &[],
                request.method == "HEAD",
            );
        }
    };
    let requested_is_symlink = fs::symlink_metadata(&requested_path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);

    let path = match contained_path(&config.root, &requested_path) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            log_request(&peer, &request, "404");
            return write_text_response(
                &mut stream,
                "404 Not Found",
                "Not found\n",
                &[],
                request.method == "HEAD",
            );
        }
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            log_request(&peer, &request, "403");
            return write_text_response(
                &mut stream,
                "403 Forbidden",
                "Path escapes served directory\n",
                &[],
                request.method == "HEAD",
            );
        }
        Err(error) => return Err(error),
    };

    if path.file_name().is_some_and(|name| name == SHARE_FILE) {
        log_request(&peer, &request, "404");
        return write_text_response(
            &mut stream,
            "404 Not Found",
            "Not found\n",
            &[],
            request.method == "HEAD",
        );
    }

    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            log_request(&peer, &request, "404");
            return write_text_response(
                &mut stream,
                "404 Not Found",
                "Not found\n",
                &[],
                request.method == "HEAD",
            );
        }
        Err(error) => return Err(error),
    };

    let pin = request_pin(&request);
    if !requested_is_symlink {
        if let Some((_share_dir, share)) = find_applicable_share(&config.root, &path, &metadata)? {
            if !share.allows_read(pin.as_deref()) {
                log_request(&peer, &request, "401");
                return write_html_response(
                    &mut stream,
                    "401 Unauthorized",
                    &pin_prompt_html(&request.target, true),
                    &[("WWW-Authenticate", "PIN realm=\"Splinterparty\"")],
                    request.method == "HEAD",
                );
            }
        }
    }

    if metadata.is_dir() {
        log_request(&peer, &request, "200");
        return serve_directory(
            &mut stream,
            &config.root,
            &path,
            config.port(),
            pin.as_deref(),
            request.method == "HEAD",
        );
    }

    if metadata.is_file() {
        return serve_file(
            &mut stream,
            &peer,
            &request,
            &path,
            &metadata,
            metadata.len(),
            request.method == "HEAD",
        );
    }

    log_request(&peer, &request, "403");
    write_text_response(
        &mut stream,
        "403 Forbidden",
        "Unsupported filesystem entry\n",
        &[],
        request.method == "HEAD",
    )
}

fn log_request(peer: &str, request: &Request, status: &str) {
    println!(
        "{} {} -> {} ({peer})",
        request.method, request.target, status
    );
}

struct Request {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn query_value(&self, name: &str) -> Option<String> {
        let (_, query) = self.target.split_once('?')?;
        form_value(query, name)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ByteRange {
    start: u64,
    end: u64,
}

fn parse_range_header(value: &str, len: u64) -> Option<ByteRange> {
    if len == 0 {
        return None;
    }

    let range = value.trim().strip_prefix("bytes=")?;
    if range.contains(',') {
        return None;
    }

    let (start, end) = range.split_once('-')?;
    match (start.trim(), end.trim()) {
        ("", "") => None,
        ("", suffix_len) => {
            let suffix_len = suffix_len.parse::<u64>().ok()?;
            if suffix_len == 0 {
                return None;
            }

            let start = len.saturating_sub(suffix_len);
            Some(ByteRange {
                start,
                end: len - 1,
            })
        }
        (start, "") => {
            let start = start.parse::<u64>().ok()?;
            if start >= len {
                return None;
            }
            Some(ByteRange {
                start,
                end: len - 1,
            })
        }
        (start, end) => {
            let start = start.parse::<u64>().ok()?;
            let end = end.parse::<u64>().ok()?;
            if start > end || start >= len {
                return None;
            }

            Some(ByteRange {
                start,
                end: end.min(len - 1),
            })
        }
    }
}

fn read_request(stream: &mut TcpStream) -> io::Result<Option<Request>> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();

    if reader.read_line(&mut request_line)? == 0 {
        return Ok(None);
    }

    let mut parts = request_line.split_whitespace();
    let method = match parts.next() {
        Some(method) => method.to_string(),
        None => return Ok(None),
    };
    let target = match parts.next() {
        Some(target) => target.to_string(),
        None => return Ok(None),
    };

    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }

    let content_length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Content-Length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0_u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    Ok(Some(Request {
        method,
        target,
        headers,
        body,
    }))
}

fn is_authorized(request: &Request, auth: Option<&AuthConfig>) -> bool {
    let Some(auth) = auth else {
        return true;
    };

    let Some(header) = request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Authorization"))
        .map(|(_, value)| value)
    else {
        return false;
    };

    let Some(encoded) = header.strip_prefix("Basic ") else {
        return false;
    };

    let Some(decoded) = base64_decode(encoded).and_then(|bytes| String::from_utf8(bytes).ok())
    else {
        return false;
    };

    decoded == format!("{}:{}", auth.username, auth.password)
}

#[derive(Debug)]
struct ShareConfig {
    recovery_hash: String,
    read_hash: Option<String>,
    write_hash: String,
}

impl ShareConfig {
    fn new(recovery: &str, read: Option<&str>, write: &str) -> Self {
        Self {
            recovery_hash: hash_pin(recovery),
            read_hash: read.filter(|pin| !pin.is_empty()).map(hash_pin),
            write_hash: hash_pin(write),
        }
    }

    fn from_text(text: &str) -> io::Result<Self> {
        let mut recovery_hash = None;
        let mut read_hash = None;
        let mut write_hash = None;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "recovery_hash" => recovery_hash = Some(value.to_string()),
                "read_hash" => {
                    if !value.is_empty() {
                        read_hash = Some(value.to_string());
                    }
                }
                "write_hash" => write_hash = Some(value.to_string()),
                _ => {}
            }
        }

        Ok(Self {
            recovery_hash: recovery_hash.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "share missing recovery hash")
            })?,
            read_hash,
            write_hash: write_hash.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "share missing write hash")
            })?,
        })
    }

    fn to_text(&self) -> String {
        format!(
            "version=1\nrecovery_hash={}\nread_hash={}\nwrite_hash={}\n",
            self.recovery_hash,
            self.read_hash.as_deref().unwrap_or(""),
            self.write_hash
        )
    }

    fn allows_read(&self, pin: Option<&str>) -> bool {
        let Some(pin) = pin else {
            return false;
        };
        let hash = hash_pin(pin);
        self.read_hash.as_deref() == Some(hash.as_str()) || self.write_hash == hash
    }

    fn allows_write(&self, pin: Option<&str>) -> bool {
        let Some(pin) = pin else {
            return false;
        };
        self.write_hash == hash_pin(pin)
    }

    fn allows_recovery(&self, recovery: &str) -> bool {
        self.recovery_hash == hash_pin(recovery)
    }
}

fn hash_pin(pin: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pin.as_bytes());
    hex_bytes(&hasher.finish())
}

fn request_pin(request: &Request) -> Option<String> {
    request
        .query_value("pin")
        .filter(|pin| !pin.is_empty())
        .or_else(|| {
            cookie_value(request.header("Cookie")?, PIN_COOKIE)
                .and_then(|value| form_decode(value).ok())
        })
}

fn cookie_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.split(';').find_map(|part| {
        let (cookie_name, value) = part.trim().split_once('=')?;
        if cookie_name == name {
            Some(value)
        } else {
            None
        }
    })
}

fn find_applicable_share(
    root: &Path,
    path: &Path,
    metadata: &fs::Metadata,
) -> io::Result<Option<(PathBuf, ShareConfig)>> {
    let mut current = if metadata.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(root).to_path_buf()
    };

    loop {
        let share_path = current.join(SHARE_FILE);
        if share_path.exists() {
            let share = ShareConfig::from_text(&fs::read_to_string(share_path)?)?;
            return Ok(Some((current, share)));
        }

        if current == root {
            return Ok(None);
        }
        if !current.pop() {
            return Ok(None);
        }
    }
}

fn handle_pin_route(stream: &mut TcpStream, request: &Request) -> io::Result<()> {
    if request.method == "GET" {
        let path = request
            .query_value("path")
            .unwrap_or_else(|| "/".to_string());
        return write_html_response(stream, "200 OK", &pin_prompt_html(&path, false), &[], false);
    }

    if request.method != "POST" {
        return write_text_response(
            stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "GET, POST")],
            false,
        );
    }

    let form = String::from_utf8_lossy(&request.body);
    let path = form_value(&form, "path").unwrap_or_else(|| "/".to_string());
    let pin = form_value(&form, "pin").unwrap_or_default();
    write_redirect_with_cookie(stream, &path, PIN_COOKIE, &pin)
}

fn handle_share_route(
    stream: &mut TcpStream,
    config: &Config,
    request: &Request,
) -> io::Result<()> {
    if request.method == "GET" {
        let path = request
            .query_value("path")
            .unwrap_or_else(|| "/".to_string());
        let folder = match folder_from_url_path(&config.root, &path) {
            Ok(folder) => folder,
            Err(_) => {
                return write_html_response(
                    stream,
                    "400 Bad Request",
                    &folder_only_html(&path),
                    &[],
                    false,
                );
            }
        };
        let exists = folder.join(SHARE_FILE).exists();
        return write_html_response(
            stream,
            "200 OK",
            &share_form_html(&path, exists, None),
            &[],
            false,
        );
    }

    if request.method != "POST" {
        return write_text_response(
            stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "GET, POST")],
            false,
        );
    }

    let form = String::from_utf8_lossy(&request.body);
    let path = form_value(&form, "path").unwrap_or_else(|| "/".to_string());
    let recovery = form_value(&form, "recovery").unwrap_or_default();
    let read_pin = form_value(&form, "read_pin").unwrap_or_default();
    let write_pin = form_value(&form, "write_pin").unwrap_or_default();
    let folder = match folder_from_url_path(&config.root, &path) {
        Ok(folder) => folder,
        Err(_) => {
            return write_html_response(
                stream,
                "400 Bad Request",
                &folder_only_html(&path),
                &[],
                false,
            );
        }
    };
    let share_path = folder.join(SHARE_FILE);

    if recovery.is_empty() || write_pin.is_empty() {
        return write_html_response(
            stream,
            "400 Bad Request",
            &share_form_html(
                &path,
                share_path.exists(),
                Some("Recovery passcode and read+write PIN are required."),
            ),
            &[],
            false,
        );
    }

    if share_path.exists() {
        let existing = ShareConfig::from_text(&fs::read_to_string(&share_path)?)?;
        if !existing.allows_recovery(&recovery) {
            return write_html_response(
                stream,
                "403 Forbidden",
                &share_form_html(&path, true, Some("Recovery passcode is incorrect.")),
                &[],
                false,
            );
        }
    }

    let share = ShareConfig::new(
        &recovery,
        if read_pin.is_empty() {
            None
        } else {
            Some(read_pin.as_str())
        },
        &write_pin,
    );
    if let Err(error) = fs::write(&share_path, share.to_text()) {
        return write_html_response(
            stream,
            "500 Internal Server Error",
            &folder_operation_error_html(
                &path,
                &format!(
                    "Could not save folder PINs. {}",
                    write_permission_message(&path, &error)
                ),
            ),
            &[],
            false,
        );
    }
    write_redirect_with_cookie(stream, &path, PIN_COOKIE, &write_pin)
}

fn handle_folder_route(
    stream: &mut TcpStream,
    config: &Config,
    request: &Request,
) -> io::Result<()> {
    if request.method == "GET" {
        let path = request
            .query_value("path")
            .unwrap_or_else(|| "/".to_string());
        return write_html_response(stream, "200 OK", &folder_form_html(&path, None), &[], false);
    }

    if request.method != "POST" {
        return write_text_response(
            stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "GET, POST")],
            false,
        );
    }

    let form = String::from_utf8_lossy(&request.body);
    let parent_path = form_value(&form, "path").unwrap_or_else(|| "/".to_string());
    let folder_name = form_value(&form, "name").unwrap_or_default();
    let recovery = form_value(&form, "recovery").unwrap_or_default();
    let read_pin = form_value(&form, "read_pin").unwrap_or_default();
    let write_pin = form_value(&form, "write_pin").unwrap_or_default();
    let parent_pin = form_value(&form, "parent_pin")
        .filter(|pin| !pin.is_empty())
        .or_else(|| request_pin(request));

    if folder_name.is_empty() || recovery.is_empty() || write_pin.is_empty() {
        return write_html_response(
            stream,
            "400 Bad Request",
            &folder_form_html(
                &parent_path,
                Some("Folder name, recovery passcode, and read+write PIN are required."),
            ),
            &[],
            false,
        );
    }

    if !is_safe_folder_name(&folder_name) {
        return write_html_response(
            stream,
            "400 Bad Request",
            &folder_form_html(
                &parent_path,
                Some("Folder name cannot contain path separators."),
            ),
            &[],
            false,
        );
    }

    let parent = match folder_from_url_path(&config.root, &parent_path) {
        Ok(parent) => parent,
        Err(error) => {
            return write_html_response(
                stream,
                "400 Bad Request",
                &folder_operation_error_html(
                    &parent_path,
                    &format!("Could not open parent folder: {error}."),
                ),
                &[],
                false,
            );
        }
    };
    let parent_metadata = match fs::metadata(&parent) {
        Ok(metadata) => metadata,
        Err(error) => {
            return write_html_response(
                stream,
                "500 Internal Server Error",
                &folder_operation_error_html(
                    &parent_path,
                    &format!("Could not inspect parent folder: {error}."),
                ),
                &[],
                false,
            );
        }
    };
    let applicable_share = match find_applicable_share(&config.root, &parent, &parent_metadata) {
        Ok(share) => share,
        Err(error) => {
            return write_html_response(
                stream,
                "500 Internal Server Error",
                &folder_operation_error_html(
                    &parent_path,
                    &format!("Could not read folder sharing settings: {error}."),
                ),
                &[],
                false,
            );
        }
    };
    if let Some((_share_dir, share)) = applicable_share {
        if !share.allows_write(parent_pin.as_deref()) {
            return write_html_response(
                stream,
                "403 Forbidden",
                &folder_form_html(
                    &parent_path,
                    Some("The parent folder requires its read+write PIN."),
                ),
                &[],
                false,
            );
        }
    }

    let new_folder = parent.join(&folder_name);
    if let Err(error) = fs::create_dir(&new_folder) {
        return write_html_response(
            stream,
            "500 Internal Server Error",
            &folder_form_html(
                &parent_path,
                Some(&format!(
                    "Could not create folder. {}",
                    write_permission_message(&parent_path, &error)
                )),
            ),
            &[],
            false,
        );
    }
    let share = ShareConfig::new(
        &recovery,
        if read_pin.is_empty() {
            None
        } else {
            Some(read_pin.as_str())
        },
        &write_pin,
    );
    if let Err(error) = fs::write(new_folder.join(SHARE_FILE), share.to_text()) {
        let _ = fs::remove_dir(&new_folder);
        return write_html_response(
            stream,
            "500 Internal Server Error",
            &folder_form_html(
                &parent_path,
                Some(&format!(
                    "Could not save folder PINs. {}",
                    write_permission_message(&parent_path, &error)
                )),
            ),
            &[],
            false,
        );
    }

    let new_path = format!(
        "{}/{}",
        parent_path.trim_end_matches('/'),
        url_encode_path_segment(OsStr::new(&folder_name))
    );
    write_redirect_with_cookie(stream, &new_path, PIN_COOKIE, &write_pin)
}

fn handle_symlink_route(
    stream: &mut TcpStream,
    config: &Config,
    request: &Request,
) -> io::Result<()> {
    let session_pin = request_pin(request);

    if request.method == "GET" {
        let path = request
            .query_value("path")
            .unwrap_or_else(|| "/".to_string());
        let target_path = request.query_value("target_path").unwrap_or_default();
        let name = request.query_value("name").unwrap_or_default();
        return write_symlink_form_response(
            stream,
            "200 OK",
            config,
            session_pin.as_deref(),
            &path,
            None,
            &target_path,
            &name,
        );
    }

    if request.method != "POST" {
        return write_text_response(
            stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "GET, POST")],
            false,
        );
    }

    let form = String::from_utf8_lossy(&request.body);
    let parent_path = form_value(&form, "path").unwrap_or_else(|| "/".to_string());
    let link_name = form_value(&form, "name")
        .unwrap_or_default()
        .trim()
        .to_string();
    let target_path = form_value(&form, "target_path")
        .unwrap_or_default()
        .trim()
        .to_string();
    let parent_pin = form_value(&form, "parent_pin")
        .filter(|pin| !pin.is_empty())
        .or_else(|| session_pin.clone());
    let target_pin = form_value(&form, "target_pin")
        .filter(|pin| !pin.is_empty())
        .or_else(|| session_pin.clone());

    if link_name.is_empty() || target_path.is_empty() {
        return write_symlink_form_response(
            stream,
            "400 Bad Request",
            config,
            session_pin.as_deref(),
            &parent_path,
            Some("Link name and target path are required."),
            &target_path,
            &link_name,
        );
    }

    if !is_safe_symlink_name(&link_name) {
        return write_symlink_form_response(
            stream,
            "400 Bad Request",
            config,
            session_pin.as_deref(),
            &parent_path,
            Some("Link name cannot contain path separators or internal Splinterparty names."),
            &target_path,
            &link_name,
        );
    }

    if !target_path.starts_with('/') {
        return write_symlink_form_response(
            stream,
            "400 Bad Request",
            config,
            session_pin.as_deref(),
            &parent_path,
            Some("Target path must start with /."),
            &target_path,
            &link_name,
        );
    }

    let parent = match folder_from_url_path(&config.root, &parent_path) {
        Ok(parent) => parent,
        Err(error) => {
            return write_symlink_form_response(
                stream,
                "400 Bad Request",
                config,
                session_pin.as_deref(),
                &parent_path,
                Some(&format!("Could not open parent folder: {error}.")),
                &target_path,
                &link_name,
            );
        }
    };

    let parent_metadata = fs::metadata(&parent)?;
    if let Some((_share_dir, share)) =
        find_applicable_share(&config.root, &parent, &parent_metadata)?
    {
        if !share.allows_write(parent_pin.as_deref()) {
            return write_symlink_form_response(
                stream,
                "403 Forbidden",
                config,
                session_pin.as_deref(),
                &parent_path,
                Some("The destination folder requires its read+write PIN."),
                &target_path,
                &link_name,
            );
        }
    }

    let requested_target = match path_for_request(&config.root, &target_path) {
        Some(path) => path,
        None => {
            return write_symlink_form_response(
                stream,
                "400 Bad Request",
                config,
                session_pin.as_deref(),
                &parent_path,
                Some("Invalid target path."),
                &target_path,
                &link_name,
            );
        }
    };

    let resolved_target = match contained_path(&config.root, &requested_target) {
        Ok(path) => path,
        Err(error) => {
            let message = if error.kind() == io::ErrorKind::PermissionDenied {
                "Target symlink resolves outside the served directory."
            } else {
                "Target path does not exist."
            };
            return write_symlink_form_response(
                stream,
                "400 Bad Request",
                config,
                session_pin.as_deref(),
                &parent_path,
                Some(message),
                &target_path,
                &link_name,
            );
        }
    };

    let target_metadata = fs::metadata(&resolved_target)?;
    if let Some((_share_dir, share)) =
        find_applicable_share(&config.root, &resolved_target, &target_metadata)?
    {
        if !share.allows_read(target_pin.as_deref()) {
            return write_symlink_form_response(
                stream,
                "403 Forbidden",
                config,
                session_pin.as_deref(),
                &parent_path,
                Some("The target file/folder requires its read or read+write PIN."),
                &target_path,
                &link_name,
            );
        }
    }

    let link_path = parent.join(&link_name);
    if link_path.exists() || fs::symlink_metadata(&link_path).is_ok() {
        return write_symlink_form_response(
            stream,
            "409 Conflict",
            config,
            session_pin.as_deref(),
            &parent_path,
            Some("A file, folder, or symlink with that name already exists."),
            &target_path,
            &link_name,
        );
    }

    #[cfg(unix)]
    {
        if let Err(error) = std::os::unix::fs::symlink(&resolved_target, &link_path) {
            return write_symlink_form_response(
                stream,
                "500 Internal Server Error",
                config,
                session_pin.as_deref(),
                &parent_path,
                Some(&format!(
                    "Could not create symlink. {}",
                    write_permission_message(&parent_path, &error)
                )),
                &target_path,
                &link_name,
            );
        }
    }

    #[cfg(not(unix))]
    {
        return write_symlink_form_response(
            stream,
            "500 Internal Server Error",
            config,
            session_pin.as_deref(),
            &parent_path,
            Some("Creating symlinks from the browser is currently supported only on Unix/Linux."),
            &target_path,
            &link_name,
        );
    }

    write_redirect_with_cookie(
        stream,
        &parent_path,
        PIN_COOKIE,
        parent_pin.as_deref().unwrap_or(""),
    )
}

fn handle_remote_route(
    stream: &mut TcpStream,
    config: &Config,
    request: &Request,
) -> io::Result<()> {
    if request.method == "GET" {
        let path = request
            .query_value("path")
            .unwrap_or_else(|| "/".to_string());
        return write_html_response(stream, "200 OK", &remote_form_html(&path, None), &[], false);
    }

    if request.method != "POST" {
        return write_text_response(
            stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "GET, POST")],
            false,
        );
    }

    let form = String::from_utf8_lossy(&request.body);
    let parent_path = form_value(&form, "path").unwrap_or_else(|| "/".to_string());
    let name = form_value(&form, "name")
        .unwrap_or_default()
        .trim()
        .to_string();
    let url = form_value(&form, "url")
        .unwrap_or_default()
        .trim()
        .to_string();
    let remote_path = form_value(&form, "remote_path")
        .unwrap_or_else(|| "/".to_string())
        .trim()
        .to_string();
    let parent_pin = form_value(&form, "parent_pin")
        .filter(|pin| !pin.is_empty())
        .or_else(|| request_pin(request));

    if name.is_empty() || url.is_empty() || remote_path.is_empty() {
        return write_html_response(
            stream,
            "400 Bad Request",
            &remote_form_html(
                &parent_path,
                Some("Name, remote URL, and remote path are required."),
            ),
            &[],
            false,
        );
    }

    if !is_safe_remote_link_name(&name) {
        return write_html_response(
            stream,
            "400 Bad Request",
            &remote_form_html(
                &parent_path,
                Some("Link name cannot contain path separators or internal Splinterparty names."),
            ),
            &[],
            false,
        );
    }

    if !is_safe_remote_url(&url) {
        return write_html_response(
            stream,
            "400 Bad Request",
            &remote_form_html(
                &parent_path,
                Some("Remote URL must start with http:// or https://."),
            ),
            &[],
            false,
        );
    }

    if !remote_path.starts_with('/') || remote_path.contains("..") {
        return write_html_response(
            stream,
            "400 Bad Request",
            &remote_form_html(
                &parent_path,
                Some("Remote path must start with / and cannot contain '..'."),
            ),
            &[],
            false,
        );
    }

    let parent = match folder_from_url_path(&config.root, &parent_path) {
        Ok(parent) => parent,
        Err(error) => {
            return write_html_response(
                stream,
                "400 Bad Request",
                &folder_operation_error_html(
                    &parent_path,
                    &format!("Could not open parent folder: {error}."),
                ),
                &[],
                false,
            );
        }
    };

    let parent_metadata = fs::metadata(&parent)?;
    if let Some((_share_dir, share)) =
        find_applicable_share(&config.root, &parent, &parent_metadata)?
    {
        if !share.allows_write(parent_pin.as_deref()) {
            return write_html_response(
                stream,
                "403 Forbidden",
                &remote_form_html(
                    &parent_path,
                    Some("The parent folder requires its read+write PIN."),
                ),
                &[],
                false,
            );
        }
    }

    let filename = format!("{}{}", name, REMOTE_LINK_SUFFIX);
    let target = parent.join(&filename);
    if target.exists() {
        return write_html_response(
            stream,
            "409 Conflict",
            &remote_form_html(
                &parent_path,
                Some("A remote link with that name already exists."),
            ),
            &[],
            false,
        );
    }

    let link = RemoteLink {
        name: name.clone(),
        url,
        path: remote_path,
    };

    if let Err(error) = fs::write(&target, link.to_text()) {
        return write_html_response(
            stream,
            "500 Internal Server Error",
            &remote_form_html(
                &parent_path,
                Some(&format!(
                    "Could not save remote link. {}",
                    write_permission_message(&parent_path, &error)
                )),
            ),
            &[],
            false,
        );
    }

    write_redirect_with_cookie(
        stream,
        &parent_path,
        PIN_COOKIE,
        parent_pin.as_deref().unwrap_or(""),
    )
}

fn handle_upload_route(
    stream: &mut TcpStream,
    config: &Config,
    request: &Request,
) -> io::Result<()> {
    if request.method != "POST" {
        return write_text_response(
            stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "POST")],
            false,
        );
    }

    let upload = parse_upload_request(request)?;
    let path = upload.path;
    let filename = upload.filename;
    let contents = upload.contents;
    let pin = request_pin(request);

    if !is_safe_folder_name(&filename) {
        return write_html_response(
            stream,
            "400 Bad Request",
            &upload_error_html(
                &path,
                "Filename cannot be empty or contain path separators.",
            ),
            &[],
            false,
        );
    }

    let folder = match folder_from_url_path(&config.root, &path) {
        Ok(folder) => folder,
        Err(_) => {
            return write_html_response(
                stream,
                "400 Bad Request",
                &folder_only_html(&path),
                &[],
                false,
            );
        }
    };
    let metadata = fs::metadata(&folder)?;
    if let Some((_share_dir, share)) = find_applicable_share(&config.root, &folder, &metadata)? {
        if !share.allows_write(pin.as_deref()) {
            return write_html_response(
                stream,
                "403 Forbidden",
                &upload_error_html(
                    &path,
                    "Unlock this directory with its read+write PIN before uploading.",
                ),
                &[],
                false,
            );
        }
    }

    let upload_hash = hash_bytes(&contents);
    let target = match unique_upload_target(&folder, &filename, &upload_hash) {
        Ok(target) => target,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return write_html_response(
                stream,
                "409 Conflict",
                &upload_error_html(&path, "That exact file already exists."),
                &[],
                false,
            );
        }
        Err(error) => return Err(error),
    };

    if let Err(error) = fs::write(&target, &contents) {
        return write_html_response(
            stream,
            "500 Internal Server Error",
            &upload_error_html(
                &path,
                &format!(
                    "Could not save upload. {}",
                    write_permission_message(&path, &error)
                ),
            ),
            &[],
            false,
        );
    }
    write_redirect_with_cookie(stream, &path, PIN_COOKIE, pin.as_deref().unwrap_or(""))
}

fn handle_delete_route(
    stream: &mut TcpStream,
    config: &Config,
    request: &Request,
) -> io::Result<()> {
    if request.method == "GET" {
        let url_path = request
            .query_value("path")
            .unwrap_or_else(|| "/".to_string());
        let require_pin = match delete_target(&config.root, &url_path) {
            Ok((path, metadata)) => {
                if let Some((_share_dir, share)) =
                    find_applicable_share(&config.root, &path, &metadata)?
                {
                    !share.allows_write(request_pin(request).as_deref())
                } else {
                    false
                }
            }
            Err(_) => true,
        };
        return write_html_response(
            stream,
            "200 OK",
            &delete_form_html(&url_path, None, require_pin),
            &[],
            false,
        );
    }

    if request.method != "POST" {
        return write_text_response(
            stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "GET, POST")],
            false,
        );
    }

    let form = String::from_utf8_lossy(&request.body);
    let url_path = form_value(&form, "path").unwrap_or_else(|| "/".to_string());
    let pin = form_value(&form, "pin")
        .filter(|pin| !pin.is_empty())
        .or_else(|| request_pin(request))
        .unwrap_or_default();

    let (path, metadata) = match delete_target(&config.root, &url_path) {
        Ok(target) => target,
        Err(error) => {
            let (status, message) = match error.kind() {
                io::ErrorKind::InvalidInput => ("400 Bad Request", "Bad request path."),
                io::ErrorKind::NotFound => ("404 Not Found", "File not found."),
                io::ErrorKind::PermissionDenied => {
                    ("403 Forbidden", "Path escapes served directory.")
                }
                _ => return Err(error),
            };
            return write_html_response(
                stream,
                status,
                &delete_form_html(&url_path, Some(message), true),
                &[],
                false,
            );
        }
    };

    let is_symlink = metadata.file_type().is_symlink();
    if !metadata.is_file() && !is_symlink {
        return write_html_response(
            stream,
            "400 Bad Request",
            &delete_form_html(
                &url_path,
                Some("Only files and symlinks can be deleted from this page."),
                true,
            ),
            &[],
            false,
        );
    }

    if path
        .file_name()
        .is_some_and(|name| name == SHARE_FILE || name == UPLOAD_FILE)
    {
        return write_html_response(
            stream,
            "403 Forbidden",
            &delete_form_html(
                &url_path,
                Some("Internal Splinterparty files cannot be deleted."),
                true,
            ),
            &[],
            false,
        );
    }

    let require_pin =
        if let Some((_share_dir, share)) = find_applicable_share(&config.root, &path, &metadata)? {
            if !share.allows_write(Some(&pin)) {
                return write_html_response(
                    stream,
                    "403 Forbidden",
                    &delete_form_html(
                        &url_path,
                        Some("Read+write PIN required or incorrect."),
                        true,
                    ),
                    &[],
                    false,
                );
            }
            true
        } else {
            false
        };

    if let Err(error) = fs::remove_file(&path) {
        return write_html_response(
            stream,
            "500 Internal Server Error",
            &delete_form_html(
                &url_path,
                Some(&format!(
                    "Could not delete file. {}",
                    write_permission_message(&url_path, &error)
                )),
                require_pin,
            ),
            &[],
            false,
        );
    }

    let parent_path = path
        .parent()
        .map(|parent| url_path_for(&config.root, parent))
        .unwrap_or_else(|| "/".to_string());
    write_redirect_with_cookie(stream, &parent_path, PIN_COOKIE, &pin)
}

fn delete_target(root: &Path, url_path: &str) -> io::Result<(PathBuf, fs::Metadata)> {
    let requested_path = path_for_request(root, url_path)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad request path"))?;

    let file_name = requested_path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cannot delete root"))?;
    let parent = requested_path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad request path"))?;
    let parent = contained_path(root, parent)?;
    let path = parent.join(file_name);
    let metadata = fs::symlink_metadata(&path)?;
    Ok((path, metadata))
}

fn handle_move_route(stream: &mut TcpStream, config: &Config, request: &Request) -> io::Result<()> {
    if request.method != "POST" {
        return write_text_response(
            stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "POST")],
            false,
        );
    }

    let form = String::from_utf8_lossy(&request.body);
    let source_path = form_value(&form, "source").unwrap_or_default();
    let destination_path = form_value(&form, "destination").unwrap_or_default();
    let pin = request_pin(request).unwrap_or_default();

    let (source, source_metadata) = match move_source(&config.root, &source_path) {
        Ok(source) => source,
        Err(error) => {
            return write_text_response(
                stream,
                move_error_status(&error),
                &format!("Could not move source: {error}\n"),
                &[],
                false,
            );
        }
    };

    let destination = match folder_from_url_path(&config.root, &destination_path) {
        Ok(destination) => destination,
        Err(error) => {
            return write_text_response(
                stream,
                move_error_status(&error),
                &format!("Could not open destination folder: {error}\n"),
                &[],
                false,
            );
        }
    };

    let destination_metadata = fs::metadata(&destination)?;
    if !share_allows_write(&config.root, &source, &source_metadata, Some(&pin))? {
        return write_text_response(
            stream,
            "403 Forbidden",
            "Source folder read+write PIN required or incorrect.\n",
            &[],
            false,
        );
    }

    if !share_allows_write(
        &config.root,
        &destination,
        &destination_metadata,
        Some(&pin),
    )? {
        return write_text_response(
            stream,
            "403 Forbidden",
            "Destination folder read+write PIN required or incorrect.\n",
            &[],
            false,
        );
    }

    let file_name = source
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad source path"))?;
    let target = destination.join(file_name);

    if source == target {
        return write_text_response(stream, "204 No Content", "", &[], false);
    }

    if target.exists() || fs::symlink_metadata(&target).is_ok() {
        return write_text_response(
            stream,
            "409 Conflict",
            "A file or symlink with that name already exists in the destination.\n",
            &[],
            false,
        );
    }

    if let Err(error) = fs::rename(&source, &target) {
        return write_text_response(
            stream,
            "500 Internal Server Error",
            &format!("Could not move item: {error}\n"),
            &[],
            false,
        );
    }

    write_text_response(stream, "204 No Content", "", &[], false)
}

fn move_source(root: &Path, url_path: &str) -> io::Result<(PathBuf, fs::Metadata)> {
    let (path, metadata) = delete_target(root, url_path)?;
    let is_symlink = metadata.file_type().is_symlink();
    if !metadata.is_file() && !is_symlink {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "only files and symlinks can be moved",
        ));
    }

    if path
        .file_name()
        .is_some_and(|name| name == SHARE_FILE || name == UPLOAD_FILE)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "internal Splinterparty files cannot be moved",
        ));
    }

    Ok((path, metadata))
}

fn share_allows_write(
    root: &Path,
    path: &Path,
    metadata: &fs::Metadata,
    pin: Option<&str>,
) -> io::Result<bool> {
    Ok(
        if let Some((_share_dir, share)) = find_applicable_share(root, path, metadata)? {
            share.allows_write(pin)
        } else {
            true
        },
    )
}

fn move_error_status(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::InvalidInput => "400 Bad Request",
        io::ErrorKind::NotFound => "404 Not Found",
        io::ErrorKind::PermissionDenied => "403 Forbidden",
        _ => "500 Internal Server Error",
    }
}

fn handle_copy_route(stream: &mut TcpStream, config: &Config, request: &Request) -> io::Result<()> {
    if request.method != "POST" {
        return write_text_response(
            stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "POST")],
            false,
        );
    }

    let form = String::from_utf8_lossy(&request.body);
    let source_path = form_value(&form, "source").unwrap_or_default();
    let destination_path = form_value(&form, "destination").unwrap_or_default();
    let pin = request_pin(request).unwrap_or_default();

    let (source, source_metadata) = match move_source(&config.root, &source_path) {
        Ok(source) => source,
        Err(error) => {
            return write_text_response(
                stream,
                move_error_status(&error),
                &format!("Could not copy source: {error}\n"),
                &[],
                false,
            );
        }
    };

    let destination = match folder_from_url_path(&config.root, &destination_path) {
        Ok(destination) => destination,
        Err(error) => {
            return write_text_response(
                stream,
                move_error_status(&error),
                &format!("Could not open destination folder: {error}\n"),
                &[],
                false,
            );
        }
    };

    let destination_metadata = fs::metadata(&destination)?;
    if !share_allows_write(
        &config.root,
        &destination,
        &destination_metadata,
        Some(&pin),
    )? {
        return write_text_response(
            stream,
            "403 Forbidden",
            "Destination folder read+write PIN required or incorrect.\n",
            &[],
            false,
        );
    }

    let file_name = source
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad source path"))?;
    let target = destination.join(file_name);
    if target.exists() || fs::symlink_metadata(&target).is_ok() {
        return write_text_response(
            stream,
            "409 Conflict",
            "A file or symlink with that name already exists in the destination.\n",
            &[],
            false,
        );
    }

    if source_metadata.file_type().is_symlink() {
        let link_target = fs::read_link(&source)?;
        #[cfg(unix)]
        {
            if let Err(error) = std::os::unix::fs::symlink(link_target, &target) {
                return write_text_response(
                    stream,
                    "500 Internal Server Error",
                    &format!("Could not copy symlink: {error}\n"),
                    &[],
                    false,
                );
            }
        }

        #[cfg(not(unix))]
        {
            return write_text_response(
                stream,
                "500 Internal Server Error",
                "Copying symlinks is currently supported only on Unix/Linux.\n",
                &[],
                false,
            );
        }
    } else if let Err(error) = fs::copy(&source, &target) {
        return write_text_response(
            stream,
            "500 Internal Server Error",
            &format!("Could not copy file: {error}\n"),
            &[],
            false,
        );
    }

    write_text_response(stream, "204 No Content", "", &[], false)
}

// ── Chunked upload state ──────────────────────────────────────────────────────
//
// When the browser uploads a file larger than LARGE_FILE_PART_SIZE it sends
// each 100 MiB slice as a separate POST to /__chunk.  The server tracks
// progress in a small manifest file (.splinterparty.upload) stored next to the
// in-progress part files.  Once every part has arrived and its SHA-256 matches
// the client-supplied digest the parts are concatenated into the final file and
// all temporary files are removed.

struct ChunkedUploadState {
    filename: String,
    total_parts: u32,
    /// SHA-256 hex strings for parts that have been received, indexed by part index.
    received: BTreeMap<u32, String>,
}

impl ChunkedUploadState {
    fn to_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "version=1");
        let _ = writeln!(out, "filename={}", self.filename);
        let _ = writeln!(out, "total_parts={}", self.total_parts);
        for (index, hash) in &self.received {
            let _ = writeln!(out, "part={index},{hash}");
        }
        out
    }

    fn from_text(text: &str) -> io::Result<Self> {
        let mut filename = None;
        let mut total_parts = None;
        let mut received = BTreeMap::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "filename" => filename = Some(value.to_string()),
                "total_parts" => total_parts = value.parse().ok(),
                "part" => {
                    if let Some((idx, hash)) = value.split_once(',') {
                        if let Ok(idx) = idx.parse::<u32>() {
                            received.insert(idx, hash.to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            filename: filename.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "chunked upload missing filename",
                )
            })?,
            total_parts: total_parts.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "chunked upload missing total_parts",
                )
            })?,
            received,
        })
    }

    fn is_complete(&self) -> bool {
        self.received.len() as u32 == self.total_parts
    }
}

fn handle_chunk_route(
    stream: &mut TcpStream,
    config: &Config,
    request: &Request,
) -> io::Result<()> {
    if request.method != "POST" {
        return write_text_response(
            stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "POST")],
            false,
        );
    }

    // Parse fields from multipart body
    let content_type = request.header("Content-Type").unwrap_or("");
    let boundary = match content_type
        .split(';')
        .map(str::trim)
        .find_map(|p| p.strip_prefix("boundary="))
    {
        Some(b) => b.trim_matches('"').to_string(),
        None => {
            return write_text_response(
                stream,
                "400 Bad Request",
                "Expected multipart/form-data\n",
                &[],
                false,
            );
        }
    };

    let chunk_req = match parse_chunk_request(&request.body, &boundary) {
        Ok(r) => r,
        Err(e) => {
            return write_text_response(
                stream,
                "400 Bad Request",
                &format!("Bad chunk request: {e}\n"),
                &[],
                false,
            );
        }
    };

    let pin = request_pin(request);
    let folder = match folder_from_url_path(&config.root, &chunk_req.path) {
        Ok(f) => f,
        Err(_) => {
            return write_text_response(
                stream,
                "400 Bad Request",
                "Invalid upload path\n",
                &[],
                false,
            );
        }
    };

    // Auth check
    let folder_meta = fs::metadata(&folder)?;
    if let Some((_share_dir, share)) = find_applicable_share(&config.root, &folder, &folder_meta)? {
        if !share.allows_write(pin.as_deref()) {
            return write_text_response(
                stream,
                "403 Forbidden",
                "PIN required to upload here\n",
                &[],
                false,
            );
        }
    }

    // Validate filename
    if !is_safe_folder_name(&chunk_req.filename) {
        return write_text_response(stream, "400 Bad Request", "Invalid filename\n", &[], false);
    }

    // Verify the hash the client sent matches the data we received
    let mut hasher = Sha256::new();
    hasher.update(&chunk_req.data);
    let actual_hash = hex_bytes(&hasher.finish());
    if actual_hash != chunk_req.expected_hash {
        return write_text_response(
            stream,
            "400 Bad Request",
            &format!(
                "Hash mismatch for part {}: expected {}, got {}\n",
                chunk_req.part_index, chunk_req.expected_hash, actual_hash
            ),
            &[],
            false,
        );
    }

    // Derive stable temp-directory name from the filename
    let temp_dir_name = format!("{}.upload-parts", chunk_req.filename);
    let temp_dir = folder.join(&temp_dir_name);
    let state_path = temp_dir.join(UPLOAD_FILE);

    // The final path is selected after assembly. A same-name upload is allowed
    // when the bytes are different; it will get a numbered filename.

    // Load or create state
    let mut state = if state_path.exists() {
        let text = fs::read_to_string(&state_path)?;
        let s = ChunkedUploadState::from_text(&text)?;
        // Guard against a session reusing the same temp dir with a different filename
        if s.filename != chunk_req.filename || s.total_parts != chunk_req.total_parts {
            return write_text_response(
                stream,
                "409 Conflict",
                "Upload session conflict: filename or part count mismatch.\n",
                &[],
                false,
            );
        }
        s
    } else {
        fs::create_dir_all(&temp_dir)?;
        ChunkedUploadState {
            filename: chunk_req.filename.clone(),
            total_parts: chunk_req.total_parts,
            received: BTreeMap::new(),
        }
    };

    // Reject duplicate parts
    if state.received.contains_key(&chunk_req.part_index) {
        return write_text_response(
            stream,
            "409 Conflict",
            &format!("Part {} already received.\n", chunk_req.part_index),
            &[],
            false,
        );
    }

    // Write part file
    let part_path = temp_dir.join(format!("{:08}.part", chunk_req.part_index));
    fs::write(&part_path, &chunk_req.data)?;

    // Record the part in state
    state
        .received
        .insert(chunk_req.part_index, actual_hash.clone());
    fs::write(&state_path, state.to_text())?;

    // If all parts are in, reassemble
    if state.is_complete() {
        let temp_output = folder.join(format!("{}.assembling", chunk_req.filename));
        {
            let mut out = File::create(&temp_output)?;
            for idx in 0..state.total_parts {
                let p = temp_dir.join(format!("{:08}.part", idx));
                let mut part_file = File::open(&p)?;
                io::copy(&mut part_file, &mut out)?;
            }
        }

        let assembled_hash = hash_file(&temp_output)?;
        let final_target = match unique_upload_target(&folder, &chunk_req.filename, &assembled_hash)
        {
            Ok(target) => target,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temp_output);
                let _ = fs::remove_dir_all(&temp_dir);
                return write_text_response(
                    stream,
                    "409 Conflict",
                    "That exact file already exists.\n",
                    &[],
                    false,
                );
            }
            Err(error) => return Err(error),
        };

        fs::rename(&temp_output, &final_target)?;
        // Clean up temp directory
        let _ = fs::remove_dir_all(&temp_dir);

        return write_text_response(stream, "200 OK", "assembled\n", &[], false);
    }

    write_text_response(
        stream,
        "200 OK",
        &format!(
            "part {} of {} received\n",
            chunk_req.part_index + 1,
            chunk_req.total_parts
        ),
        &[],
        false,
    )
}

struct ChunkRequest {
    path: String,
    filename: String,
    part_index: u32,
    total_parts: u32,
    expected_hash: String,
    data: Vec<u8>,
}

fn parse_chunk_request(body: &[u8], boundary: &str) -> io::Result<ChunkRequest> {
    let boundary_bytes = format!("--{boundary}").into_bytes();
    let mut path = None;
    let mut filename = None;
    let mut part_index = None;
    let mut total_parts = None;
    let mut expected_hash = None;
    let mut data = None;

    for raw_part in split_bytes(body, &boundary_bytes).into_iter().skip(1) {
        let mut part = raw_part;
        if part.starts_with(b"\r\n") {
            part = &part[2..];
        }
        if part.starts_with(b"--") {
            break;
        }
        let Some(sep) = find_bytes(part, b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&part[..sep]);
        let mut value = &part[sep + 4..];
        if value.ends_with(b"\r\n") {
            value = &value[..value.len() - 2];
        }
        let disposition = headers
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("content-disposition:"))
            .unwrap_or("");
        let Some(name) = multipart_disposition_value(disposition, "name") else {
            continue;
        };
        match name.as_str() {
            "path" => path = Some(String::from_utf8_lossy(value).to_string()),
            "filename" => filename = Some(String::from_utf8_lossy(value).to_string()),
            "part_index" => part_index = String::from_utf8_lossy(value).trim().parse().ok(),
            "total_parts" => total_parts = String::from_utf8_lossy(value).trim().parse().ok(),
            "expected_hash" => {
                expected_hash = Some(String::from_utf8_lossy(value).trim().to_string())
            }
            "data" => data = Some(value.to_vec()),
            _ => {}
        }
    }

    Ok(ChunkRequest {
        path: path.unwrap_or_else(|| "/".to_string()),
        filename: filename.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing filename in chunk")
        })?,
        part_index: part_index.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing part_index in chunk")
        })?,
        total_parts: total_parts.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing total_parts in chunk")
        })?,
        expected_hash: expected_hash.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing expected_hash in chunk")
        })?,
        data: data
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing data in chunk"))?,
    })
}

// ── End chunked upload ────────────────────────────────────────────────────────

struct UploadRequest {
    path: String,
    filename: String,
    contents: Vec<u8>,
}

fn parse_upload_request(request: &Request) -> io::Result<UploadRequest> {
    let content_type = request.header("Content-Type").unwrap_or("");
    if let Some(boundary) = content_type
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("boundary="))
    {
        return parse_multipart_upload(&request.body, boundary.trim_matches('"'));
    }

    let form = String::from_utf8_lossy(&request.body);
    Ok(UploadRequest {
        path: form_value(&form, "path").unwrap_or_else(|| "/".to_string()),
        filename: form_value(&form, "filename").unwrap_or_default(),
        contents: form_value(&form, "contents")
            .unwrap_or_default()
            .into_bytes(),
    })
}

fn parse_multipart_upload(body: &[u8], boundary: &str) -> io::Result<UploadRequest> {
    let boundary = format!("--{boundary}").into_bytes();
    let mut path = None;
    let mut filename = None;
    let mut contents = None;

    for raw_part in split_bytes(body, &boundary).into_iter().skip(1) {
        let mut part = raw_part;
        if part.starts_with(b"\r\n") {
            part = &part[2..];
        }
        if part.starts_with(b"--") {
            break;
        }

        let Some(separator) = find_bytes(part, b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&part[..separator]);
        let mut value = &part[separator + 4..];
        if value.ends_with(b"\r\n") {
            value = &value[..value.len() - 2];
        }
        let disposition = headers
            .lines()
            .find(|line| {
                line.to_ascii_lowercase()
                    .starts_with("content-disposition:")
            })
            .unwrap_or("");
        let Some(name) = multipart_disposition_value(disposition, "name") else {
            continue;
        };

        match name.as_str() {
            "path" => path = Some(String::from_utf8_lossy(value).to_string()),
            "file" => {
                filename = multipart_disposition_value(disposition, "filename").and_then(|name| {
                    Path::new(&name)
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                });
                contents = Some(value.to_vec());
            }
            _ => {}
        }
    }

    Ok(UploadRequest {
        path: path.unwrap_or_else(|| "/".to_string()),
        filename: filename.unwrap_or_default(),
        contents: contents.unwrap_or_default(),
    })
}

fn split_bytes<'a>(input: &'a [u8], delimiter: &[u8]) -> Vec<&'a [u8]> {
    let mut parts = Vec::new();
    let mut start = 0;
    while let Some(offset) = find_bytes(&input[start..], delimiter) {
        parts.push(&input[start..start + offset]);
        start += offset + delimiter.len();
    }
    parts.push(&input[start..]);
    parts
}

fn find_bytes(input: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > input.len() {
        return None;
    }
    input
        .windows(needle.len())
        .position(|window| window == needle)
}

fn multipart_disposition_value(header: &str, key: &str) -> Option<String> {
    header.split(';').map(str::trim).find_map(|part| {
        let (name, value) = part.split_once('=')?;
        if name == key {
            Some(value.trim_matches('"').to_string())
        } else {
            None
        }
    })
}

fn folder_from_url_path(root: &Path, url_path: &str) -> io::Result<PathBuf> {
    let target = path_for_request(root, url_path)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid folder path"))?;
    let folder = contained_path(root, &target)?;
    if folder.is_dir() {
        Ok(folder)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "share path must be a folder",
        ))
    }
}

fn url_path_for(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    if relative.as_os_str().is_empty() {
        return "/".to_string();
    }

    let mut output = String::from("/");
    for (index, component) in relative.components().enumerate() {
        if index > 0 {
            output.push('/');
        }
        if let Component::Normal(part) = component {
            output.push_str(&url_encode_path_segment(part));
        }
    }
    output
}

fn pin_prompt_html(path: &str, invalid: bool) -> String {
    let mut body = String::new();
    body.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Enter PIN</title><style>");
    body.push_str(DIRECTORY_CSS);
    body.push_str(FORM_CSS);
    body.push_str("</style></head><body><main class=\"form-page\"><section class=\"panel\"><p class=\"eyebrow\">Splinterparty</p><h1>Choose access level</h1><p class=\"muted\">Guest access uses the read PIN. Elevated access uses the read+write PIN. After elevated access is unlocked, actions like delete only ask for confirmation.</p>");
    if invalid {
        body.push_str("<p class=\"error\">PIN required or incorrect.</p>");
    }
    body.push_str(
        "<form method=\"post\" action=\"/__pin\" autocomplete=\"off\"><input type=\"hidden\" name=\"path\" value=\"",
    );
    body.push_str(&escape_html(path));
    body.push_str("\"><label>Access level<select name=\"access\"><option value=\"guest\">Guest / read-only</option><option value=\"elevated\">Elevated / read+write</option></select></label><label>PIN<input name=\"pin\" type=\"password\" autocomplete=\"new-password\" autofocus required></label><button type=\"submit\">Open folder</button></form></section></main></body></html>");
    body
}
fn share_form_html(path: &str, existing: bool, error: Option<&str>) -> String {
    let mut body = String::new();
    body.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Share settings</title><style>");
    body.push_str(DIRECTORY_CSS);
    body.push_str(FORM_CSS);
    body.push_str("</style></head><body><main class=\"form-page\"><section class=\"panel\"><p class=\"eyebrow\">Splinterparty</p><h1>Share settings</h1><p class=\"muted\">");
    body.push_str(&escape_html(path));
    body.push_str("</p>");
    if let Some(error) = error {
        body.push_str("<p class=\"error\">");
        body.push_str(&escape_html(error));
        body.push_str("</p>");
    }
    body.push_str(
        "<form method=\"post\" action=\"/__share\" autocomplete=\"off\"><input type=\"hidden\" name=\"path\" value=\"",
    );
    body.push_str(&escape_html(path));
    body.push_str("\"><label>Recovery passcode<input name=\"recovery\" type=\"password\" autocomplete=\"new-password\" required></label><label>Read PIN <span>optional for sharing downloads</span><input name=\"read_pin\" type=\"password\" autocomplete=\"new-password\"></label><label>Read+write PIN <span>required for owner access</span><input name=\"write_pin\" type=\"password\" autocomplete=\"new-password\" required></label><button type=\"submit\">");
    body.push_str(if existing {
        "Change PINs"
    } else {
        "Create protected share"
    });
    body.push_str("</button></form><p class=\"muted\">Keep the recovery passcode. It is required to change these PINs later.</p></section></main></body></html>");
    body
}

fn folder_form_html(path: &str, error: Option<&str>) -> String {
    let mut body = String::new();
    body.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>New folder</title><style>");
    body.push_str(DIRECTORY_CSS);
    body.push_str(FORM_CSS);
    body.push_str("</style></head><body><main class=\"form-page\"><section class=\"panel\"><p class=\"eyebrow\">Splinterparty</p><h1>New protected folder</h1><p class=\"muted\">");
    body.push_str(&escape_html(path));
    body.push_str("</p>");
    if let Some(error) = error {
        body.push_str("<p class=\"error\">");
        body.push_str(&escape_html(error));
        body.push_str("</p>");
    }
    body.push_str(
        "<form method=\"post\" action=\"/__folder\" autocomplete=\"off\"><input type=\"hidden\" name=\"path\" value=\"",
    );
    body.push_str(&escape_html(path));
    body.push_str("\"><label>Folder name<input name=\"name\" required></label><label>Parent read+write PIN <span>required when creating inside a protected folder</span><input name=\"parent_pin\" type=\"password\" autocomplete=\"new-password\"></label><label>Recovery passcode<input name=\"recovery\" type=\"password\" autocomplete=\"new-password\" required></label><label>Read PIN <span>optional for sharing downloads</span><input name=\"read_pin\" type=\"password\" autocomplete=\"new-password\"></label><label>Read+write PIN <span>required for owner access</span><input name=\"write_pin\" type=\"password\" autocomplete=\"new-password\" required></label><button type=\"submit\">Create folder</button></form><p class=\"muted\">The recovery passcode is required to change this folder's PINs later.</p></section></main></body></html>");
    body
}

fn write_symlink_form_response(
    stream: &mut TcpStream,
    status: &str,
    config: &Config,
    pin: Option<&str>,
    path: &str,
    error: Option<&str>,
    target_path: &str,
    name: &str,
) -> io::Result<()> {
    let body = symlink_form_html_with_values(config, pin, path, error, target_path, name)?;
    write_html_response(stream, status, &body, &[], false)
}

fn symlink_form_html_with_values(
    _config: &Config,
    _pin: Option<&str>,
    path: &str,
    error: Option<&str>,
    target_path: &str,
    name: &str,
) -> io::Result<String> {
    let mut body = String::new();
    body.push_str(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>New symlink</title><style>"#);
    body.push_str(DIRECTORY_CSS);
    body.push_str(FORM_CSS);
    body.push_str(r#"</style></head><body><main class="form-page"><section class="panel"><p class="eyebrow">Splinterparty</p><h1>New symlink</h1><p class="muted">"#);
    body.push_str(&escape_html(path));
    body.push_str("</p>");
    if let Some(error) = error {
        body.push_str(r#"<p class="error">"#);
        body.push_str(&escape_html(error));
        body.push_str("</p>");
    }
    body.push_str(r#"<form method="post" action="/__symlink" autocomplete="off"><label>Destination folder <span>type any folder path you can write to, example: /family</span><input name="path" required placeholder="/family" autocomplete="off" value=""#);
    body.push_str(&escape_html(path));
    body.push_str(r#""></label><label>Symlink name <span>shown in the destination folder</span><input name="name" required placeholder="doc1.pdf" autocomplete="off" value=""#);
    body.push_str(&escape_html(name));
    body.push_str(r#""></label><label>Target path <span>type any existing file or folder path, example: /family/shared.pdf</span><input name="target_path" required placeholder="/family/shared.pdf" autocomplete="off" value=""#);
    body.push_str(&escape_html(target_path));
    body.push_str(r#""></label><label>Destination read+write PIN <span>required if the destination folder is protected</span><input name="parent_pin" type="password" autocomplete="new-password"></label><label>Target read PIN <span>required if the target is inside a protected folder</span><input name="target_pin" type="password" autocomplete="new-password"></label><button type="submit">Create symlink</button></form><p class="muted">A symlink points to an existing file or folder on this same server, so the file is not stored twice. Symlinks are only allowed when their resolved target stays inside the served Splinterparty root.</p><p><a class="up" href=""#);
    body.push_str(&escape_html(path));
    body.push_str(r#"">Cancel</a></p></section></main></body></html>"#);
    Ok(body)
}

fn remote_form_html(path: &str, error: Option<&str>) -> String {
    let mut body = String::new();
    body.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>New remote link</title><style>");
    body.push_str(DIRECTORY_CSS);
    body.push_str(FORM_CSS);
    body.push_str("</style></head><body><main class=\"form-page\"><section class=\"panel\"><p class=\"eyebrow\">Splinterparty</p><h1>New remote link</h1><p class=\"muted\">");
    body.push_str(&escape_html(path));
    body.push_str("</p>");
    if let Some(error) = error {
        body.push_str("<p class=\"error\">");
        body.push_str(&escape_html(error));
        body.push_str("</p>");
    }
    body.push_str(
        "<form method=\"post\" action=\"/__remote\" autocomplete=\"off\"><input type=\"hidden\" name=\"path\" value=\"",
    );
    body.push_str(&escape_html(path));
    body.push_str("\"><label>Link name <span>shown in this folder</span><input name=\"name\" required placeholder=\"Work Drive\" autocomplete=\"off\"></label><label>Remote Splinterparty URL <span>example: http://frankie:8080</span><input name=\"url\" required placeholder=\"http://frankie:8080\" autocomplete=\"off\"></label><label>Remote path <span>example: /mira/doc1 or /general</span><input name=\"remote_path\" required autocomplete=\"off\" value=\"/\"></label><label>Parent read+write PIN <span>required when creating inside a protected folder</span><input name=\"parent_pin\" type=\"password\" autocomplete=\"new-password\"></label><button type=\"submit\">Create remote link</button></form><p class=\"muted\">Remote links are virtual symlinks between Splinterparty instances. The remote instance resolves its own local symlinks before serving files.</p><p><a class=\"up\" href=\"");
    body.push_str(&escape_html(path));
    body.push_str("\">Cancel</a></p></section></main></body></html>");
    body
}

fn delete_form_html(path: &str, error: Option<&str>, require_pin: bool) -> String {
    let mut body = String::new();
    body.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Delete file</title><style>");
    body.push_str(DIRECTORY_CSS);
    body.push_str(FORM_CSS);
    body.push_str("</style></head><body><main class=\"form-page\"><section class=\"panel\"><p class=\"eyebrow\">Splinterparty</p><h1>Delete file</h1><p class=\"muted\">");
    body.push_str(&escape_html(path));
    body.push_str("</p>");
    if let Some(error) = error {
        body.push_str("<p class=\"error\">");
        body.push_str(&escape_html(error));
        body.push_str("</p>");
    }
    body.push_str(
        "<form method=\"post\" action=\"/__delete\" autocomplete=\"off\"><input type=\"hidden\" name=\"path\" value=\"",
    );
    body.push_str(&escape_html(path));
    body.push_str("\">");
    if require_pin {
        body.push_str("<label>Read+write PIN<input name=\"pin\" type=\"password\" autocomplete=\"new-password\" autofocus required></label>");
    } else {
        body.push_str(
            "<p class=\"muted\">This file is outside a protected share, so no PIN is required.</p>",
        );
    }
    body.push_str("<button type=\"submit\">Delete file</button></form><p class=\"muted\">This permanently removes the file from the server directory.</p><p><a class=\"up\" href=\"");
    let parent = path
        .trim_end_matches('/')
        .rsplit_once('/')
        .map(|(parent, _)| if parent.is_empty() { "/" } else { parent })
        .unwrap_or("/");
    body.push_str(&escape_html(parent));
    body.push_str("\">Cancel</a></p></section></main></body></html>");
    body
}

fn unique_upload_target(folder: &Path, filename: &str, content_hash: &str) -> io::Result<PathBuf> {
    for index in 0..10_000_u32 {
        let candidate_name = numbered_filename(filename, index);
        let candidate = folder.join(&candidate_name);

        if !candidate.exists() {
            return Ok(candidate);
        }

        let metadata = fs::metadata(&candidate)?;
        if metadata.is_file() && hash_file(&candidate)? == content_hash {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "an identical file already exists",
            ));
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not find an unused filename",
    ))
}

fn numbered_filename(filename: &str, index: u32) -> String {
    if index == 0 {
        return filename.to_string();
    }

    let path = Path::new(filename);
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap_or(filename);
    let extension = path.extension().and_then(OsStr::to_str);

    match extension {
        Some(extension) if !extension.is_empty() => format!("{stem} ({index}).{extension}"),
        _ => format!("{stem} ({index})"),
    }
}

fn upload_error_html(path: &str, error: &str) -> String {
    let mut body = String::new();
    body.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Upload failed</title><style>");
    body.push_str(DIRECTORY_CSS);
    body.push_str(FORM_CSS);
    body.push_str("</style></head><body><main class=\"form-page\"><section class=\"panel\"><p class=\"eyebrow\">Splinterparty</p><h1>Upload failed</h1><p class=\"error\">");
    body.push_str(&escape_html(error));
    body.push_str("</p><p><a class=\"up\" href=\"");
    body.push_str(&escape_html(path));
    body.push_str("\">Back to folder</a></p></section></main></body></html>");
    body
}

fn write_permission_message(path: &str, error: &io::Error) -> String {
    if error.kind() == io::ErrorKind::PermissionDenied {
        format!(
            "The server process cannot write to {path}. Choose a writable served directory or change the folder permissions."
        )
    } else {
        format!("The server could not write to {path}: {error}.")
    }
}

fn directory_writable(path: &Path) -> bool {
    let probe = path.join(".splinterparty-write-test");
    match File::create(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

fn folder_only_html(path: &str) -> String {
    folder_operation_error_html(path, "Only folders can have PINs or contain new folders.")
}

fn folder_operation_error_html(path: &str, error: &str) -> String {
    let mut body = String::new();
    body.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Folder error</title><style>");
    body.push_str(DIRECTORY_CSS);
    body.push_str(FORM_CSS);
    body.push_str("</style></head><body><main class=\"form-page\"><section class=\"panel\"><p class=\"eyebrow\">Splinterparty</p><h1>Folder error</h1><p class=\"muted\">");
    body.push_str(&escape_html(path));
    body.push_str("</p><p class=\"error\">");
    body.push_str(&escape_html(error));
    body.push_str("</p><p class=\"muted\">If this is a permission problem, make sure the OS user running Splinterparty can write to the served directory.</p><p><a class=\"up\" href=\"");
    body.push_str(&escape_html(path));
    body.push_str("\">Back</a></p></section></main></body></html>");
    body
}

#[derive(Debug, Clone)]
struct RemoteLink {
    name: String,
    url: String,
    path: String,
}

impl RemoteLink {
    fn from_text(input: &str) -> Option<Self> {
        let mut name = None;
        let mut url = None;
        let mut path = None;

        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=')?;
            match key.trim() {
                "name" => name = Some(value.trim().to_string()),
                "url" => url = Some(value.trim().to_string()),
                "path" => path = Some(value.trim().to_string()),
                _ => {}
            }
        }

        let link = Self {
            name: name?,
            url: url?,
            path: path.unwrap_or_else(|| "/".to_string()),
        };
        if is_safe_remote_url(&link.url) && link.path.starts_with('/') {
            Some(link)
        } else {
            None
        }
    }

    fn to_text(&self) -> String {
        format!("name={}\nurl={}\npath={}\n", self.name, self.url, self.path)
    }

    fn href(&self) -> String {
        let url = self.url.trim_end_matches('/');
        if self.path.starts_with('/') {
            format!("{}{}", url, self.path)
        } else {
            format!("{}/{}", url, self.path)
        }
    }
}

fn is_safe_remote_url(url: &str) -> bool {
    (url.starts_with("http://") || url.starts_with("https://"))
        && !url.contains('\n')
        && !url.contains('\r')
}

fn is_safe_remote_link_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\n')
        && !name.contains('\r')
        && name != SHARE_FILE
        && name != UPLOAD_FILE
        && !name.ends_with(REMOTE_LINK_SUFFIX)
}

fn is_safe_folder_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && name != SHARE_FILE
}

fn is_safe_symlink_name(name: &str) -> bool {
    is_safe_folder_name(name)
        && name != UPLOAD_FILE
        && !name.ends_with(REMOTE_LINK_SUFFIX)
        && !name.ends_with(".upload-parts")
}

fn form_value(input: &str, name: &str) -> Option<String> {
    input.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if form_decode(key).ok()? == name {
            form_decode(value).ok()
        } else {
            None
        }
    })
}

fn form_decode(value: &str) -> io::Result<String> {
    let replaced = value.replace('+', " ");
    percent_decode(&replaced)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid form encoding"))
}

fn url_encode_query_value(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            byte => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn path_for_request(root: &Path, target: &str) -> Option<PathBuf> {
    let raw_path = target.split_once('?').map_or(target, |(path, _)| path);
    let decoded = percent_decode(raw_path)?;
    let relative = decoded.trim_start_matches('/');

    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => return None,
        }
    }

    Some(path)
}

fn contained_path(root: &Path, requested_path: &Path) -> io::Result<PathBuf> {
    let canonical = fs::canonicalize(requested_path)?;
    if canonical.starts_with(root) {
        Ok(canonical)
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "resolved path escapes served directory",
        ))
    }
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = hex_value(bytes[index + 1])?;
                let low = hex_value(bytes[index + 2])?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            b'%' => return None,
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn serve_directory(
    stream: &mut TcpStream,
    root: &Path,
    path: &Path,
    bind_port: u16,
    pin: Option<&str>,
    skip_body: bool,
) -> io::Result<()> {
    let mut entries = fs::read_dir(path)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == SHARE_FILE
                || name_str == UPLOAD_FILE
                || name_str.ends_with(".upload-parts")
            {
                None
            } else {
                Some(Ok(entry))
            }
        })
        .collect::<Result<Vec<_>, io::Error>>()?;
    entries.sort_by_key(|entry| {
        (
            entry.file_type().map(|kind| !kind.is_dir()).unwrap_or(true),
            entry.file_name(),
        )
    });

    let relative = path.strip_prefix(root).unwrap_or(path);
    let title = if relative.as_os_str().is_empty() {
        "/".to_string()
    } else {
        format!("/{}", relative.display())
    };
    let entry_count = entries.len();
    let directory_metadata = fs::metadata(path)?;
    let can_write_here = share_allows_write(root, path, &directory_metadata, pin)?;

    let mut body = String::new();
    body.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    body.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">");
    body.push_str("<title>Splinterparty ");
    body.push_str(&escape_html(&title));
    body.push_str("</title><style>");
    body.push_str(DIRECTORY_CSS);
    body.push_str(
        "</style></head><body><main><header><div><p class=\"eyebrow\">Splinterparty</p><h1>",
    );
    body.push_str(&escape_html(&title));
    body.push_str("</h1></div><div class=\"summary\"><span>");
    body.push_str(&entry_count.to_string());
    body.push_str("</span><small>items</small></div></header>");

    if let Some(remote_url) = tailscale_remote_url(bind_port) {
        body.push_str(
            "<section class=\"remote-card\"><strong>Tailscale remote access</strong><code>",
        );
        body.push_str(&escape_html(&remote_url));
        body.push_str("</code><small>Open this URL from another device connected to the same Tailscale network.</small></section>");
    }

    body.push_str("<nav>");
    body.push_str("<a class=\"up\" href=\"/__share?path=");
    body.push_str(&url_encode_query_value(&url_path_for(root, path)));
    body.push_str("\">Share settings</a></nav>");

    body.push_str("<section class=\"browser\"><div class=\"row head\"><span>Name</span><span>Type</span><span>Size</span><span>Modified</span></div>");

    if !relative.as_os_str().is_empty() {
        let parent_path = path.parent().unwrap_or(root);
        let parent_url_path = url_path_for(root, parent_path);
        body.push_str(r#"<div class="row item" data-name=".." data-path=""#);
        body.push_str(&escape_html(&parent_url_path));
        body.push_str(r#"" data-delete-path=""#);
        body.push_str(&escape_html(&parent_url_path));
        body.push_str(r#"" data-is-dir="1" data-download="0" data-delete="0" data-can-symlink="0" data-can-drag="0" data-drop-target=""#);
        body.push_str(if can_write_here { "1" } else { "0" });
        body.push_str(r#""><a class="name" href="../"><span class="icon">DIR</span><span>..</span></a><span class="type">Parent folder</span><span>-</span><span>-</span></div>"#);
    }

    for entry in entries {
        let name = entry.file_name();
        let display_name = name.to_string_lossy();
        if display_name.ends_with(REMOTE_LINK_SUFFIX) {
            if let Ok(contents) = fs::read_to_string(entry.path()) {
                if let Some(link) = RemoteLink::from_text(&contents) {
                    let href = link.href();
                    body.push_str(r#"<div class="row item" data-name=""#);
                    body.push_str(&escape_html(&link.name));
                    body.push_str(r#"" data-path=""#);
                    body.push_str(&escape_html(&href));
                    body.push_str(r#"" data-delete-path=""#);
                    let delete_path = format!(
                        "{}/{}",
                        url_path_for(root, path).trim_end_matches('/'),
                        url_encode_path_segment(&name)
                    );
                    body.push_str(&escape_html(&delete_path));
                    body.push_str(r#"" data-is-dir="0" data-download="1" data-delete="1" data-can-symlink="0" data-can-drag=""#);
                    body.push_str(if can_write_here { "1" } else { "0" });
                    body.push_str(r#"" data-drop-target="0""#);
                    if can_write_here {
                        body.push_str(r#" draggable="true""#);
                    }
                    body.push_str(r#"><a class="name" href=""#);
                    body.push_str(&escape_html(&href));
                    body.push_str(r#""><span class="icon">NET</span><span>"#);
                    body.push_str(&escape_html(&link.name));
                    body.push_str(r#" ↗</span></a><span class="type">Remote link</span><span>-</span><span>-</span></div>"#);
                    continue;
                }
            }
        }
        let symlink_metadata = fs::symlink_metadata(entry.path())?;
        let is_symlink = symlink_metadata.file_type().is_symlink();
        let metadata = entry.metadata()?;
        let is_dir = metadata.is_dir();
        let suffix = if is_dir { "/" } else { "" };
        let type_label = if is_symlink && is_dir {
            "Symlink to folder"
        } else if is_symlink {
            "Symlink to file"
        } else if is_dir {
            "Folder"
        } else if metadata.len() > LARGE_FILE_PART_SIZE {
            "Large file"
        } else {
            "File"
        };
        let size_label = if is_dir {
            "-".to_string()
        } else {
            human_bytes(metadata.len())
        };
        let modified = metadata
            .modified()
            .ok()
            .and_then(system_time_label)
            .unwrap_or_else(|| "-".to_string());
        let href = format!("{}{}", url_encode_path_segment(&name), suffix);

        let item_url_path = format!(
            "{}/{}",
            url_path_for(root, path).trim_end_matches('/'),
            url_encode_path_segment(&name)
        );
        body.push_str("<div class=\"row item\" data-name=\"");
        body.push_str(&escape_html(&display_name));
        body.push_str("\" data-path=\"");
        body.push_str(&escape_html(&item_url_path));
        body.push_str("\" data-delete-path=\"");
        body.push_str(&escape_html(&item_url_path));
        body.push_str("\" data-is-dir=\"");
        body.push_str(if is_dir { "1" } else { "0" });
        body.push_str("\" data-download=\"");
        body.push_str(if !is_dir { "1" } else { "0" });
        body.push_str("\" data-delete=\"");
        body.push_str(if !is_dir || is_symlink { "1" } else { "0" });
        body.push_str("\" data-can-symlink=\"");
        body.push_str(if !is_dir { "1" } else { "0" });
        body.push_str("\" data-can-drag=\"");
        let can_drag = can_write_here && (!is_dir || is_symlink);
        body.push_str(if can_drag { "1" } else { "0" });
        body.push_str("\" data-drop-target=\"");
        body.push_str(if can_write_here && is_dir && !is_symlink {
            "1"
        } else {
            "0"
        });
        if can_drag {
            body.push_str("\" draggable=\"true");
        }
        body.push_str("\"><a class=\"name\" href=\"");
        body.push_str(&href);
        body.push_str("\"><span class=\"icon\">");
        body.push_str(if is_symlink {
            "LINK"
        } else if is_dir {
            "DIR"
        } else {
            "FILE"
        });
        body.push_str("</span><span>");
        body.push_str(&escape_html(&display_name));
        body.push_str(suffix);
        body.push_str("</span></a><span class=\"type ");
        body.push_str(if metadata.len() > LARGE_FILE_PART_SIZE && !is_dir {
            "large"
        } else {
            ""
        });
        body.push_str("\">");
        body.push_str(type_label);
        body.push_str("</span><span>");
        body.push_str(&size_label);
        body.push_str("</span><span>");
        body.push_str(&modified);
        body.push_str("</span></div>");
    }

    if entry_count == 0 {
        body.push_str("<div class=\"empty\">This directory is empty.</div>");
    }

    body.push_str("</section><section class=\"upload-panel\"><h2>Upload file</h2>");
    body.push_str("<form id=\"upload-form\" method=\"post\" action=\"/__upload\" enctype=\"multipart/form-data\">");
    body.push_str("<input type=\"hidden\" name=\"path\" value=\"");
    body.push_str(&escape_html(&url_path_for(root, path)));
    body.push_str("\">");
    body.push_str(
        "<label>File<input id=\"upload-file\" name=\"file\" type=\"file\" required></label>",
    );
    body.push_str("<button type=\"submit\">Upload</button></form>");
    body.push_str("<div id=\"upload-progress\" style=\"display:none;margin-top:12px\">");
    body.push_str(
        "<div id=\"upload-status\" style=\"margin-bottom:6px;font-size:14px;color:#53606f\"></div>",
    );
    body.push_str(
        "<div style=\"height:8px;border-radius:4px;background:#e7eefc;overflow:hidden\">",
    );
    body.push_str("<div id=\"upload-bar\" style=\"height:100%;width:0%;background:#155eef;transition:width 0.2s\"></div>");
    body.push_str("</div></div>");

    // Encode the current folder path for use inside the JS string
    let folder_url_path = escape_html(&url_path_for(root, path));

    body.push_str("<script>");
    body.push_str("(function(){");
    body.push_str("const PART=100*1024*1024;");
    body.push_str("const form=document.getElementById('upload-form');");
    body.push_str("const fileInput=document.getElementById('upload-file');");
    body.push_str("const progress=document.getElementById('upload-progress');");
    body.push_str("const bar=document.getElementById('upload-bar');");
    body.push_str("const status=document.getElementById('upload-status');");
    body.push_str("async function sha256hex(buf){");
    body.push_str("  const digest=await crypto.subtle.digest('SHA-256',buf);");
    body.push_str("  return Array.from(new Uint8Array(digest)).map(b=>b.toString(16).padStart(2,'0')).join('');");
    body.push_str("}");
    body.push_str("form.addEventListener('submit',async function(e){");
    body.push_str("  const file=fileInput.files[0];");
    body.push_str("  if(!file||file.size<=PART){return;}"); // small files use normal form POST
    body.push_str("  e.preventDefault();");
    body.push_str("  form.querySelector('button').disabled=true;");
    body.push_str("  progress.style.display='block';");
    body.push_str("  const totalParts=Math.ceil(file.size/PART);");
    body.push_str("  const folderPath='");
    body.push_str(&folder_url_path);
    body.push_str("';");
    body.push_str("  for(let i=0;i<totalParts;i++){");
    body.push_str("    const start=i*PART;");
    body.push_str("    const slice=file.slice(start,start+PART);");
    body.push_str("    const buf=await slice.arrayBuffer();");
    body.push_str("    const hash=await sha256hex(buf);");
    body.push_str("    status.textContent='Uploading part '+(i+1)+' of '+totalParts+'…';");
    body.push_str("    bar.style.width=(i/totalParts*100).toFixed(1)+'%';");
    body.push_str("    const fd=new FormData();");
    body.push_str("    fd.append('path',folderPath);");
    body.push_str("    fd.append('filename',file.name);");
    body.push_str("    fd.append('part_index',String(i));");
    body.push_str("    fd.append('total_parts',String(totalParts));");
    body.push_str("    fd.append('expected_hash',hash);");
    body.push_str("    fd.append('data',new Blob([buf]));");
    body.push_str("    const resp=await fetch('/__chunk',{method:'POST',body:fd});");
    body.push_str("    if(!resp.ok){");
    body.push_str("      status.textContent='Error on part '+(i+1)+': '+(await resp.text());");
    body.push_str("      status.style.color='#991b1b';");
    body.push_str("      form.querySelector('button').disabled=false;");
    body.push_str("      return;");
    body.push_str("    }");
    body.push_str("  }");
    body.push_str("  bar.style.width='100%';");
    body.push_str("  status.textContent='Upload complete — assembling file…';");
    body.push_str("  setTimeout(()=>window.location.reload(),800);");
    body.push_str("});");
    body.push_str("})();");
    body.push_str("</script>");

    body.push_str(
        r##"<div id="context-menu" class="context-menu">
    <a id="ctx-open" href="#">Open</a>
    <a id="ctx-download" href="#" download>Download</a>
    <button id="ctx-symlink" type="button">Symlink to this file…</button>
    <button id="ctx-copy" type="button">Copy</button>
    <button id="ctx-cut" type="button">Cut</button>
    <button id="ctx-paste" type="button">Paste</button>
    <button id="ctx-folder" type="button">New folder here…</button>
    <a id="ctx-delete" class="danger" href="#">Delete…</a>
    </div>"##,
    );
    body.push_str("<script>");
    body.push_str("(function(){const menu=document.getElementById('context-menu');if(!menu)return;let current=null;const currentFolder='");
    body.push_str(&folder_url_path);
    body.push_str("';const canWrite=");
    body.push_str(if can_write_here { "true" } else { "false" });
    body.push_str(";const clipKey='splinterparty.clipboard';let clip=loadClip();const open=document.getElementById('ctx-open');const down=document.getElementById('ctx-download');const del=document.getElementById('ctx-delete');const sym=document.getElementById('ctx-symlink');const copy=document.getElementById('ctx-copy');const cut=document.getElementById('ctx-cut');const paste=document.getElementById('ctx-paste');const folder=document.getElementById('ctx-folder');function loadClip(){try{return JSON.parse(sessionStorage.getItem(clipKey)||'null');}catch(_){return null;}}function saveClip(value){clip=value;if(value){sessionStorage.setItem(clipKey,JSON.stringify(value));}else{sessionStorage.removeItem(clipKey);}}function hide(){menu.style.display='none';}function folderTarget(){if(current&&current.dataset.isDir==='1')return current.dataset.path;return currentFolder;}function canClip(row){return canWrite&&row&&row.dataset.canDrag==='1';}async function transfer(endpoint,source,destination){const body=new URLSearchParams({source:source,destination:destination});const resp=await fetch(endpoint,{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded'},body:body});if(resp.ok||resp.status===204){window.location.reload();return;}alert((await resp.text())||'Could not complete operation.');}document.addEventListener('click',hide);document.addEventListener('keydown',e=>{if(e.key==='Escape')hide();});document.querySelectorAll('.row.item').forEach(row=>{row.addEventListener('contextmenu',e=>{e.preventDefault();clip=loadClip();current=row;const path=row.dataset.path;open.href=path;down.href=path;down.style.display=row.dataset.download==='1'?'block':'none';del.href='/__delete?path='+encodeURIComponent(row.dataset.deletePath||path);del.style.display=row.dataset.delete==='1'?'block':'none';sym.style.display=row.dataset.canSymlink==='1'?'block':'none';copy.style.display=canClip(row)?'block':'none';cut.style.display=canClip(row)?'block':'none';paste.style.display=canWrite&&clip?'block':'none';folder.style.display=canWrite?'block':'none';menu.style.left=Math.min(e.clientX,window.innerWidth-220)+'px';menu.style.top=Math.min(e.clientY,window.innerHeight-270)+'px';menu.style.display='block';});});sym.addEventListener('click',()=>{if(!current)return;hide();const target=current.dataset.path;const defaultName=current.dataset.name||'link';const name=prompt('Symlink name:', defaultName);if(!name)return;const params=new URLSearchParams({path:currentFolder,target_path:target,name:name});window.location.href='/__symlink?'+params.toString();});copy.addEventListener('click',()=>{if(!canClip(current))return;saveClip({path:current.dataset.path,mode:'copy'});hide();});cut.addEventListener('click',()=>{if(!canClip(current))return;saveClip({path:current.dataset.path,mode:'cut'});hide();});paste.addEventListener('click',()=>{clip=loadClip();if(!canWrite||!clip)return;const dest=folderTarget();const endpoint=clip.mode==='cut'?'/__move':'/__copy';const source=clip.path;if(clip.mode==='cut')saveClip(null);hide();transfer(endpoint,source,dest);});folder.addEventListener('click',()=>{hide();window.location.href='/__folder?path='+encodeURIComponent(folderTarget());});window.splinterTransfer=transfer;})();");
    body.push_str("</script>");
    body.push_str("<script>");
    body.push_str("(function(){let dragged=null;document.querySelectorAll('.row.item').forEach(row=>{if(row.dataset.canDrag==='1'){row.addEventListener('dragstart',e=>{dragged=row;row.classList.add('dragging');e.dataTransfer.effectAllowed='move';e.dataTransfer.setData('text/plain',row.dataset.path);});row.addEventListener('dragend',()=>{row.classList.remove('dragging');dragged=null;document.querySelectorAll('.drop-target').forEach(el=>el.classList.remove('drop-target'));});}if(row.dataset.dropTarget==='1'){row.addEventListener('dragover',e=>{if(!dragged||dragged===row)return;e.preventDefault();e.dataTransfer.dropEffect='move';row.classList.add('drop-target');});row.addEventListener('dragleave',()=>row.classList.remove('drop-target'));row.addEventListener('drop',e=>{if(!dragged||dragged===row)return;e.preventDefault();row.classList.remove('drop-target');const source=dragged.dataset.path;const destination=row.dataset.path;if(window.splinterTransfer)window.splinterTransfer('/__move',source,destination);});}});})();");
    body.push_str("</script>");

    body.push_str("</section></main></body></html>\n");

    write_text_response(
        stream,
        "200 OK",
        &body,
        &[("Content-Type", "text/html; charset=utf-8")],
        skip_body,
    )
}

fn serve_file(
    stream: &mut TcpStream,
    peer: &str,
    request: &Request,
    path: &Path,
    metadata: &fs::Metadata,
    len: u64,
    skip_body: bool,
) -> io::Result<()> {
    let etag = file_etag(metadata);
    if request
        .header("If-None-Match")
        .is_some_and(|value| etag_matches(value, &etag))
    {
        log_request(peer, request, "304");
        write!(
            stream,
            "HTTP/1.1 304 Not Modified\r\nETag: {etag}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n"
        )?;
        return Ok(());
    }

    let range = request
        .header("Range")
        .and_then(|value| parse_range_header(value, len));

    if request.header("Range").is_some() && range.is_none() {
        log_request(peer, request, "416");
        write!(
            stream,
            "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nContent-Range: bytes */{len}\r\nConnection: close\r\n\r\n"
        )?;
        return Ok(());
    }

    let (status, start, end) = match range {
        Some(range) => ("206 Partial Content", range.start, range.end),
        None => ("200 OK", 0, len.saturating_sub(1)),
    };
    log_request(peer, request, if range.is_some() { "206" } else { "200" });
    let content_len = if len == 0 { 0 } else { end - start + 1 };

    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {content_len}\r\nContent-Type: {}\r\nETag: {etag}\r\nAccept-Ranges: bytes\r\n",
        content_type(path)
    )?;
    if range.is_some() {
        write!(stream, "Content-Range: bytes {start}-{end}/{len}\r\n")?;
    }
    write!(stream, "Connection: close\r\n\r\n")?;

    if skip_body {
        return Ok(());
    }

    let mut file = File::open(path)?;
    if start > 0 {
        file.seek(SeekFrom::Start(start))?;
    }

    let mut buffer = [0_u8; READ_BUF_SIZE];
    let mut remaining = content_len;
    loop {
        if remaining == 0 {
            break;
        }

        let read_len = buffer.len().min(remaining as usize);
        let bytes_read = file.read(&mut buffer[..read_len])?;
        if bytes_read == 0 {
            break;
        }
        stream.write_all(&buffer[..bytes_read])?;
        remaining -= bytes_read as u64;
    }

    Ok(())
}

fn write_text_response(
    stream: &mut TcpStream,
    status: &str,
    body: &str,
    headers: &[(&str, &str)],
    skip_body: bool,
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    )?;
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(stream, "\r\n")?;

    if !skip_body {
        stream.write_all(body.as_bytes())?;
    }

    Ok(())
}

fn write_html_response(
    stream: &mut TcpStream,
    status: &str,
    body: &str,
    headers: &[(&str, &str)],
    skip_body: bool,
) -> io::Result<()> {
    let mut owned_headers = vec![("Content-Type", "text/html; charset=utf-8")];
    owned_headers.extend_from_slice(headers);
    write_text_response(stream, status, body, &owned_headers, skip_body)
}

fn write_redirect_with_cookie(
    stream: &mut TcpStream,
    location: &str,
    cookie_name: &str,
    cookie_value: &str,
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 302 Found\r\nLocation: {}\r\nSet-Cookie: {}={}; Path=/; HttpOnly; SameSite=Lax\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        location,
        cookie_name,
        url_encode_query_value(cookie_value)
    )
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(OsStr::to_str) {
        Some("css") => "text/css; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        Some("gif") => "image/gif",
        Some("htm") | Some("html") => "text/html; charset=utf-8",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

fn file_etag(metadata: &fs::Metadata) -> String {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    weak_etag(metadata.len(), modified)
}

fn weak_etag(len: u64, modified_unix_seconds: u64) -> String {
    format!("W/\"{len:x}-{modified_unix_seconds:x}\"")
}

fn etag_matches(header: &str, etag: &str) -> bool {
    header
        .split(',')
        .map(str::trim)
        .any(|value| value == "*" || value == etag)
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn url_encode_path_segment(value: &OsStr) -> String {
    value
        .to_string_lossy()
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            byte => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_path_rejects_parent_components_after_decoding() {
        let root = Path::new("/srv/files");

        assert!(path_for_request(root, "/%2e%2e/secrets.txt").is_none());
        assert!(path_for_request(root, "/nested/../secrets.txt").is_none());
    }

    #[test]
    fn request_path_accepts_normal_relative_paths() {
        let root = Path::new("/srv/files");

        assert_eq!(
            path_for_request(root, "/photos/image%201.jpg?download=1"),
            Some(PathBuf::from("/srv/files/photos/image 1.jpg"))
        );
    }

    #[test]
    fn percent_decode_rejects_invalid_input() {
        assert_eq!(
            percent_decode("/file%20name"),
            Some("/file name".to_string())
        );
        assert_eq!(percent_decode("/bad%2"), None);
        assert_eq!(percent_decode("/bad%xx"), None);
    }

    #[test]
    fn bool_parser_accepts_setup_config_values() {
        assert!(parse_bool("true"));
        assert!(parse_bool("YES"));
        assert!(parse_bool("1"));
        assert!(!parse_bool("false"));
    }

    #[test]
    fn base64_decoder_handles_basic_auth_payloads() {
        assert_eq!(
            base64_decode("c3BsaW50ZXI6c2VjcmV0"),
            Some(b"splinter:secret".to_vec())
        );
        assert_eq!(base64_decode("not valid!"), None);
    }

    #[test]
    fn authorization_accepts_matching_basic_credentials() {
        let request = Request {
            method: "GET".to_string(),
            target: "/".to_string(),
            headers: vec![(
                "Authorization".to_string(),
                "Basic c3BsaW50ZXI6c2VjcmV0".to_string(),
            )],
            body: Vec::new(),
        };
        let auth = AuthConfig {
            username: "splinter".to_string(),
            password: "secret".to_string(),
        };

        assert!(is_authorized(&request, Some(&auth)));
    }

    #[test]
    fn authorization_rejects_missing_or_wrong_credentials() {
        let auth = AuthConfig {
            username: "splinter".to_string(),
            password: "secret".to_string(),
        };
        let missing = Request {
            method: "GET".to_string(),
            target: "/".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let wrong = Request {
            method: "GET".to_string(),
            target: "/".to_string(),
            headers: vec![(
                "Authorization".to_string(),
                "Basic c3BsaW50ZXI6d3Jvbmc=".to_string(),
            )],
            body: Vec::new(),
        };

        assert!(!is_authorized(&missing, Some(&auth)));
        assert!(!is_authorized(&wrong, Some(&auth)));
        assert!(is_authorized(&missing, None));
    }

    #[test]
    fn auth_config_requires_both_username_and_password() {
        assert!(
            AuthConfig::from_parts(Some("admin".to_string()), Some("admin".to_string())).is_some()
        );
        assert!(AuthConfig::from_parts(Some("admin".to_string()), None).is_none());
        assert!(AuthConfig::from_parts(None, Some("admin".to_string())).is_none());
        assert!(AuthConfig::from_parts(Some(String::new()), Some("admin".to_string())).is_none());
    }

    #[test]
    fn share_config_allows_read_or_write_pin_but_not_recovery_for_reading() {
        let share = ShareConfig::new("recover", Some("read"), "write");

        assert!(share.allows_read(Some("read")));
        assert!(share.allows_read(Some("write")));
        assert!(!share.allows_read(Some("recover")));
        assert!(share.allows_write(Some("write")));
        assert!(!share.allows_write(Some("read")));
        assert!(share.allows_recovery("recover"));
    }

    #[test]
    fn share_config_round_trips_without_plaintext_pins() {
        let share = ShareConfig::new("recover", Some("read"), "write");
        let text = share.to_text();

        assert!(!text.contains("=recover"));
        assert!(!text.contains("=read"));
        assert!(!text.contains("=write"));

        let parsed = ShareConfig::from_text(&text).unwrap();
        assert!(parsed.allows_read(Some("read")));
        assert!(parsed.allows_recovery("recover"));
    }

    #[test]
    fn request_pin_reads_query_or_cookie() {
        let query = Request {
            method: "GET".to_string(),
            target: "/folder?pin=1234".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let cookie = Request {
            method: "GET".to_string(),
            target: "/folder".to_string(),
            headers: vec![("Cookie".to_string(), "other=x; sp_pin=abcd".to_string())],
            body: Vec::new(),
        };

        assert_eq!(request_pin(&query), Some("1234".to_string()));
        assert_eq!(request_pin(&cookie), Some("abcd".to_string()));
    }

    #[test]
    fn multipart_upload_parser_keeps_device_file_bytes() {
        let body = b"--boundary\r\nContent-Disposition: form-data; name=\"path\"\r\n\r\n/uploads\r\n--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"photo.bin\"\r\nContent-Type: application/octet-stream\r\n\r\nabc\x00\xff\r\n--boundary--\r\n";

        let upload = parse_multipart_upload(body, "boundary").unwrap();

        assert_eq!(upload.path, "/uploads");
        assert_eq!(upload.filename, "photo.bin");
        assert_eq!(upload.contents, b"abc\x00\xff");
    }

    #[test]
    fn safe_folder_names_reject_path_tricks() {
        assert!(is_safe_folder_name("photos"));
        assert!(is_safe_folder_name("my folder"));
        assert!(is_safe_folder_name("upload.txt"));
        assert!(!is_safe_folder_name(""));
        assert!(!is_safe_folder_name("."));
        assert!(!is_safe_folder_name(".."));
        assert!(!is_safe_folder_name("../outside"));
        assert!(!is_safe_folder_name("nested/folder"));
        assert!(!is_safe_folder_name(SHARE_FILE));
    }

    #[test]
    fn enabled_label_is_human_readable() {
        assert_eq!(enabled_label(true), "enabled");
        assert_eq!(enabled_label(false), "disabled");
    }

    #[test]
    fn sha256_matches_known_vectors() {
        let mut empty = Sha256::new();
        empty.update(b"");
        assert_eq!(
            hex_bytes(&empty.finish()),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        let mut abc = Sha256::new();
        abc.update(b"abc");
        assert_eq!(
            hex_bytes(&abc.finish()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn duplicate_scan_groups_files_by_size_and_hash() {
        let root = env::temp_dir().join(format!("splinterparty-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("a.txt"), b"same").unwrap();
        fs::write(root.join("nested").join("b.txt"), b"same").unwrap();
        fs::write(root.join("c.txt"), b"diff").unwrap();

        let report = find_duplicates(&root).unwrap();

        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].size, 4);
        assert_eq!(report.groups[0].paths.len(), 2);
        assert_eq!(report.duplicate_bytes, 4);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn contained_path_rejects_symlink_escape() {
        let root = env::temp_dir().join(format!("splinterparty-contain-{}", std::process::id()));
        let outside = env::temp_dir().join(format!("splinterparty-outside-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), b"secret").unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("link.txt")).unwrap();
            assert!(contained_path(&root, &root.join("link.txt")).is_err());
        }

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn split_manifest_round_trips() {
        let manifest = SplitManifest {
            original_name: "video.mp4".to_string(),
            original_size: LARGE_FILE_PART_SIZE + 7,
            part_size: LARGE_FILE_PART_SIZE,
            parts: vec![
                SplitPart {
                    index: 0,
                    file_name: "00000000.part".to_string(),
                    size: LARGE_FILE_PART_SIZE,
                    sha256: "a".repeat(64),
                },
                SplitPart {
                    index: 1,
                    file_name: "00000001.part".to_string(),
                    size: 7,
                    sha256: "b".repeat(64),
                },
            ],
        };

        let parsed = SplitManifest::from_text(&manifest.to_text()).unwrap();

        assert_eq!(parsed.original_name, "video.mp4");
        assert_eq!(parsed.original_size, LARGE_FILE_PART_SIZE + 7);
        assert_eq!(parsed.parts.len(), 2);
        assert_eq!(parsed.parts[1].size, 7);
    }

    #[test]
    fn unix_time_formatter_uses_utc_calendar_dates() {
        assert_eq!(format_unix_seconds(0), "1970-01-01 00:00 UTC");
        assert_eq!(format_unix_seconds(1_700_000_000), "2023-11-14 22:13 UTC");
    }

    #[test]
    fn range_parser_accepts_standard_byte_ranges() {
        assert_eq!(
            parse_range_header("bytes=0-4", 10),
            Some(ByteRange { start: 0, end: 4 })
        );
        assert_eq!(
            parse_range_header("bytes=4-", 10),
            Some(ByteRange { start: 4, end: 9 })
        );
        assert_eq!(
            parse_range_header("bytes=-4", 10),
            Some(ByteRange { start: 6, end: 9 })
        );
        assert_eq!(
            parse_range_header("bytes=6-99", 10),
            Some(ByteRange { start: 6, end: 9 })
        );
    }

    #[test]
    fn range_parser_rejects_invalid_ranges() {
        assert_eq!(parse_range_header("items=0-4", 10), None);
        assert_eq!(parse_range_header("bytes=8-4", 10), None);
        assert_eq!(parse_range_header("bytes=10-", 10), None);
        assert_eq!(parse_range_header("bytes=0-1,4-5", 10), None);
        assert_eq!(parse_range_header("bytes=-0", 10), None);
        assert_eq!(parse_range_header("bytes=0-0", 0), None);
    }

    #[test]
    fn weak_etags_are_stable_and_quoted() {
        assert_eq!(weak_etag(4096, 1_700_000_000), "W/\"1000-6553f100\"");
    }

    #[test]
    fn etag_matching_handles_lists_and_wildcards() {
        let etag = weak_etag(12, 34);

        assert!(etag_matches(&etag, &etag));
        assert!(etag_matches("W/\"bad\", W/\"c-22\"", &etag));
        assert!(etag_matches("*", &etag));
        assert!(!etag_matches("W/\"bad\"", &etag));
    }
}
