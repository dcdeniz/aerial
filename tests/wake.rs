//! Integration tests for wake notifications (#8) and the `aerial watch` path
//! (#11), exercising the real socket transport end to end.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use aerial::daemon;
use aerial::{Daemon, DaemonRequest, DaemonResponse, WatchEvent};

/// Start a daemon on its own thread and return its socket path once it is
/// accepting connections.
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
        },
    )
    .expect("send");
    match response {
        DaemonResponse::Sent { envelope } => envelope.id,
        other => panic!("unexpected send response: {other:?}"),
    }
}

#[test]
fn watch_receives_event_when_mail_arrives() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = start_daemon(dir.path());

    let (tx, rx) = mpsc::channel();
    let watch_socket = socket.clone();
    thread::spawn(move || {
        let _ = daemon::watch(&watch_socket, "receiver", move |event| {
            let _ = tx.send(event);
        });
    });
    // Let the watcher register its subscription before we send.
    thread::sleep(Duration::from_millis(300));

    let id = send(&socket, "sender", "receiver", "wake up");

    let event = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("watcher should be woken");
    assert_eq!(
        event,
        WatchEvent::Message {
            agent: "receiver".to_owned(),
            id,
        }
    );
}

#[test]
fn late_watcher_is_replayed_pending_mail() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = start_daemon(dir.path());

    // Mail arrives before anyone is watching.
    let id = send(&socket, "sender", "receiver", "queued while offline");

    // Attaching a watcher afterwards must still surface the pending envelope.
    let (tx, rx) = mpsc::channel();
    let watch_socket = socket.clone();
    thread::spawn(move || {
        let _ = daemon::watch(&watch_socket, "receiver", move |event| {
            let _ = tx.send(event);
        });
    });

    let event = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("late watcher should replay pending mail");
    assert_eq!(
        event,
        WatchEvent::Message {
            agent: "receiver".to_owned(),
            id,
        }
    );
}
