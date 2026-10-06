use serde_json::json;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    process::Command,
    thread,
};
#[test]
fn relay_roundtrip_and_local_arguments() {
    let dir = std::env::temp_dir().join(format!("herdr-open-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    fs::write(
        dir.join("relay.json"),
        json!({"port":listener.local_addr().unwrap().port(),"token":"secret"}).to_string(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(&mut stream).read_line(&mut line).unwrap();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["path"], "/repo/中文 space");
        assert_eq!(value["token"], "secret");
        assert_eq!(value["editor"], "zed");
        stream.write_all(b"{\"ok\":true}\n").unwrap();
    });
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-open"))
        .args(["open", "zed"])
        .env("HERDR_PLUGIN_CONFIG_DIR", &dir)
        .env(
            "HERDR_PLUGIN_CONTEXT_JSON",
            json!({"workspace_cwd":"/repo/中文 space"}).to_string(),
        )
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    server.join().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-open"))
        .args(["remote", "code", "nas", "/repo/a b"])
        .env("HERDR_OPEN_CODE_BIN", "/bin/echo")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "--remote ssh-remote+nas /repo/a b"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-open"))
        .args(["local", "zed", "/repo/a b"])
        .env("HERDR_OPEN_ZED_BIN", "/bin/false")
        .output()
        .unwrap();
    assert!(!output.status.success());
    fs::remove_dir_all(dir).unwrap();
}
