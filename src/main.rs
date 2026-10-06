use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    env, fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};
static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop(_: i32) {
    STOP.store(true, Ordering::Relaxed);
}
extern "C" {
    fn signal(number: i32, handler: extern "C" fn(i32)) -> usize;
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const PORT: u16 = 47831;
const LIMIT: u64 = 65536;

fn error(message: &str) -> Box<dyn std::error::Error> {
    message.into()
}
fn editor(value: &str) -> Result<&str> {
    match value {
        "zed" | "code" => Ok(value),
        _ => Err(error("editor must be zed or code")),
    }
}
fn valid_path(path: &str) -> Result<()> {
    if !Path::new(path).is_absolute() || path.chars().any(char::is_control) {
        return Err(error(
            "workspace path must be absolute and contain no control characters",
        ));
    }
    Ok(())
}
fn workspace(context: &Value) -> Result<String> {
    for value in [
        context.pointer("/worktree/checkout_path"),
        context.get("workspace_cwd"),
        context.get("focused_pane_cwd"),
    ] {
        if let Some(path) = value.and_then(Value::as_str).filter(|s| !s.is_empty()) {
            valid_path(path)?;
            return Ok(path.into());
        }
    }
    Err(error("Herdr did not provide a workspace path"))
}
fn target_valid(target: &str) -> Result<()> {
    if target.is_empty()
        || target.starts_with('-')
        || !target
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-@".contains(c))
    {
        return Err(error(
            "use an SSH config alias or user@host; configure ports and IPv6 in ~/.ssh/config",
        ));
    }
    Ok(())
}
fn encode(path: &str) -> String {
    path.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn editor_args(editor: &str, target: Option<&str>, path: &str) -> Result<Vec<String>> {
    valid_path(path)?;
    let mut args = Vec::new();
    match (self::editor(editor)?, target) {
        ("zed", Some(host)) => {
            target_valid(host)?;
            args.push(format!("ssh://{host}{}", encode(path)));
        }
        ("code", Some(host)) => {
            target_valid(host)?;
            args.extend(["--remote".into(), format!("ssh-remote+{host}"), path.into()]);
        }
        (_, None) => args.push(path.into()),
        _ => unreachable!(),
    }
    Ok(args)
}
fn launch(editor: &str, target: Option<&str>, path: &str) -> Result<()> {
    let executable = env::var(if editor == "zed" {
        "HERDR_OPEN_ZED_BIN"
    } else {
        "HERDR_OPEN_CODE_BIN"
    })
    .unwrap_or_else(|_| editor.into());
    // The editor CLI returns once it has handed the request to the GUI. Reap it
    // and report its exit status instead of silently accepting failed launches.
    let status = Command::new(executable)
        .args(editor_args(editor, target, path)?)
        .stdin(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(error("editor CLI failed"));
    }
    Ok(())
}
fn read_message(stream: &mut TcpStream) -> Result<Value> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut bytes = Vec::new();
    BufReader::new(stream.take(LIMIT + 1)).read_until(b'\n', &mut bytes)?;
    if bytes.len() as u64 > LIMIT || bytes.last() != Some(&b'\n') {
        return Err(error("relay message exceeds limit or is incomplete"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
fn write_message(stream: &mut TcpStream, value: &Value) -> Result<()> {
    serde_json::to_writer(&mut *stream, value)?;
    stream.write_all(b"\n")?;
    Ok(())
}
fn relay_path() -> Result<PathBuf> {
    Ok(PathBuf::from(
        env::var("HERDR_PLUGIN_CONFIG_DIR")
            .map_err(|_| error("HERDR_PLUGIN_CONFIG_DIR is missing"))?,
    )
    .join("relay.json"))
}
fn request(editor: &str) -> Result<()> {
    self::editor(editor)?;
    let context: Value = serde_json::from_str(
        &env::var("HERDR_PLUGIN_CONTEXT_JSON")
            .map_err(|_| error("run open through a Herdr plugin action"))?,
    )?;
    let path = workspace(&context)?;
    let config_path = relay_path()?;
    if config_path.exists() {
        let config: Value = serde_json::from_slice(&fs::read(config_path)?)?;
        let port = config["port"]
            .as_u64()
            .filter(|p| *p > 0 && *p <= 65535)
            .ok_or("invalid relay port")?;
        let token = config["token"]
            .as_str()
            .filter(|t| !t.is_empty())
            .ok_or("invalid relay token")?;
        let mut stream = TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse()?,
            Duration::from_secs(2),
        )
        .map_err(|_| error("relay unavailable; reconnect with herdr-open attach HOST"))?;
        write_message(
            &mut stream,
            &json!({"version":1,"token":token,"editor":editor,"path":path}),
        )?;
        let reply = read_message(&mut stream)?;
        if reply["ok"] != true {
            return Err(error(
                reply["error"].as_str().unwrap_or("relay rejected request"),
            ));
        }
        return Ok(());
    }
    if ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|key| env::var_os(key).is_some())
    {
        return Err(error("remote session needs herdr-open attach HOST"));
    }
    launch(editor, None, &path)
}
fn handle(stream: &mut TcpStream, target: &str, token: &str) -> Result<()> {
    let message = read_message(stream)?;
    if message["version"] != 1 || message["token"].as_str() != Some(token) {
        return Err(error("invalid relay version or token"));
    }
    launch(
        editor(message["editor"].as_str().unwrap_or(""))?,
        Some(target),
        message["path"].as_str().unwrap_or(""),
    )
}
struct Tunnel {
    control: PathBuf,
    target: String,
}
impl Drop for Tunnel {
    fn drop(&mut self) {
        let _ = Command::new("ssh")
            .args(["-S"])
            .arg(&self.control)
            .args(["-O", "exit", &self.target])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if let Some(dir) = self.control.parent() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}
fn attach(target: &str, port: u16, extra: &[String]) -> Result<i32> {
    target_valid(target)?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let mut random = [0u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut random)?;
    let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let dir = env::temp_dir().join(format!(
        "herdr-open-{}-{}",
        std::process::id(),
        &token[..12]
    ));
    fs::create_dir(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    let tunnel = Tunnel {
        control: dir.join("ctl"),
        target: target.into(),
    };
    let forward = format!(
        "127.0.0.1:{port}:127.0.0.1:{}",
        listener.local_addr()?.port()
    );
    let status = Command::new("ssh")
        .args(["-M", "-S"])
        .arg(&tunnel.control)
        .args([
            "-fNT",
            "-o",
            "ExitOnForwardFailure=yes",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=3",
            "-R",
            &forward,
            target,
        ])
        .status()?;
    if !status.success() {
        return Err(error("SSH reverse forwarding failed; check SSH access and whether another attach uses this port"));
    }
    // Keep secrets off argv and write atomically with owner-only permissions.
    let setup = "set -eu; d=$(herdr plugin config-dir kongdd.open); test -d \"$d\"; umask 077; f=$(mktemp \"$d/relay.json.XXXXXX\"); cat > \"$f\"; mv \"$f\" \"$d/relay.json\"";
    let mut child = Command::new("ssh")
        .arg("-S")
        .arg(&tunnel.control)
        .args([target, setup])
        .stdin(Stdio::piped())
        .spawn()?;
    serde_json::to_writer(
        child.stdin.take().ok_or("SSH stdin missing")?,
        &json!({"port":port,"token":token}),
    )?;
    if !child.wait()?.success() {
        return Err(error("remote setup failed; install kongdd/herdr-open on the server and put herdr on its noninteractive PATH"));
    }
    let host = target.to_string();
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let reply = match handle(&mut stream, &host, &token) {
                        Ok(()) => json!({"ok":true}),
                        Err(e) => json!({"ok":false,"error":e.to_string()}),
                    };
                    let _ = write_message(&mut stream, &reply);
                }
                Err(e) => {
                    eprintln!("herdr-open relay: {e}");
                    break;
                }
            }
        }
    });
    eprintln!("herdr-open: relay ready for {target} (remote port {port})");
    unsafe {
        signal(2, stop);
        signal(15, stop);
        signal(1, stop);
    }
    let mut herdr =
        Command::new(env::var("HERDR_OPEN_HERDR_BIN").unwrap_or_else(|_| "herdr".into()))
            .args(["--remote", target, "--remote-keybindings", "server"])
            .args(extra)
            .spawn()?;
    let status = loop {
        if let Some(status) = herdr.try_wait()? {
            break status;
        }
        if STOP.load(Ordering::Relaxed) {
            let _ = herdr.kill();
            break herdr.wait()?;
        }
        thread::sleep(Duration::from_millis(100));
    };
    drop(tunnel);
    Ok(status.code().unwrap_or(1))
}
fn run() -> Result<i32> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("open") if args.len() == 2 => {
            request(&args[1])?;
            Ok(0)
        }
        Some("local") if args.len() == 3 => {
            launch(editor(&args[1])?, None, &args[2])?;
            Ok(0)
        }
        Some("remote") if args.len() == 4 => {
            launch(editor(&args[1])?, Some(&args[2]), &args[3])?;
            Ok(0)
        }
        Some("reset") if args.len() == 1 => {
            match fs::remove_file(relay_path()?) {
                Ok(()) => (),
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            };
            Ok(0)
        }
        Some("attach") if args.len() >= 2 => {
            let mut port = PORT;
            let mut index = 2;
            if args.get(index).map(String::as_str) == Some("--port") {
                port = args.get(index + 1).ok_or("missing port")?.parse()?;
                if port == 0 {
                    return Err(error("port must be 1–65535"));
                }
                index += 2;
            }
            if args.get(index).map(String::as_str) == Some("--") {
                index += 1;
            } else if args.len() > index {
                return Err(error("put Herdr arguments after --"));
            }
            attach(&args[1], port, &args[index..])
        }
        None | Some("--help" | "-h") => {
            println!("herdr-open open zed|code\nherdr-open attach HOST [--port PORT] [-- HERDR_ARGS...]\nherdr-open local zed|code /path\nherdr-open remote zed|code HOST /path\nherdr-open reset\n\nOverride CLIs with HERDR_OPEN_ZED_BIN, HERDR_OPEN_CODE_BIN, HERDR_OPEN_HERDR_BIN.");
            Ok(0)
        }
        _ => Err(error("invalid arguments; run herdr-open --help")),
    }
}
fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("herdr-open: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection() {
        assert_eq!(workspace(&json!({"worktree":{"checkout_path":"/checkout"},"workspace_cwd":"/workspace","focused_pane_cwd":"/pane"})).unwrap(), "/checkout");
        assert_eq!(
            workspace(&json!({"workspace_cwd":"","focused_pane_cwd":"/pane"})).unwrap(),
            "/pane"
        );
        assert!(workspace(&json!({"workspace_cwd":"relative"})).is_err());
    }
    #[test]
    fn commands() {
        assert_eq!(
            editor_args("zed", Some("nas"), "/repo/中文 #?").unwrap(),
            vec!["ssh://nas/repo/%E4%B8%AD%E6%96%87%20%23%3F"]
        );
        assert_eq!(
            editor_args("code", Some("nas"), "/a b").unwrap(),
            vec!["--remote", "ssh-remote+nas", "/a b"]
        );
        assert!(editor_args("sh", None, "/tmp").is_err());
        assert!(editor_args("zed", Some("-oProxyCommand=x"), "/tmp").is_err());
    }
    #[test]
    fn authentication() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let sender = thread::spawn(move || {
            let mut s = TcpStream::connect(address).unwrap();
            write_message(&mut s, &json!({"version":1,"token":"wrong"})).unwrap();
        });
        let (mut stream, _) = listener.accept().unwrap();
        assert!(handle(&mut stream, "nas", "secret").is_err());
        sender.join().unwrap();
    }
    #[test]
    fn oversized_message() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let sender = thread::spawn(move || {
            let mut s = TcpStream::connect(address).unwrap();
            let _ = s.write_all(&vec![b'x'; LIMIT as usize + 1]);
        });
        let (mut stream, _) = listener.accept().unwrap();
        assert!(read_message(&mut stream).is_err());
        sender.join().unwrap();
    }
}
