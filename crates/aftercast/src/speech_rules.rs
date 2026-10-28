//! Ordered, persisted ACT-style speech corrections; no audio is played by these APIs.
use crate::{Error, HostHandle, runtime::Request};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};

const HEADER: usize = 32;
const CAPACITY: usize = 4 * 1024 * 1024;
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeechRule {
    pub pattern: String,
    pub replacement: String,
    pub active: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeechRuleRevision {
    session: u64,
    value: u64,
}
#[derive(Clone, Debug)]
pub struct SpeechRuleSnapshot {
    pub revision: SpeechRuleRevision,
    pub rules: Vec<SpeechRule>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Reply {
    revision: u64,
    rules: Vec<SpeechRule>,
}
fn invalid(error: impl std::fmt::Display) -> Error {
    Error::InvalidCorrectionRequest(error.to_string())
}
impl HostHandle {
    pub async fn speech_rules(&self) -> Result<SpeechRuleSnapshot, Error> {
        self.speech_rule_call(3, &serde_json::json!({})).await
    }
    /// Saves ordered rules before publication; no speech is emitted by editing.
    pub async fn save_speech_rules(
        &self,
        revision: SpeechRuleRevision,
        rules: &[SpeechRule],
    ) -> Result<SpeechRuleSnapshot, Error> {
        if revision.session != self.session || revision.value == 0 || rules.len() > 64 {
            return Err(invalid("invalid speech rule revision or count"));
        }
        let expected = revision
            .value
            .checked_add(1)
            .ok_or_else(|| invalid("revision exhausted"))?;
        let result = self
            .speech_rule_call(
                4,
                &serde_json::json!({"revision": revision.value, "speechRules": rules}),
            )
            .await?;
        if result.revision.value != expected || result.rules != rules {
            return Err(invalid("invalid saved speech rules"));
        }
        Ok(result)
    }
    async fn speech_rule_call(
        &self,
        command: u32,
        value: &serde_json::Value,
    ) -> Result<SpeechRuleSnapshot, Error> {
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
        if reply.revision == 0 || reply.rules.len() > 64 {
            return Err(invalid("invalid speech rule snapshot"));
        }
        Ok(SpeechRuleSnapshot {
            revision: SpeechRuleRevision {
                session: self.session,
                value: reply.revision,
            },
            rules: reply.rules,
        })
    }
}
