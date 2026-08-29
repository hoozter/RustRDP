use crate::freerdp::ConnectionCommand;
use crate::model::Profile;
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::process::{ChildStderr, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;
use zeroize::Zeroize;

const ACTIVE_AFTER: Duration = Duration::from_millis(750);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const MAX_TECHNICAL_OUTPUT: usize = 64 * 1024;

#[derive(Clone, Debug)]
pub struct Session {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub profile_name: String,
    pub pid: u32,
    pub state: SessionState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionState {
    Connecting,
    Active,
    Disconnecting,
    Exited(SessionExit),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionExit {
    pub code: Option<i32>,
    pub title: String,
    pub message: String,
    pub technical_details: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("could not start FreeRDP: {0}")]
    Spawn(io::Error),
    #[error("could not pass the password securely to FreeRDP: {0}")]
    CredentialHandoff(io::Error),
    #[error("session not found")]
    NotFound,
}

#[derive(Debug)]
enum Control {
    Disconnect,
}

#[derive(Debug)]
enum Event {
    Active(Uuid),
    Exited(Uuid, SessionExit),
}

pub struct SessionManager {
    sessions: BTreeMap<Uuid, Session>,
    controls: BTreeMap<Uuid, Sender<Control>>,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
}

impl Default for SessionManager {
    fn default() -> Self {
        let (events_tx, events_rx) = unbounded();
        Self {
            sessions: BTreeMap::new(),
            controls: BTreeMap::new(),
            events_tx,
            events_rx,
        }
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        for control in self.controls.values() {
            let _ = control.send(Control::Disconnect);
        }
    }
}

impl SessionManager {
    pub fn sessions(&self) -> impl Iterator<Item = &Session> {
        self.sessions.values()
    }

    pub fn launch(
        &mut self,
        profile: &Profile,
        connection: ConnectionCommand,
        mut password: Option<String>,
    ) -> Result<Uuid, SessionError> {
        let mut command = Command::new(&connection.program);
        command
            .args(&connection.arguments)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .stdin(if connection.password_via_stdin {
                Stdio::piped()
            } else {
                Stdio::null()
            });
        let mut child = command.spawn().map_err(SessionError::Spawn)?;
        if connection.password_via_stdin {
            let handoff = child
                .stdin
                .take()
                .ok_or_else(|| {
                    SessionError::CredentialHandoff(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "FreeRDP stdin was not available",
                    ))
                })
                .and_then(|mut stdin| {
                    let secret = password.as_deref().unwrap_or_default();
                    stdin
                        .write_all(secret.as_bytes())
                        .and_then(|_| stdin.write_all(b"\n"))
                        .and_then(|_| stdin.flush())
                        .map_err(SessionError::CredentialHandoff)
                });
            if let Some(secret) = password.as_mut() {
                secret.zeroize();
            }
            if let Err(error) = handoff {
                let _ = child.kill();
                return Err(error);
            }
        }
        if let Some(secret) = password.as_mut() {
            secret.zeroize();
        }

        let id = Uuid::new_v4();
        let pid = child.id();
        let (control_tx, control_rx) = unbounded();
        let events = self.events_tx.clone();
        let stderr = child.stderr.take();
        thread::Builder::new()
            .name(format!("rustrdp-session-{id}"))
            .spawn(move || watch_process(id, child, stderr, control_rx, events))
            .map_err(SessionError::Spawn)?;
        self.controls.insert(id, control_tx);
        self.sessions.insert(
            id,
            Session {
                id,
                profile_id: profile.id,
                profile_name: profile.name.clone(),
                pid,
                state: SessionState::Connecting,
            },
        );
        Ok(id)
    }

    pub fn disconnect(&mut self, session_id: Uuid) -> Result<(), SessionError> {
        let control = self
            .controls
            .get(&session_id)
            .ok_or(SessionError::NotFound)?;
        control
            .send(Control::Disconnect)
            .map_err(|_| SessionError::NotFound)?;
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.state = SessionState::Disconnecting;
        }
        Ok(())
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events_rx.try_recv() {
            changed = true;
            match event {
                Event::Active(id) => {
                    if let Some(session) = self.sessions.get_mut(&id)
                        && session.state == SessionState::Connecting
                    {
                        session.state = SessionState::Active;
                    }
                }
                Event::Exited(id, exit) => {
                    self.controls.remove(&id);
                    if let Some(session) = self.sessions.get_mut(&id) {
                        session.state = SessionState::Exited(exit);
                    }
                }
            }
        }
        changed
    }

    pub fn dismiss_exited(&mut self, session_id: Uuid) {
        if self
            .sessions
            .get(&session_id)
            .is_some_and(|session| matches!(session.state, SessionState::Exited(_)))
        {
            self.sessions.remove(&session_id);
        }
    }
}

