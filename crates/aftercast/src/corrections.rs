//! Session-owned ACT rename and ability redirect definitions.
use crate::{Error, HostHandle, runtime::Request};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
const HEADER: usize = 32;
const CAPACITY: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameDefinition {
    pub before: String,
    pub after: String,
    pub active: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RedirectDefinition {
    pub ability: String,
    pub destination: String,
    pub r#type: i32,
    pub active: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorrectionRevision {
    session: u64,
    value: u64,
}
#[derive(Debug, Clone)]
pub struct CorrectionSnapshot {
    pub revision: CorrectionRevision,
    pub renames: Vec<RenameDefinition>,
    pub redirects: Vec<RedirectDefinition>,
    /// False means the original file has unsupported content and cannot safely be overwritten.
    pub editable: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Reply {
    revision: u64,
    renames: Vec<RenameDefinition>,
    redirects: Vec<RedirectDefinition>,
    editable: bool,
}
impl HostHandle {
    pub async fn corrections(&self) -> Result<CorrectionSnapshot, Error> {
        self.correction_call(1, &serde_json::json!({})).await
    }
    /// Atomically replaces both rule lists. Existing encounters are not rewritten.
    pub async fn save_corrections(
        &self,
        revision: CorrectionRevision,
        renames: &[RenameDefinition],
        redirects: &[RedirectDefinition],
    ) -> Result<CorrectionSnapshot, Error> {
        if revision.session != self.session || revision.value == 0 {
            return Err(invalid("revision belongs to another session"));
        }
        if renames.len() > 1024 || redirects.len() > 1024 {
            return Err(invalid("too many rules"));
        }
        let expected = revision
            .value
            .checked_add(1)
            .ok_or_else(|| invalid("revision exhausted"))?;
        let result = self.correction_call(
            2,
            &serde_json::json!({"revision":revision.value,"renames":renames,"redirects":redirects}),
        ).await?;
        if result.revision.value != expected || !result.editable {
            return Err(invalid("invalid saved correction revision"));
        }
        Ok(result)
    }
    async fn correction_call(
        &self,
        command: u32,
        value: &serde_json::Value,
    ) -> Result<CorrectionSnapshot, Error> {
        let input = serde_json::to_vec(value).map_err(invalid)?;
        if input.len() > 128 * 1024 {
            return Err(invalid("request exceeds 128 KiB"));
        }
        let mut packet = BytesMut::with_capacity(HEADER + CAPACITY);
        packet.put_u32_le((HEADER + CAPACITY) as u32);
        packet.put_u32_le(1);
        packet.put_u64_le(self.session);
        packet.put_u32_le(command);
        packet.put_u32_le(input.len() as u32);
        packet.put_u64_le(0);
        packet.extend_from_slice(&input);
        packet.resize(HEADER + CAPACITY, 0);
        self.request(Request::Corrections, &mut packet).await?;
        let length = packet
            .get(24..28)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or_else(|| invalid("missing reply length"))? as usize;
        if length == 0 || length > CAPACITY {
            return Err(invalid("invalid reply length"));
        }
        let output = packet
            .get(HEADER..HEADER + length)
            .ok_or_else(|| invalid("truncated reply"))?;
        let reply: Reply = serde_json::from_slice(output).map_err(invalid)?;
        if reply.revision == 0 || reply.renames.len() > 1024 || reply.redirects.len() > 1024 {
            return Err(invalid("invalid correction snapshot"));
        }
        Ok(CorrectionSnapshot {
            revision: CorrectionRevision {
                session: self.session,
                value: reply.revision,
            },
            renames: reply.renames,
            redirects: reply.redirects,
            editable: reply.editable,
        })
    }
}
fn invalid(error: impl std::fmt::Display) -> Error {
    Error::InvalidCorrectionRequest(error.to_string())
}
