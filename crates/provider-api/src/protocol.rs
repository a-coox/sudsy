//! Worker protocol v3: length-prefixed frames carrying MessagePack messages.
//!
//! A frame is a 4-byte little-endian length and that many payload bytes. A message is an
//! externally tagged enum (rmp-serde's default): a one-entry map from the variant name to a
//! payload. Payload structs are MessagePack arrays in field order, mirrored by
//! `workers/csharp/Sudsy.Worker/Protocol/Messages.cs`; change both together and bump
//! [`PROTOCOL_VERSION`].

use core_flow::BodyGraph;
use core_semantics::Answers;
use core_syntax::wire::WireTree;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};

pub const PROTOCOL_VERSION: u32 = 3;
pub const FLOW_GRAPH_FORMAT_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: u32 = 512 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol_version: u32,
    pub grammar_hash: String,
    pub flow_graph_format: u32,
    pub wire_format: u32,
    /// Any of: binding, types, values, flow, project.
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorkspace {
    pub paths: Vec<String>,
    pub properties: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseRequest {
    pub file_id: String,
    pub version: u64,
    pub text: String,
    pub language_version: Option<String>,
    /// Conditional compilation symbols (`#if NAME`); empty for none.
    pub preprocessor_symbols: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateText {
    pub file_id: String,
    pub version: u64,
    pub text: String,
}

/// A batch of binding, type and value questions about one file, answered in one round trip.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    pub file_id: String,
    pub version: u64,
    /// What to answer: `declarations`, `references`, `unused_imports`, `maybe_null`, `symbol_findings`,
    /// `diagnostics`.
    pub items: Vec<String>,
    /// For `diagnostics`: the compiler diagnostic ids to report.
    pub diagnostic_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowGraphs {
    pub file_id: String,
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Graphs {
    pub file_id: String,
    pub version: u64,
    pub bodies: Vec<BodyGraph>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Empty {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireParseError {
    /// Byte offset into the text.
    pub start: u32,
    pub length: u32,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parsed {
    pub file_id: String,
    pub version: u64,
    pub tree: WireTree,
    pub errors: Vec<WireParseError>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolError {
    /// version_mismatch, handshake_required, unsupported, bad_request, internal.
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    Hello(Hello),
    OpenWorkspace(OpenWorkspace),
    Parse(ParseRequest),
    UpdateText(UpdateText),
    Query(Query),
    FlowGraphs(FlowGraphs),
    Shutdown(Empty),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    Hello(Hello),
    Parsed(Parsed),
    Answers(Answers),
    Graphs(Graphs),
    Ok(Empty),
    Error(ProtocolError),
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    #[error("frame of {0} bytes exceeds the limit")]
    TooLarge(u32),
    #[error("malformed message: {0}")]
    Decode(#[from] rmp_serde::decode::Error),
    #[error("cannot encode message: {0}")]
    Encode(#[from] rmp_serde::encode::Error),
}

pub fn write_frame(w: &mut impl Write, payload: &[u8]) -> Result<(), FrameError> {
    let len = u32::try_from(payload.len()).map_err(|_| FrameError::TooLarge(u32::MAX))?;
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(len));
    }
    w.write_all(&len.to_le_bytes())?;
    w.write_all(payload)?;
    w.flush()?;
    Ok(())
}

/// Reads one frame; `Ok(None)` at a clean end of stream.
pub fn read_frame(r: &mut impl Read) -> Result<Option<Vec<u8>>, FrameError> {
    let mut header = [0u8; 4];
    let mut got = 0;
    while got < 4 {
        match r.read(&mut header[got..])? {
            0 if got == 0 => return Ok(None),
            0 => return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into()),
            n => got += n,
        }
    }
    let len = u32::from_le_bytes(header);
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(len));
    }
    let mut payload = vec![0u8; len as usize];
    r.read_exact(&mut payload)?;
    Ok(Some(payload))
}

pub fn send<T: Serialize>(w: &mut impl Write, message: &T) -> Result<(), FrameError> {
    write_frame(w, &rmp_serde::to_vec(message)?)
}

pub fn decode<'a, T: Deserialize<'a>>(frame: &'a [u8]) -> Result<T, FrameError> {
    Ok(rmp_serde::from_slice(frame)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_detect_truncation() {
        let mut buf = Vec::new();
        send(&mut buf, &Request::Shutdown(Empty {})).unwrap();
        let frame = read_frame(&mut &buf[..]).unwrap().unwrap();
        assert_eq!(
            decode::<Request>(&frame).unwrap(),
            Request::Shutdown(Empty {})
        );
        assert!(read_frame(&mut &b""[..]).unwrap().is_none());
        assert!(read_frame(&mut &buf[..buf.len() - 1]).is_err());
        assert!(read_frame(&mut &buf[..2]).is_err());
        assert!(matches!(
            read_frame(&mut &[255u8, 255, 255, 255][..]),
            Err(FrameError::TooLarge(_))
        ));
    }

    #[test]
    fn messages_use_the_documented_layout() {
        // {"Shutdown": []}: a one-entry map, payload struct as an empty array.
        assert_eq!(
            rmp_serde::to_vec(&Request::Shutdown(Empty {})).unwrap(),
            b"\x81\xa8Shutdown\x90"
        );
        let hello = Hello {
            protocol_version: 2,
            grammar_hash: "h".into(),
            flow_graph_format: 1,
            wire_format: 1,
            capabilities: vec![],
        };
        let bytes = rmp_serde::to_vec(&Response::Hello(hello)).unwrap();
        assert_eq!(bytes, b"\x81\xa5Hello\x95\x02\xa1h\x01\x01\x90");
    }
}