fn watch_process(
    id: Uuid,
    mut child: std::process::Child,
    stderr: Option<ChildStderr>,
    controls: Receiver<Control>,
    events: Sender<Event>,
) {
    let output_reader = stderr.map(|stderr| thread::spawn(move || read_bounded(stderr)));
    let started = Instant::now();
    let mut announced_active = false;
    let mut requested_disconnect = false;
    let status = loop {
        if controls.try_recv().is_ok() {
            requested_disconnect = true;
            let _ = child.kill();
        }
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if !announced_active && started.elapsed() >= ACTIVE_AFTER {
                    let _ = events.send(Event::Active(id));
                    announced_active = true;
                }
            }
            Err(_) => break None,
        }
        thread::sleep(POLL_INTERVAL);
    };
    let technical = output_reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    let code = status.and_then(|status| status.code());
    let exit = classify_exit(code, &technical, requested_disconnect);
    let _ = events.send(Event::Exited(id, exit));
}

fn read_bounded(mut stderr: impl Read) -> String {
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        let Ok(count) = stderr.read(&mut buffer) else {
            break;
        };
        if count == 0 {
            break;
        }
        let remaining = MAX_TECHNICAL_OUTPUT.saturating_sub(retained.len());
        let keep = count.min(remaining);
        retained.extend_from_slice(&buffer[..keep]);
        truncated |= keep < count;
    }
    let mut output = redact_diagnostics(&String::from_utf8_lossy(&retained));
    if truncated {
        output.push_str("\n[additional FreeRDP diagnostics omitted]");
    }
    output
}

fn redact_diagnostics(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.to_ascii_lowercase().contains("password") || line.contains("/p:") {
                "[credential-related diagnostic redacted]".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn classify_exit(code: Option<i32>, technical: &str, requested_disconnect: bool) -> SessionExit {
    let lower = technical.to_ascii_lowercase();
    let (title, message) = if requested_disconnect {
        (
            "Session disconnected",
            "The remote desktop session was closed.",
        )
    } else if lower.contains("logon_failure") || lower.contains("authentication failed") {
        (
            "Sign-in failed",
            "Check the username, domain, and password, then try again.",
        )
    } else if lower.contains("certificate") {
        (
            "Certificate problem",
            "FreeRDP could not verify the remote computer's certificate.",
        )
    } else if lower.contains("name or service not known") || lower.contains("getaddrinfo") {
        (
            "Remote computer not found",
            "Check the hostname or IP address and your network connection.",
        )
    } else if lower.contains("connect failed") || lower.contains("connection refused") {
        (
            "Could not reach the remote computer",
            "Check that it is online and accepts Remote Desktop connections.",
        )
    } else if code == Some(0) {
        (
            "Session ended",
            "The remote desktop session ended normally.",
        )
    } else {
        (
            "Session ended unexpectedly",
            "FreeRDP closed before the session completed.",
        )
    };
    SessionExit {
        code,
        title: title.to_owned(),
        message: message.to_owned(),
        technical_details: technical.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn translates_authentication_failure_and_redacts_secret_diagnostics() {
        let technical = redact_diagnostics("ERRCONNECT_LOGON_FAILURE\npassword: hunter2");
        let exit = classify_exit(Some(131), &technical, false);
        assert_eq!(exit.title, "Sign-in failed");
        assert!(!exit.technical_details.contains("hunter2"));
    }

    #[test]
    fn drains_large_diagnostics_while_bounding_retained_output() {
        let input = vec![b'x'; MAX_TECHNICAL_OUTPUT + 16_384];
        let output = read_bounded(std::io::Cursor::new(input));
        assert!(output.len() < MAX_TECHNICAL_OUTPUT + 100);
        assert!(output.ends_with("[additional FreeRDP diagnostics omitted]"));
    }

    #[test]
    fn tracks_a_real_child_process_to_exit() {
        let profile = Profile::default();
        let command = ConnectionCommand {
            program: PathBuf::from("/bin/sh"),
            arguments: vec![OsString::from("-c"), OsString::from("exit 0")],
            password_via_stdin: false,
        };
        let mut manager = SessionManager::default();
        let id = manager.launch(&profile, command, None).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            manager.poll();
            if matches!(
                manager.sessions.get(&id).map(|session| &session.state),
                Some(SessionState::Exited(_))
            ) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("child process exit was not observed");
    }
}
