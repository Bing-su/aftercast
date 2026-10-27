//! Owned ordinary text reports; plugin callbacks run once in the originating worker.
use crate::{EncounterId, Error, HostHandle, runtime::Request};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
const HEADER: usize = 32;
const CAPACITY: usize = 16 * 1024 * 1024;
const MAX_TEXT: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextReportRevision {
    session: u64,
    value: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextReportCatalog {
    pub revision: TextReportRevision,
    pub player_variables: Vec<String>,
    pub allied_variables: Vec<String>,
    /// Registration keys, not displayed labels. Unknown keys retain ACT's damage-sort fallback.
    pub sort_columns: Vec<String>,
    /// ACT's built-in format (the legacy API's negative format index).
    pub default_options: TextReportOptions,
    /// Registered options in catalog order. Copy into a draft; never re-resolve an old index.
    pub presets: Vec<TextReportPreset>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextReportOptions {
    pub player_format: String,
    pub sorting: String,
    pub show_only_allies: bool,
    pub show_allied_info: bool,
    pub allies_format: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextReportPreset {
    pub index: u32,
    pub options: TextReportOptions,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextReport {
    pub encounter: EncounterId,
    pub revision: TextReportRevision,
    /// UTF-8 text with ACT's LF framing; no file or clipboard write occurs.
    pub text: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogReply {
    revision: u64,
    player_variables: Vec<String>,
    allied_variables: Vec<String>,
    sort_columns: Vec<String>,
    default_options: TextReportOptions,
    presets: Vec<TextReportPreset>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReportReply {
    encounter_id: u64,
    revision: u64,
    text: String,
}
impl TextReportOptions {
    fn validate(&self) -> Result<(), Error> {
        if self.player_format.len() > 16384
            || self.allies_format.len() > 16384
            || self.sorting.len() > 256
            || self.sorting.chars().any(char::is_control)
        {
            return Err(invalid("invalid report templates or sort key"));
        }
        Ok(())
    }
}
fn invalid(error: impl std::fmt::Display) -> Error {
    Error::InvalidReportRequest(error.to_string())
}
fn keys(values: &[String], count: usize, bytes: usize) -> Result<(), Error> {
    let mut unique = HashSet::new();
    if values.len() > count
        || values
            .iter()
            .any(|key| key.len() > bytes || !unique.insert(key))
    {
        return Err(invalid("invalid report catalog keys"));
    }
    Ok(())
}
fn decode_catalog(bytes: &[u8], session: u64) -> Result<TextReportCatalog, Error> {
    let reply: CatalogReply = serde_json::from_slice(bytes).map_err(invalid)?;
    if reply.revision == 0 {
        return Err(invalid("zero report revision"));
    }
    keys(&reply.player_variables, 1024, 256)?;
    keys(&reply.allied_variables, 1024, 256)?;
    keys(&reply.sort_columns, 256, 1024)?;
    if reply.presets.len() > 1024 {
        return Err(invalid("too many report presets"));
    }
    let mut budget = 0;
    for (index, preset) in reply.presets.iter().enumerate() {
        if preset.index as usize != index {
            return Err(invalid("report preset order mismatch"));
        }
    }
    for options in std::iter::once(&reply.default_options)
        .chain(reply.presets.iter().map(|preset| &preset.options))
    {
        options.validate()?;
        budget += options.player_format.len() + options.allies_format.len() + options.sorting.len();
        if budget > MAX_TEXT {
            return Err(invalid("report preset catalog exceeds budget"));
        }
    }
    Ok(TextReportCatalog {
        revision: TextReportRevision {
            session,
            value: reply.revision,
        },
        player_variables: reply.player_variables,
        allied_variables: reply.allied_variables,
        sort_columns: reply.sort_columns,
        default_options: reply.default_options,
        presets: reply.presets,
    })
}
fn decode_report(
    bytes: &[u8],
    encounter: EncounterId,
    revision: TextReportRevision,
) -> Result<Option<TextReport>, Error> {
    let Some(reply): Option<ReportReply> = serde_json::from_slice(bytes).map_err(invalid)? else {
        return Ok(None);
    };
    if reply.encounter_id != encounter.value
        || reply.revision != revision.value
        || reply.text.len() > MAX_TEXT
    {
        return Err(invalid(
            "report reply does not match selection or output bound",
        ));
    }
    Ok(Some(TextReport {
        encounter,
        revision,
        text: reply.text,
    }))
}
impl HostHandle {
    /// Discover registered variable/sort keys without invoking formatter callbacks.
    pub async fn text_report_catalog(&self) -> Result<TextReportCatalog, Error> {
        let output = self.report_call(1, &serde_json::json!({})).await?;
        decode_catalog(&output, self.session)
    }
    /// Render one exact selection. No automatic revision retry or callback capacity probe occurs.
    /// A removed encounter returns None; changed registries and callback failures return an error.
    pub async fn text_report(
        &self,
        encounter: EncounterId,
        revision: TextReportRevision,
        options: &TextReportOptions,
    ) -> Result<Option<TextReport>, Error> {
        if encounter.session != self.session
            || revision.session != self.session
            || encounter.value == 0
            || revision.value == 0
        {
            return Err(invalid("report selection belongs to another session"));
        }
        options.validate()?;
        let output = self.report_call(2, &serde_json::json!({"encounterId":encounter.value,"revision":revision.value,"options":options})).await?;
        decode_report(&output, encounter, revision)
    }
    async fn report_call(
        &self,
        command: u32,
        request: &serde_json::Value,
    ) -> Result<Vec<u8>, Error> {
        let input = serde_json::to_vec(request).map_err(invalid)?;
        if !(2..=256 * 1024).contains(&input.len()) {
            return Err(invalid("report request exceeds capacity"));
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
        self.request(Request::Reports, &mut packet).await?;
        decode_packet(&packet, self.session, command)
    }
}
fn decode_packet(packet: &[u8], session: u64, command: u32) -> Result<Vec<u8>, Error> {
    if packet.len() != HEADER + CAPACITY
        || packet.get(..4) != Some(((HEADER + CAPACITY) as u32).to_le_bytes().as_slice())
        || packet.get(4..8) != Some(1_u32.to_le_bytes().as_slice())
        || packet.get(8..16) != Some(session.to_le_bytes().as_slice())
        || packet.get(16..20) != Some(command.to_le_bytes().as_slice())
        || packet.get(28..32) != Some([0; 4].as_slice())
    {
        return Err(invalid("report response envelope mismatch"));
    }

    let length = u32::from_le_bytes(
        packet
            .get(24..28)
            .ok_or_else(|| invalid("missing report length"))?
            .try_into()
            .map_err(invalid)?,
    ) as usize;
    if length == 0 || length > CAPACITY {
        return Err(invalid("invalid report response length"));
    }
    Ok(packet
        .get(HEADER..HEADER + length)
        .ok_or_else(|| invalid("truncated report response"))?
        .to_vec())
}
#[cfg(test)]
mod tests;
