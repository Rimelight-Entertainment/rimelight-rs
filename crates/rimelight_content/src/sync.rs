//! The DocRoom wire protocol and a transport-free sync session (EDITOR_API.md, module `sync`).
//!
//! A frame is one kind byte followed by the payload: `0x00` snapshot bytes, `0x01` update bytes, `0x02` presence (plain
//! JSON `{user, name, color, cursor, at}`), `0x03` event (JSON). The room relays bytes unchanged. The session keeps the
//! version last sent, so each call to `outgoing` exports only what is new.
//!
//! On `wasm32` there is no clock, so the `*_at` methods take the time as a parameter. The plain methods exist only on
//! other targets.

use crate::editor::EditorDoc;
use crate::error::Error;
use loro::{ExportMode, VersionVector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

const KIND_SNAPSHOT: u8 = 0x00;
const KIND_UPDATE: u8 = 0x01;
const KIND_PRESENCE: u8 = 0x02;
const KIND_EVENT: u8 = 0x03;

/// A presence entry older than this is no longer shown.
const PRESENCE_TTL_MS: u64 = 30_000;

/// One message of the room protocol.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Snapshot(Vec<u8>),
    Update(Vec<u8>),
    Presence(Vec<u8>),
    Event(Value),
}

/// Encodes a frame: the kind byte, then the payload.
pub fn encode(frame: &Frame) -> Vec<u8> {
    match frame {
        Frame::Snapshot(bytes) => prefixed(KIND_SNAPSHOT, bytes),
        Frame::Update(bytes) => prefixed(KIND_UPDATE, bytes),
        Frame::Presence(bytes) => prefixed(KIND_PRESENCE, bytes),
        Frame::Event(value) => prefixed(KIND_EVENT, &serde_json::to_vec(value).unwrap_or_default()),
    }
}

/// Decodes a frame. An empty input or an unknown kind byte is [`Error::Protocol`].
pub fn decode(bytes: &[u8]) -> Result<Frame, Error> {
    let Some((&kind, payload)) = bytes.split_first() else {
        return Err(Error::Protocol("empty frame".into()));
    };
    match kind {
        KIND_SNAPSHOT => Ok(Frame::Snapshot(payload.to_vec())),
        KIND_UPDATE => Ok(Frame::Update(payload.to_vec())),
        KIND_PRESENCE => Ok(Frame::Presence(payload.to_vec())),
        KIND_EVENT => serde_json::from_slice(payload)
            .map(Frame::Event)
            .map_err(|e| Error::Protocol(e.to_string())),
        other => Err(Error::Protocol(format!("unknown frame kind {other}"))),
    }
}

fn prefixed(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 1);
    out.push(kind);
    out.extend_from_slice(payload);
    out
}

/// This user's caret or selection in one rich text field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub block: String,
    pub field: String,
    pub locale: String,
    pub anchor: usize,
    pub head: usize,
}

/// Another user's presence, as the session shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCursor {
    pub user_id: String,
    pub name: String,
    pub color: String,
    pub cursor: Option<Cursor>,
}

/// What [`SyncSession::receive`] did with a frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Received {
    /// Document bytes were imported; the document changed (or was already up to date).
    DocChanged,
    /// Presence was recorded.
    Presence,
    /// An event frame, passed through for the caller.
    Event(Value),
    /// Nothing to do: this user's own presence, or bytes that did not import.
    Nothing,
}

/// The presence payload on the wire (`0x02`).
#[derive(Serialize, Deserialize)]
struct PresenceWire {
    user: String,
    name: String,
    color: String,
    cursor: Option<Cursor>,
    at: u64,
}

#[derive(Debug, Clone)]
struct RemotePresence {
    name: String,
    color: String,
    cursor: Option<Cursor>,
    at: u64,
}

/// One user's sync state for one document: the version last sent, this user's presence, and the others' presence.
pub struct SyncSession {
    user_id: String,
    name: String,
    color: String,
    sent: VersionVector,
    cursor: Option<Cursor>,
    presence_dirty: bool,
    remote: HashMap<String, RemotePresence>,
}

impl SyncSession {
    pub fn new(user_id: &str, name: &str, color: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
            name: name.to_string(),
            color: color.to_string(),
            sent: VersionVector::new(),
            cursor: None,
            presence_dirty: false,
            remote: HashMap::new(),
        }
    }

    /// Imports a snapshot or update into `doc`, or records presence. Own presence is ignored.
    pub fn receive(&mut self, doc: &EditorDoc, frame: Frame) -> Received {
        match frame {
            Frame::Snapshot(bytes) | Frame::Update(bytes) => {
                if doc.doc().import(&bytes).is_ok() {
                    Received::DocChanged
                } else {
                    Received::Nothing
                }
            }
            Frame::Presence(bytes) => match serde_json::from_slice::<PresenceWire>(&bytes) {
                Ok(wire) if wire.user != self.user_id => {
                    self.remote.insert(
                        wire.user,
                        RemotePresence {
                            name: wire.name,
                            color: wire.color,
                            cursor: wire.cursor,
                            at: wire.at,
                        },
                    );
                    Received::Presence
                }
                _ => Received::Nothing,
            },
            Frame::Event(value) => Received::Event(value),
        }
    }

    /// Frames to send now, with `now_ms` as the presence time: the document updates since the last call, then this
    /// user's presence if the cursor changed.
    pub fn outgoing_at(&mut self, doc: &EditorDoc, now_ms: u64) -> Vec<Frame> {
        let loro = doc.doc();
        loro.commit();
        let mut frames = Vec::new();
        let version = loro.oplog_vv();
        if version != self.sent
            && let Ok(bytes) = loro.export(ExportMode::updates(&self.sent))
        {
            frames.push(Frame::Update(bytes));
            self.sent = version;
        }
        if self.presence_dirty {
            let wire = PresenceWire {
                user: self.user_id.clone(),
                name: self.name.clone(),
                color: self.color.clone(),
                cursor: self.cursor.clone(),
                at: now_ms,
            };
            if let Ok(bytes) = serde_json::to_vec(&wire) {
                frames.push(Frame::Presence(bytes));
            }
            self.presence_dirty = false;
        }
        frames
    }

    /// Sets this user's caret or selection. The next `outgoing` call sends it.
    pub fn set_cursor(&mut self, cursor: Option<Cursor>) {
        self.cursor = cursor;
        self.presence_dirty = true;
    }

    /// Other users' presence that is not older than 30 seconds at `now_ms`.
    pub fn remote_cursors_at(&self, now_ms: u64) -> Vec<RemoteCursor> {
        let mut out: Vec<RemoteCursor> = self
            .remote
            .iter()
            .filter(|(_, presence)| now_ms.saturating_sub(presence.at) <= PRESENCE_TTL_MS)
            .map(|(user_id, presence)| RemoteCursor {
                user_id: user_id.clone(),
                name: presence.name.clone(),
                color: presence.color.clone(),
                cursor: presence.cursor.clone(),
            })
            .collect();
        out.sort_by(|a, b| a.user_id.cmp(&b.user_id));
        out
    }

    /// [`SyncSession::outgoing_at`] with the system clock.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn outgoing(&mut self, doc: &EditorDoc) -> Vec<Frame> {
        self.outgoing_at(doc, now_ms())
    }

    /// [`SyncSession::remote_cursors_at`] with the system clock.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn remote_cursors(&self) -> Vec<RemoteCursor> {
        self.remote_cursors_at(now_ms())
    }
}

/// Milliseconds since the Unix epoch, from the system clock.
#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}
