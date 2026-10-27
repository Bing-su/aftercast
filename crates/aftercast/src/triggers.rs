//! Managed ACT trigger definitions and bounded result tabs. Mutations persist before publication.
use crate::{Error, HostHandle, runtime::Request};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
const HEADER: usize = 32;
const CAPACITY: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TriggerDefinition {
    pub category: String,
    pub regex: String,
    /// 0 None, 1 Beep, 2 WAV, 3 TTS; expressions in sound_data use .NET Match.Result syntax.
    pub sound_type: i32,
    pub sound_data: String,
    pub active: bool,
    pub restrict_to_category_zone: bool,
    pub timer: bool,
    pub timer_name: String,
    pub tabbed: bool,
}
impl Default for TriggerDefinition {
    fn default() -> Self {
        Self {
            category: " General".into(),
            regex: String::new(),
            sound_type: 0,
            sound_data: String::new(),
            active: true,
            restrict_to_category_zone: false,
            timer: false,
            timer_name: String::new(),
            tabbed: true,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TriggerItem {
    pub key: String,
    pub definition: TriggerDefinition,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerRevision {
    session: u64,
    value: u64,
}
#[derive(Debug, Clone)]
pub struct TriggerPage {
    pub revision: TriggerRevision,
    pub total: u32,
    pub original_lines: bool,
    pub process_imports: bool,
    pub items: Vec<TriggerItem>,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TriggerCapture {
    pub name: String,
    pub value: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TriggerResult {
    pub sequence: u64,
    pub key: String,
    pub category: String,
    pub tab_name: String,
    /// Source wall-clock DateTime ticks; no timezone conversion is implied.
    pub time_ticks: u64,
    pub time_sorter: i32,
    pub is_import: bool,
    pub line: String,
    pub captures: Vec<TriggerCapture>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerResultCursor {
    session: u64,
    sequence: u64,
}
#[derive(Debug, Clone)]
pub struct TriggerResultPage {
    pub latest_sequence: u64,
    pub omitted: u64,
    pub items: Vec<TriggerResult>,
    pub next_cursor: TriggerResultCursor,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawResults {
    latest_sequence: u64,
    omitted: u64,
    items: Vec<TriggerResult>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawPage {
    revision: u64,
    total: u32,
    original_lines: bool,
    process_imports: bool,
    items: Vec<TriggerItem>,
}

impl HostHandle {
    pub async fn triggers(&self, offset: u32, limit: u32) -> Result<TriggerPage, Error> {
        let page: RawPage = self
            .trigger_call(1, &serde_json::json!({"offset": offset, "limit": limit}))
            .await?;
        if page.revision == 0
            || page.items.len() > limit as usize
            || page.items.len() > page.total.saturating_sub(offset) as usize
        {
            return Err(trigger_error("invalid trigger definition page"));
        }
        Ok(TriggerPage {
            revision: TriggerRevision {
                session: self.session,
                value: page.revision,
            },
            total: page.total,
            original_lines: page.original_lines,
            process_imports: page.process_imports,
            items: page.items,
        })
    }
    /// Create rejects an existing ACT category|regex key. Edit supplies the original key; rename rejects collisions.
    pub async fn save_trigger(
        &self,
        revision: TriggerRevision,
        previous_key: Option<&str>,
        definition: &TriggerDefinition,
    ) -> Result<TriggerRevision, Error> {
        self.check_trigger_revision(revision)?;
        let value = self.trigger_call(2, &serde_json::json!({"revision":revision.value,"previousKey":previous_key,"definition":definition})).await?;
        Ok(TriggerRevision {
            session: self.session,
            value,
        })
    }
    pub async fn delete_trigger(
        &self,
        revision: TriggerRevision,
        key: &str,
    ) -> Result<TriggerRevision, Error> {
        self.check_trigger_revision(revision)?;
        let value = self
            .trigger_call(3, &serde_json::json!({"revision":revision.value,"key":key}))
            .await?;
        Ok(TriggerRevision {
            session: self.session,
            value,
        })
    }
    pub async fn save_trigger_options(
        &self,
        revision: TriggerRevision,
        original_lines: bool,
        process_imports: bool,
    ) -> Result<TriggerRevision, Error> {
        self.check_trigger_revision(revision)?;
        let value = self.trigger_call(4, &serde_json::json!({"revision":revision.value,"originalLines":original_lines,"processImports":process_imports})).await?;
        Ok(TriggerRevision {
            session: self.session,
            value,
        })
    }
    pub async fn reload_triggers(
        &self,
        revision: TriggerRevision,
    ) -> Result<TriggerRevision, Error> {
        self.check_trigger_revision(revision)?;
        let value = self
            .trigger_call(5, &serde_json::json!({"revision":revision.value}))
            .await?;
        Ok(TriggerRevision {
            session: self.session,
            value,
        })
    }
    /// Returns ascending results after a sequence, maximum 16. Results are transient and session-owned.
    pub async fn trigger_results(
        &self,
        after: Option<TriggerResultCursor>,
        limit: u32,
    ) -> Result<TriggerResultPage, Error> {
        if after.is_some_and(|cursor| cursor.session != self.session) {
            return Err(trigger_error(
                "result cursor belongs to another host session",
            ));
        }
        let after = after.map_or(0, |cursor| cursor.sequence);
        let page: RawResults = self
            .trigger_call(6, &serde_json::json!({"after":after,"limit":limit}))
            .await?;
        let mut sequence = after;
        for item in &page.items {
            if item.sequence <= sequence
                || item.sequence > page.latest_sequence
                || item.time_ticks > 3_155_378_975_999_999_999
            {
                return Err(trigger_error("invalid trigger result sequence or time"));
            }
            sequence = item.sequence;
        }
        if page.items.len() > limit as usize {
            return Err(trigger_error("trigger page exceeds requested limit"));
        }
        if page.items.is_empty() {
            sequence = page.latest_sequence;
        }
        Ok(TriggerResultPage {
            latest_sequence: page.latest_sequence,
            omitted: page.omitted,
            items: page.items,
            next_cursor: TriggerResultCursor {
                session: self.session,
                sequence,
            },
        })
    }
    pub async fn clear_trigger_results(&self) -> Result<(), Error> {
        let _: u32 = self.trigger_call(7, &serde_json::json!({})).await?;
        Ok(())
    }
    fn check_trigger_revision(&self, revision: TriggerRevision) -> Result<(), Error> {
        if revision.session != self.session {
            return Err(Error::InvalidTriggerRequest(
                "revision belongs to another host session".into(),
            ));
        }
        Ok(())
    }
    async fn trigger_call<T: DeserializeOwned>(
        &self,
        command: u32,
        value: &impl Serialize,
    ) -> Result<T, Error> {
        let input = serde_json::to_vec(value).map_err(trigger_error)?;
        if input.len() > 128 * 1024 {
            return Err(trigger_error("trigger input exceeds 128 KiB"));
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
        self.request(Request::Triggers, &mut packet).await?;
        decode_reply(&packet)
    }
}
fn trigger_error(error: impl std::fmt::Display) -> Error {
    Error::InvalidTriggerRequest(error.to_string())
}
fn decode_reply<T: DeserializeOwned>(packet: &[u8]) -> Result<T, Error> {
    let length = packet
        .get(24..28)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| trigger_error("missing trigger reply length"))? as usize;
    if length == 0 || length > CAPACITY {
        return Err(trigger_error("invalid trigger reply length"));
    }
    let bytes = packet
        .get(HEADER..HEADER + length)
        .ok_or_else(|| trigger_error("truncated trigger reply"))?;
    serde_json::from_slice(bytes).map_err(trigger_error)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_signed_sound_type_is_readable_without_normalizing_plugin_state() {
        let item: TriggerItem = serde_json::from_str(r#"{"key":"x|x","definition":{"category":"x","regex":"x","soundType":-1,"soundData":"","active":true,"restrictToCategoryZone":false,"timer":false,"timerName":"","tabbed":false}}"#)
            .expect("ACT stores SoundType as a signed integer");
        assert_eq!(item.definition.sound_type, -1);
    }
    #[test]
    fn malformed_reply_cannot_escape_its_declared_buffer() {
        let mut packet = vec![0; HEADER];
        decode_reply::<u64>(&packet).expect_err("invalid reply must be rejected");
        packet[24..28].copy_from_slice(&5_u32.to_le_bytes());
        packet.extend_from_slice(b"0");
        decode_reply::<u64>(&packet).expect_err("invalid reply must be rejected");
        packet[24..28].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(decode_reply::<u64>(&packet).expect("one JSON number"), 0);
    }
}
