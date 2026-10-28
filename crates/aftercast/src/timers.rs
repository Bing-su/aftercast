//! Persistent spell definitions, separate from transient countdowns and retained frames.
use crate::{Error, HostHandle, runtime::Request};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
const HEADER: usize = 32;
const CAPACITY: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimerDefinition {
    pub name: String,
    pub category: String,
    pub only_master_ticks: bool,
    pub restrict_to_category: bool,
    pub remove_value: i32,
    pub panel1_display: bool,
    pub panel2_display: bool,
    pub fill_color_argb: i32,
    pub tooltip: String,
    pub modable: bool,
    pub active_in_list: bool,
    pub radial_display: bool,
    pub timer_value: i32,
    pub warning_value: i32,
    pub absolute_timing: bool,
    pub restrict_to_me: bool,
    pub start_sound_data: String,
    pub warning_sound_data: String,
}
impl Default for TimerDefinition {
    fn default() -> Self {
        Self {
            name: String::new(),
            category: " General".into(),
            only_master_ticks: false,
            restrict_to_category: false,
            remove_value: -15,
            panel1_display: true,
            panel2_display: false,
            fill_color_argb: -16_776_961,
            tooltip: String::new(),
            modable: true,
            active_in_list: true,
            radial_display: true,
            timer_value: 30,
            warning_value: 10,
            absolute_timing: false,
            restrict_to_me: false,
            start_sound_data: String::new(),
            warning_sound_data: String::new(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimerItem {
    pub key: String,
    pub definition: TimerDefinition,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerRevision {
    session: u64,
    value: u64,
}
#[derive(Debug, Clone)]
pub struct TimerBatchPreview {
    pub revision: TimerRevision,
    pub items: Vec<TimerItem>,
    pub conflicts: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawTimerBatchPreview {
    revision: u64,
    items: Vec<TimerItem>,
    conflicts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TimerSnippetPreview {
    pub revision: TimerRevision,
    pub key: String,
    pub exists: bool,
    pub definition: TimerDefinition,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawTimerSnippetPreview {
    revision: u64,
    key: String,
    exists: bool,
    definition: TimerDefinition,
}

#[derive(Debug, Clone)]
pub struct TimerPage {
    pub revision: TimerRevision,
    pub total: u32,
    /// Includes expired frames retained for removal; this is not a count of enabled definitions.
    pub retained_frames: u32,
    pub items: Vec<TimerItem>,
}
#[derive(Debug, Clone)]
pub struct SavedTimer {
    pub revision: TimerRevision,
    pub item: TimerItem,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawSaved {
    revision: u64,
    item: TimerItem,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawPage {
    revision: u64,
    total: u32,
    retained_frames: u32,
    items: Vec<TimerItem>,
}
impl HostHandle {
    pub async fn timer_definitions(&self, offset: u32, limit: u32) -> Result<TimerPage, Error> {
        if !(1..=16).contains(&limit) {
            return Err(invalid("limit must be 1..16"));
        }
        let page: RawPage = self
            .timer_call(1, &serde_json::json!({"offset":offset,"limit":limit}))
            .await?;
        if page.revision == 0
            || page.items.len() > limit as usize
            || page.items.len() > page.total.saturating_sub(offset) as usize
        {
            return Err(invalid("invalid timer definition page"));
        }
        Ok(TimerPage {
            revision: TimerRevision {
                session: self.session,
                value: page.revision,
            },
            total: page.total,
            retained_frames: page.retained_frames,
            items: page.items,
        })
    }
    /// Preview one allowlisted, homogeneous ACT Config section without changes.
    pub async fn preview_timer_config(&self, snippet: &str) -> Result<TimerBatchPreview, Error> {
        if snippet.len() > 16384 || snippet.contains('\0') {
            return Err(invalid("Config exceeds 16 KiB or contains NUL"));
        }
        let raw: RawTimerBatchPreview = self
            .timer_call(6, &serde_json::json!({"snippet": snippet}))
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
            || raw
                .items
                .iter()
                .any(|item| item.key.is_empty() || item.key.len() > 8192)
            || conflicts.len() != raw.conflicts.len()
            || raw.conflicts.iter().any(|key| !keys.contains(key))
        {
            return Err(invalid("invalid Config preview"));
        }
        Ok(TimerBatchPreview {
            revision: TimerRevision {
                session: self.session,
                value: raw.revision,
            },
            items: raw.items,
            conflicts: raw.conflicts,
        })
    }
    /// Atomically add a homogeneous batch; existing definitions are never overwritten.
    pub async fn save_timer_batch(
        &self,
        revision: TimerRevision,
        definitions: &[TimerDefinition],
    ) -> Result<TimerRevision, Error> {
        self.check_timer_revision(revision)?;
        if definitions.is_empty() || definitions.len() > 32 {
            return Err(invalid("select 1–32 definitions"));
        }
        let value: u64 = self
            .timer_call(
                7,
                &serde_json::json!({"revision": revision.value, "definitions": definitions}),
            )
            .await?;
        if value == 0 {
            return Err(invalid("invalid batch revision"));
        }
        Ok(TimerRevision {
            session: self.session,
            value,
        })
    }

    /// Preview one ACT Spell snippet without changing definitions or starting timers.
    pub async fn preview_timer_snippet(&self, snippet: &str) -> Result<TimerSnippetPreview, Error> {
        if snippet.len() > 16384 || snippet.contains('\0') {
            return Err(invalid("timer snippet exceeds 16 KiB or contains NUL"));
        }
        let raw: RawTimerSnippetPreview = self
            .timer_call(5, &serde_json::json!({"snippet": snippet}))
            .await?;
        if raw.revision == 0 || raw.key.is_empty() || raw.key.len() > 4096 {
            return Err(invalid("invalid timer snippet preview"));
        }
        Ok(TimerSnippetPreview {
            revision: TimerRevision {
                session: self.session,
                value: raw.revision,
            },
            key: raw.key,
            exists: raw.exists,
            definition: raw.definition,
        })
    }
    /// Persists before publication. The previous key is the identity returned by timer_definitions.
    pub async fn save_timer_definition(
        &self,
        revision: TimerRevision,
        previous_key: Option<&str>,
        definition: &TimerDefinition,
    ) -> Result<SavedTimer, Error> {
        self.check_timer_revision(revision)?;
        let value: RawSaved = self.timer_call(2,&serde_json::json!({"revision":revision.value,"previousKey":previous_key,"definition":definition})).await?;
        if value.revision == 0 || value.item.key.is_empty() {
            return Err(invalid("invalid saved timer identity"));
        }
        Ok(SavedTimer {
            revision: TimerRevision {
                session: self.session,
                value: value.revision,
            },
            item: value.item,
        })
    }
    pub async fn delete_timer_definition(
        &self,
        revision: TimerRevision,
        key: &str,
    ) -> Result<TimerRevision, Error> {
        self.check_timer_revision(revision)?;
        let value = self
            .timer_call(3, &serde_json::json!({"revision":revision.value,"key":key}))
            .await?;
        Ok(TimerRevision {
            session: self.session,
            value,
        })
    }
    /// Strict private-file replacement. Preserves live countdowns; this is not ACT's partial XML import.
    pub async fn reload_timer_definitions(
        &self,
        revision: TimerRevision,
    ) -> Result<TimerRevision, Error> {
        self.check_timer_revision(revision)?;
        let value = self
            .timer_call(4, &serde_json::json!({"revision":revision.value}))
            .await?;
        Ok(TimerRevision {
            session: self.session,
            value,
        })
    }
    fn check_timer_revision(&self, revision: TimerRevision) -> Result<(), Error> {
        if revision.session != self.session {
            return Err(invalid("revision belongs to another host session"));
        }
        Ok(())
    }
    async fn timer_call<T: DeserializeOwned>(
        &self,
        command: u32,
        value: &impl Serialize,
    ) -> Result<T, Error> {
        let input = serde_json::to_vec(value).map_err(invalid)?;
        if input.len() > 128 * 1024 {
            return Err(invalid("timer input exceeds 128 KiB"));
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
        self.request(Request::Timers, &mut packet).await?;
        let length = u32::from_le_bytes(
            packet
                .get(24..28)
                .ok_or_else(|| invalid("missing reply length"))?
                .try_into()
                .map_err(invalid)?,
        ) as usize;
        if length == 0 || length > CAPACITY {
            return Err(invalid("invalid timer reply length"));
        }
        serde_json::from_slice(
            packet
                .get(HEADER..HEADER + length)
                .ok_or_else(|| invalid("truncated timer reply"))?,
        )
        .map_err(invalid)
    }
}
fn invalid(error: impl std::fmt::Display) -> Error {
    Error::InvalidTimerRequest(error.to_string())
}
