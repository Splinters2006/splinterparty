use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8080";
const READ_BUF_SIZE: usize = 64 * 1024;

fn main() -> io::Result<()> {
    let config = Config::from_env()?;
    let listener = TcpListener::bind(&config.bind_addr)?;

    println!(
        "serving {} on http://{}",
        config.root.display(),
        config.bind_addr
    );

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, &config.root) {
                    eprintln!("request failed: {error}");
                }
            }
            Err(error) => eprintln!("connection failed: {error}"),
        }
    }

    Ok(())
}

struct Config {
    bind_addr: String,
    root: PathBuf,
}

impl Config {
    fn from_env() -> io::Result<Self> {
        let mut args = env::args().skip(1);
        let root = args
            .next()
            .map(PathBuf::from)
            .unwrap_or(env::current_dir()?);
        let bind_addr = args.next().unwrap_or_else(|| DEFAULT_BIND_ADDR.to_string());

        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "server root must be a directory",
            ));
        }

        Ok(Self { bind_addr, root })
    }
}

fn handle_connection(mut stream: TcpStream, root: &Path) -> io::Result<()> {
    let request = match read_request(&mut stream)? {
        Some(request) => request,
        None => return Ok(()),
    };

    if request.method != "GET" && request.method != "HEAD" {
        return write_text_response(
            &mut stream,
            "405 Method Not Allowed",
            "Method not allowed\n",
            &[("Allow", "GET, HEAD")],
            request.method == "HEAD",
        );
    }

    let path = match path_for_request(root, &request.target) {
        Some(path) => path,
        None => {
            return write_text_response(
                &mut stream,
                "400 Bad Request",
                "Bad request path\n",
                &[],
                request.method == "HEAD",
            );
        }
    };

    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
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

    if metadata.is_dir() {
        return serve_directory(&mut stream, root, &path, request.method == "HEAD");
    }

    if metadata.is_file() {
        return serve_file(&mut stream, &path, metadata.len(), request.method == "HEAD");
    }

    write_text_response(
        &mut stream,
        "403 Forbidden",
        "Unsupported filesystem entry\n",
        &[],
        request.method == "HEAD",
    )
}

struct Request {
    method: String,
    target: String,
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

    Ok(Some(Request { method, target }))
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
    skip_body: bool,
) -> io::Result<()> {
    let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    let relative = path.strip_prefix(root).unwrap_or(path);
    let title = if relative.as_os_str().is_empty() {
        "/".to_string()
    } else {
        format!("/{}", relative.display())
    };

    let mut body = String::new();
    body.push_str("<!doctype html><html><head><meta charset=\"utf-8\">");
    body.push_str("<title>");
    body.push_str(&escape_html(&title));
    body.push_str("</title></head><body><h1>");
    body.push_str(&escape_html(&title));
    body.push_str("</h1><ul>");

    if !relative.as_os_str().is_empty() {
        body.push_str("<li><a href=\"../\">../</a></li>");
    }

    for entry in entries {
        let name = entry.file_name();
        let display_name = name.to_string_lossy();
        let suffix = if entry.file_type()?.is_dir() { "/" } else { "" };

        body.push_str("<li><a href=\"");
        body.push_str(&url_encode_path_segment(&name));
        body.push_str(suffix);
        body.push_str("\">");
        body.push_str(&escape_html(&display_name));
        body.push_str(suffix);
        body.push_str("</a></li>");
    }

    body.push_str("</ul></body></html>\n");

    write_text_response(
        stream,
        "200 OK",
        &body,
        &[("Content-Type", "text/html; charset=utf-8")],
        skip_body,
    )
}

fn serve_file(stream: &mut TcpStream, path: &Path, len: u64, skip_body: bool) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
        content_type(path)
    )?;

    if skip_body {
        return Ok(());
    }

    let mut file = File::open(path)?;
    let mut buffer = [0_u8; READ_BUF_SIZE];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        stream.write_all(&buffer[..bytes_read])?;
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
