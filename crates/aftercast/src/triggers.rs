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
pub struct TriggerBatchPreview {
    pub revision: TriggerRevision,
    pub items: Vec<TriggerItem>,
    pub conflicts: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawTriggerBatchPreview {
    revision: u64,
    items: Vec<TriggerItem>,
    conflicts: Vec<String>,
}

/// Bounded ACT Simple mini graph data, rendered as text and bars by the caller.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MiniGraphValue {
    pub name: String,
    pub value: f64,
}
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MiniGraphSeries {
    pub values: Vec<MiniGraphValue>,
    pub axis: f64,
    pub total: f64,
    pub average: f64,
}
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MiniGraph {
    encounter_id: u64,
    column: String,
    pub no_actors: bool,
    pub data: Option<MiniGraphSeries>,
}

/// An immutable mixed Config preview owned by one host session.
#[derive(Debug, Clone)]
pub struct MixedConfigPreview {
    session: u64,
    trigger_revision: u64,
    timer_revision: u64,
    xml: String,
    pub triggers: Vec<TriggerItem>,
    pub trigger_conflicts: Vec<String>,
    pub timers: Vec<crate::TimerItem>,
    pub timer_conflicts: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MixedResponse {
    triggers: RawTriggerBatchPreview,
    timers: MixedTimers,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MixedTimers {
    revision: u64,
    items: Vec<crate::TimerItem>,
    conflicts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TriggerSnippetPreview {
    pub revision: TriggerRevision,
    pub key: String,
    pub exists: bool,
    pub definition: TriggerDefinition,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawSnippetPreview {
    revision: u64,
    key: String,
    exists: bool,
    definition: TriggerDefinition,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TriggerBenchmarkRow {
    pub key: String,
    pub category: String,
    pub restricted: bool,
    pub milliseconds: u64,
    pub matches: u32,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TriggerBenchmarkResult {
    pub lines: u32,
    pub dropped: u64,
    pub timestamp_length: u32,
    pub baseline_milliseconds: u64,
    pub rows: Vec<TriggerBenchmarkRow>,
}

impl HostHandle {
    /// Benchmark selected compiled expressions against an owned log snapshot; never runs trigger effects.
    pub async fn benchmark_triggers(
        &self,
        revision: TriggerRevision,
        snapshot: crate::ActionSnapshot,
        keys: &[String],
    ) -> Result<TriggerBenchmarkResult, Error> {
        self.check_trigger_revision(revision)?;
        if snapshot.session != self.session
            || keys.is_empty()
            || keys.len() > 32
            || keys.iter().any(|key| key.is_empty() || key.len() > 8192)
            || keys.iter().collect::<std::collections::HashSet<_>>().len() != keys.len()
        {
            return Err(trigger_error("invalid benchmark selection or snapshot"));
        }
        let result: TriggerBenchmarkResult = self.trigger_call(8, &serde_json::json!({"revision": revision.value, "snapshot": snapshot.revision, "keys": keys})).await?;
        if result.lines <= 1000
            || result.lines > 65536
            || result.rows.len() != keys.len()
            || result
                .rows
                .iter()
                .zip(keys)
                .any(|(row, key)| row.key != *key || row.matches > result.lines)
        {
            return Err(trigger_error("invalid benchmark response"));
        }
        Ok(result)
    }

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
    /// Capture a Simple graph without invalidating independent report selections.
    pub async fn mini_graph(
        &self,
        encounter: crate::EncounterId,
        column: &str,
        only_allies: bool,
    ) -> Result<MiniGraph, Error> {
        if encounter.session != self.session
            || encounter.value == 0
            || column.is_empty()
            || column.len() > 256
            || column.chars().any(char::is_control)
        {
            return Err(trigger_error("invalid mini graph encounter or column"));
        }
        let graph: MiniGraph = self.trigger_call(15, &serde_json::json!({"snapshot":encounter.value,"key":column,"originalLines":only_allies})).await?;
        validate_mini_graph(&graph, encounter.value, column)?;
        Ok(graph)
    }
    /// Apply explicit session-only parsing options. Active imports must finish first.
    pub async fn apply_selective_options(
        &self,
        options: &crate::SelectiveOptions,
    ) -> Result<(), Error> {
        options.validate()?;
        let accepted: bool = self.trigger_call(14, &serde_json::json!({"selectiveMode":options.mode as u8,"selected":options.selected})).await?;
        if !accepted {
            return Err(trigger_error("invalid selective options response"));
        }
        Ok(())
    }
    /// Validate both Config sections without changing files or registries.
    pub async fn preview_mixed_config(&self, xml: &str) -> Result<MixedConfigPreview, Error> {
        if xml.len() > 16384 || xml.contains('\0') {
            return Err(trigger_error("invalid mixed Config input"));
        }
        let raw: MixedResponse = self
            .trigger_call(12, &serde_json::json!({"snippet":xml}))
            .await?;
        validate_mixed_preview(raw, self.session, xml)
    }
    /// Explicitly import the exact text that was previewed; a faulted worker must be replaced.
    pub async fn import_mixed_config(&self, preview: &MixedConfigPreview) -> Result<(), Error> {
        if preview.session != self.session
            || !preview.trigger_conflicts.is_empty()
            || !preview.timer_conflicts.is_empty()
        {
            return Err(trigger_error(
                "mixed Config belongs to another session or has conflicts",
            ));
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ImportReply {
            faulted: bool,
            error: String,
        }
        let reply: ImportReply = self
            .trigger_call(
                13,
                &serde_json::json!({"revision":preview.trigger_revision,
            "timerRevision":preview.timer_revision, "snippet":preview.xml}),
            )
            .await?;
        if reply.faulted {
            if reply.error.is_empty() || reply.error.len() > 4096 {
                return Err(trigger_error("invalid mixed Config fault response"));
            }
            return Err(Error::MixedConfigHostFault(reply.error));
        }
        if !reply.error.is_empty() {
            return Err(trigger_error("invalid mixed Config import response"));
        }
        Ok(())
    }

    /// Preview one allowlisted, homogeneous ACT Config section without changes.
    pub async fn preview_trigger_config(
        &self,
        snippet: &str,
    ) -> Result<TriggerBatchPreview, Error> {
        if snippet.len() > 16384 || snippet.contains('\0') {
            return Err(trigger_error("Config exceeds 16 KiB or contains NUL"));
        }
        let raw: RawTriggerBatchPreview = self
            .trigger_call(10, &serde_json::json!({"snippet": snippet}))
            .await?;
        let keys = raw
            .items
            .iter()
            .map(|item| &item.key)
            .collect::<std::collections::HashSet<_>>();
        let conflicts = raw
            .conflicts
            .iter()
            .collect::<std::collections::HashSet<_>>();
        if raw.revision == 0
            || raw.items.is_empty()
            || raw.items.len() > 32
            || keys.len() != raw.items.len()
            || raw.items.iter().any(|item| {
                item.key.is_empty()
                    || item.key.len() > 8192
                    || item.key != format!("{}|{}", item.definition.category, item.definition.regex)
            })
            || conflicts.len() != raw.conflicts.len()
            || raw.conflicts.iter().any(|key| !keys.contains(key))
        {
            return Err(trigger_error("invalid Config preview"));
        }
        Ok(TriggerBatchPreview {
            revision: TriggerRevision {
                session: self.session,
                value: raw.revision,
            },
            items: raw.items,
            conflicts: raw.conflicts,
        })
    }
    /// Atomically add a homogeneous batch; existing definitions are never overwritten.
    pub async fn save_trigger_batch(
        &self,
        revision: TriggerRevision,
        definitions: &[TriggerDefinition],
    ) -> Result<TriggerRevision, Error> {
        self.check_trigger_revision(revision)?;
        if definitions.is_empty() || definitions.len() > 32 {
            return Err(trigger_error("select 1–32 definitions"));
        }
        let value: u64 = self
            .trigger_call(
                11,
                &serde_json::json!({"revision": revision.value, "definitions": definitions}),
            )
            .await?;
        if value == 0 {
            return Err(trigger_error("invalid batch revision"));
        }
        Ok(TriggerRevision {
            session: self.session,
            value,
        })
    }

    /// Preview one ACT Trigger snippet without changing definitions or running effects.
    pub async fn preview_trigger_snippet(
        &self,
        snippet: &str,
    ) -> Result<TriggerSnippetPreview, Error> {
        if snippet.len() > 16384 || snippet.contains('\0') {
            return Err(trigger_error(
                "trigger snippet exceeds 16 KiB or contains NUL",
            ));
        }
        let raw: RawSnippetPreview = self
            .trigger_call(9, &serde_json::json!({"snippet": snippet}))
            .await?;
        if raw.revision == 0
            || raw.key != format!("{}|{}", raw.definition.category, raw.definition.regex)
        {
            return Err(trigger_error("invalid trigger snippet preview"));
        }
        Ok(TriggerSnippetPreview {
            revision: TriggerRevision {
                session: self.session,
                value: raw.revision,
            },
            key: raw.key,
            exists: raw.exists,
            definition: raw.definition,
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
fn validate_mini_graph(graph: &MiniGraph, encounter: u64, column: &str) -> Result<(), Error> {
    if graph.encounter_id != encounter
        || graph.column != column
        || graph.data.as_ref().is_some_and(|data| {
            data.values.is_empty()
                || data.values.len() > 16384
                || !data.axis.is_finite()
                || data.axis <= 0.0
                || !data.total.is_finite()
                || !data.average.is_finite()
                || data.values.iter().any(|item| {
                    item.name.len() > 8192
                        || !item.value.is_finite()
                        || item.value <= 0.0
                        || item.value > data.axis
                })
        })
        || graph.no_actors && graph.data.is_some()
    {
        return Err(trigger_error("invalid mini graph response"));
    }
    Ok(())
}
fn validate_mixed_preview(
    raw: MixedResponse,
    session: u64,
    xml: &str,
) -> Result<MixedConfigPreview, Error> {
    let triggers = &raw.triggers;
    let timers = &raw.timers;
    let tk: std::collections::HashSet<_> = triggers.items.iter().map(|item| &item.key).collect();
    let mk: std::collections::HashSet<_> = timers.items.iter().map(|item| &item.key).collect();
    let tc: std::collections::HashSet<_> = triggers.conflicts.iter().collect();
    let mc: std::collections::HashSet<_> = timers.conflicts.iter().collect();
    if triggers.revision == 0
        || timers.revision == 0
        || triggers.items.is_empty()
        || timers.items.is_empty()
        || triggers.items.len() + timers.items.len() > 32
        || tk.len() != triggers.items.len()
        || mk.len() != timers.items.len()
        || tc.len() != triggers.conflicts.len()
        || mc.len() != timers.conflicts.len()
        || triggers.items.iter().any(|i| {
            i.key.is_empty()
                || i.key.len() > 8192
                || i.key != format!("{}|{}", i.definition.category, i.definition.regex)
        })
        || timers
            .items
            .iter()
            .any(|i| i.key.is_empty() || i.key.len() > 8192)
        || triggers.conflicts.iter().any(|key| !tk.contains(key))
        || timers.conflicts.iter().any(|key| !mk.contains(key))
    {
        return Err(trigger_error("invalid mixed Config preview"));
    }
    Ok(MixedConfigPreview {
        session,
        trigger_revision: triggers.revision,
        timer_revision: timers.revision,
        xml: xml.to_owned(),
        triggers: raw.triggers.items,
        trigger_conflicts: raw.triggers.conflicts,
        timers: raw.timers.items,
        timer_conflicts: raw.timers.conflicts,
    })
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
    fn mini_graph_response_rejects_malformed_values_and_identity() {
        let valid = serde_json::json!({"encounterId":1,"column":"Damage","noActors":false,"data":{"values":[{"name":"A","value":100.0}],"axis":110.0,"total":100.0,"average":100.0}});
        let parse = |value| serde_json::from_value::<MiniGraph>(value).expect("graph JSON");
        validate_mini_graph(&parse(valid.clone()), 1, "Damage").expect("valid graph");
        for bad in [
            {
                let mut v = valid.clone();
                v["encounterId"] = 2.into();
                v
            },
            {
                let mut v = valid.clone();
                v["column"] = "other".into();
                v
            },
            {
                let mut v = valid.clone();
                v["noActors"] = true.into();
                v
            },
            {
                let mut v = valid.clone();
                v["data"]["axis"] = 0.into();
                v
            },
            {
                let mut v = valid.clone();
                v["data"]["values"][0]["value"] = (-1).into();
                v
            },
            {
                let mut v = valid.clone();
                v["data"]["values"][0]["value"] = 111.into();
                v
            },
        ] {
            validate_mini_graph(&parse(bad), 1, "Damage").expect_err("invalid graph");
        }
        let mut extra = valid;
        extra["unexpected"] = true.into();
        serde_json::from_value::<MiniGraph>(extra).expect_err("unknown graph field");
    }
    #[test]
    fn mixed_preview_rejects_malformed_worker_responses() {
        let trigger = TriggerDefinition {
            category: "x".into(),
            regex: "y".into(),
            ..Default::default()
        };
        let timer = crate::TimerDefinition {
            name: "z".into(),
            category: "x".into(),
            ..Default::default()
        };
        let valid = serde_json::json!({
            "triggers": {"revision": 1, "items": [{"key":"x|y", "definition":trigger}], "conflicts":[]},
            "timers": {"revision": 2, "items": [{"key":"z", "definition":timer}], "conflicts":[]}
        });
        let parse = |v: serde_json::Value| serde_json::from_value::<MixedResponse>(v).unwrap();
        validate_mixed_preview(parse(valid.clone()), 4, "original").expect("valid mixed preview");
        for bad in [
            {
                let mut v = valid.clone();
                v["triggers"]["revision"] = 0.into();
                v
            },
            {
                let mut v = valid.clone();
                v["timers"]["revision"] = 0.into();
                v
            },
            {
                let mut v = valid.clone();
                v["triggers"]["items"][0]["key"] = "wrong".into();
                v
            },
            {
                let mut v = valid.clone();
                v["timers"]["conflicts"] = serde_json::json!(["unknown"]);
                v
            },
            {
                let mut v = valid.clone();
                v["timers"]["items"] = serde_json::json!([]);
                v
            },
        ] {
            validate_mixed_preview(parse(bad), 4, "original").expect_err("malformed mixed preview");
        }
        let mut extra = valid;
        extra["unexpected"] = true.into();
        assert!(serde_json::from_value::<MixedResponse>(extra).is_err());
    }
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
