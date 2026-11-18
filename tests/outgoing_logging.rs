use std::{io::Cursor, sync::Mutex};

use log::{LevelFilter, Log, Metadata, Record};
use tungstenite::{protocol::Role, Message, WebSocket};

struct CapturedLogs(Mutex<Option<Vec<String>>>);

impl Log for CapturedLogs {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        if let Some(logs) = self.0.lock().unwrap().as_mut() {
            logs.push(format!("{}: {}", record.target(), record.args()));
        }
    }

    fn flush(&self) {}
}

static LOGS: CapturedLogs = CapturedLogs(Mutex::new(None));

#[test]
fn outgoing_trace_logs_exclude_payloads() {
    log::set_logger(&LOGS).unwrap();
    log::set_max_level(LevelFilter::Trace);

    let payload = "private-websocket-payload";
    let hex_payload: String = payload.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect();

    for (role, peer_role) in [(Role::Client, Role::Server), (Role::Server, Role::Client)] {
        for message in [Message::text(payload), Message::binary(payload.as_bytes())] {
            let mut socket = WebSocket::from_raw_socket(Cursor::new(Vec::new()), role, None);
            *LOGS.0.lock().unwrap() = Some(Vec::new());
            socket.send(message.clone()).unwrap();
            let logs = LOGS.0.lock().unwrap().take().unwrap().join("\n");

            assert!(logs.contains("tungstenite::protocol: Sending frame:"), "{logs}");
            assert!(logs.contains("tungstenite::protocol::frame: writing frame"), "{logs}");
            assert!(!logs.contains(payload), "{logs}");
            assert!(!logs.contains(&hex_payload), "{logs}");

            // Check the wire contents with capture disabled: receive logs are separate.
            let mut peer = WebSocket::from_raw_socket(
                Cursor::new(socket.get_ref().get_ref().clone()),
                peer_role,
                None,
            );
            assert_eq!(peer.read().unwrap(), message);
        }
    }
}
