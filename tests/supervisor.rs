#![cfg(unix)]

use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use aerial::daemon;
use aerial::{Daemon, DaemonRequest, DaemonResponse};

fn start_daemon(dir: &std::path::Path) -> std::path::PathBuf {
    let daemon = Daemon::new(dir).expect("daemon");
    let socket = daemon.socket_path();
    thread::spawn(move || {
        let _ = daemon.serve();
    });

    for _ in 0..200 {
        if socket.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(socket.exists(), "daemon socket never appeared");
    socket
}

fn send(socket: &std::path::Path, from: &str, to: &str, body: &str) -> uuid::Uuid {
    let response = daemon::request(
        socket,
        &DaemonRequest::Send {
            from: from.to_owned(),
            to: to.to_owned(),
            body: body.to_owned(),
            in_reply_to: None,
            create: true,
        },
    )
    .expect("send");
    match response {
        DaemonResponse::Sent { envelope } => envelope.id,
        other => panic!("unexpected send response: {other:?}"),
    }
}

#[test]
fn agent_exec_runs_command_and_acks_on_success() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = start_daemon(dir.path());
    let output = dir.path().join("agent-output.txt");

    let mut child = Command::new(env!("CARGO_BIN_EXE_aerial"))
        .arg("agent")
        .arg("exec")
        .arg("--socket")
        .arg(&socket)
        .arg("--once")
        .arg("receiver")
        .arg("--")
        .arg("sh")
        .arg("-c")
        .arg("printf '%s|%s' \"$AERIAL_MESSAGE_ID\" \"$AERIAL_MESSAGE_BODY\" > \"$AERIAL_OUT\"")
        .env("AERIAL_OUT", &output)
        .spawn()
        .expect("spawn supervisor");

    let id = send(&socket, "sender", "receiver", "wake and work");

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().expect("poll child") {
            assert!(status.success(), "supervisor exited with {status}");
            break;
        }
        assert!(Instant::now() < deadline, "supervisor did not exit");
        thread::sleep(Duration::from_millis(50));
    }

    let captured = std::fs::read_to_string(&output).expect("agent output");
    assert_eq!(captured, format!("{id}|wake and work"));

    let pending = daemon::request(
        &socket,
        &DaemonRequest::Pending {
            agent: "receiver".to_owned(),
        },
    )
    .expect("pending");
    match pending {
        DaemonResponse::Pending { envelopes } => assert!(envelopes.is_empty()),
        other => panic!("unexpected pending response: {other:?}"),
    }
}
