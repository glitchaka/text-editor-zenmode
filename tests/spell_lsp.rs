use std::{
    io::Write,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[test]
fn spell_lsp_process_advertises_code_actions() {
    let exe = env!("CARGO_BIN_EXE_helix-sst-zen");
    let dictionary =
        std::env::temp_dir().join(format!("helix-sst-spell-smoke-{}.dic", std::process::id()));

    let mut child = Command::new(exe)
        .arg("--helix-sst-spell")
        .arg(&dictionary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("el helper ortográfico debe poder arrancar");

    let initialize = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}"#;
    let exit = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

    {
        let mut stdin = child.stdin.take().expect("stdin del helper");
        for message in [initialize, shutdown, exit] {
            write!(stdin, "Content-Length: {}\r\n\r\n{}", message.len(), message)
                .expect("debe poder enviarse una trama LSP");
        }
    }

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = child.kill();
                panic!("el helper ortográfico no finalizó tras shutdown/exit");
            }
            Err(error) => panic!("no se pudo consultar el estado del helper: {error}"),
        }
    }

    let output = child
        .wait_with_output()
        .expect("el helper ortográfico debe finalizar tras exit");

    assert!(
        output.status.success(),
        "el helper ortográfico falló: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"codeActionProvider\":true"),
        "initialize no anunció codeActionProvider: {stdout}"
    );
    assert!(
        stdout.contains("\"name\":\"helix-sst-spell\""),
        "initialize no anunció el servidor esperado: {stdout}"
    );

    let _ = std::fs::remove_file(dictionary);
}
