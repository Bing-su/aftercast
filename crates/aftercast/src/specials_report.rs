//! Owned ACT special attack report captured at a single combat revision.
use crate::{
    AvoidanceAction, AvoidanceLog, AvoidanceNumber, AvoidanceSummary, EncounterId, Error,
    HostHandle,
};
use bytes::Buf;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecialRow {
    pub group: String,
    pub key: String,
    pub occurrences: Option<i32>,
    pub swings: i32,
    pub total_swings: Option<i32>,
    pub occurrence_ratio: Option<AvoidanceNumber>,
    pub swing_ratio: Option<AvoidanceNumber>,
    pub total_rate: Option<AvoidanceNumber>,
    pub rate: AvoidanceNumber,
    pub evidence: Vec<AvoidanceAction>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecialReport {
    #[serde(skip)]
    pub revision: u64,
    pub summary: AvoidanceSummary,
    pub combatant: String,
    requested_combatant: String,
    pub dropped_logs: u64,
    pub rows: Vec<SpecialRow>,
    pub logs: Vec<AvoidanceLog>,
}
fn invalid(message: &str) -> Error {
    Error::InvalidCombatResponse(message.into())
}
impl SpecialReport {
    fn validate(&self, id: EncounterId, combatant: &str) -> Result<(), Error> {
        if self.summary.id != id.value
            || self.summary.status > 3
            || self.requested_combatant != combatant
            || self.rows.len() > 4096
            || self.logs.len() > 65536
        {
            return Err(invalid("invalid special report identity or bounds"));
        }
        for (i, log) in self.logs.iter().enumerate() {
            if log.source_index < -2
                || (log.source_index >= 0 && usize::try_from(log.source_index).ok() != Some(i))
            {
                return Err(invalid("invalid special log identity"));
            }
        }
        let mut count = 0usize;
        for row in &self.rows {
            let special = match row.group.as_str() {
                "pair" | "skill" => false,
                "special" => true,
                _ => return Err(invalid("invalid special group")),
            };
            if row.swings < 0
                || row.occurrences.is_none() != special
                || row.total_swings.is_none() != special
                || row.occurrence_ratio.is_none() != special
                || row.swing_ratio.is_none() != special
                || row.total_rate.is_none() != special
                || !row.rate.valid()
                || row.occurrence_ratio.as_ref().is_some_and(|v| !v.valid())
                || row.swing_ratio.as_ref().is_some_and(|v| !v.valid())
                || row.total_rate.as_ref().is_some_and(|v| !v.valid())
            {
                return Err(invalid("invalid special row"));
            }
            count = count
                .checked_add(row.evidence.len())
                .ok_or_else(|| invalid("special evidence overflow"))?;
            if count > 49152 {
                return Err(invalid("special evidence limit"));
            }
            if row
                .evidence
                .iter()
                .any(|action| !valid_source(action, &self.logs))
            {
                return Err(invalid("invalid special evidence source"));
            }
        }
        Ok(())
    }
}
fn valid_source(action: &AvoidanceAction, logs: &[AvoidanceLog]) -> bool {
    match action.source_index {
        -2 | -1 => true,
        n if n >= 0 => usize::try_from(n)
            .ok()
            .and_then(|i| logs.get(i))
            .is_some_and(|log| log.source_index == n && log.time_sorter == action.time_sorter),
        _ => false,
    }
}
impl HostHandle {
    /// Capture groups and source evidence atomically; competing special captures retire in-flight pages.
    pub async fn special_report(
        &self,
        id: EncounterId,
        combatant: &str,
    ) -> Result<Option<SpecialReport>, Error> {
        if combatant.len() > 4096 {
            return Err(Error::InvalidCombatRequest(
                "combatant name exceeds 4096 bytes".into(),
            ));
        }
        let mut header = self
            .query_combat_input(14, Some(id), 0, 1, Some(combatant))
            .await?;
        if header.len() < 4 {
            return Err(invalid("truncated special capture"));
        }
        match header.get_u32_le() {
            0 if header.is_empty() => return Ok(None),
            1 if header.len() == 12 => {}
            _ => return Err(invalid("invalid special capture")),
        }
        let revision = header.get_u64_le();
        let total = header.get_u32_le() as usize;
        if revision == 0 || total == 0 || total > 64 * 1024 * 1024 {
            return Err(invalid("invalid special snapshot size"));
        }
        let snapshot = EncounterId {
            session: self.session,
            value: revision,
        };
        let mut output = Vec::with_capacity(total);
        while output.len() < total {
            let limit = (total - output.len()).min(65536) as u32;
            let mut chunk = self
                .query_combat_input(15, Some(snapshot), output.len() as u32, limit, None)
                .await?;
            if chunk.len() < 8 {
                return Err(invalid("truncated special page"));
            }
            let reported = chunk.get_u32_le() as usize;
            let count = chunk.get_u32_le() as usize;
            if reported != total || count != limit as usize || chunk.len() != count {
                return Err(invalid("invalid special page"));
            }
            output.extend_from_slice(&chunk);
        }
        let mut report: SpecialReport =
            serde_json::from_slice(&output).map_err(|_error| invalid("invalid special JSON"))?;
        report.revision = revision;
        report.validate(id, combatant)?;
        Ok(Some(report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_malformed_snapshot_and_sources() {
        let text = r#"{"summary":{"Id":7,"IsImport":true,"Status":1,"Zone":"z","Title":"t","Character":"c","DurationTicks":1},"requestedCombatant":"s","combatant":"S","droppedLogs":0,"rows":[{"group":"pair","key":"Hit(Crit)","occurrences":1,"swings":1,"totalSwings":1,"occurrenceRatio":"Infinity","swingRatio":1,"totalRate":1,"rate":1,"evidence":[{"Time":"2026-01-01","TimeSorter":4,"Attacker":"S","Victim":"T","Attack":"Hit","Damage":"1","SourceIndex":0,"Amount":1,"SwingType":1,"Critical":true}]}],"logs":[{"Time":"2026-01-01","TimeSorter":4,"Type":0,"Text":"line","SourceIndex":0}]}"#;
        let report: SpecialReport = serde_json::from_str(text).expect("fixture");
        let id = EncounterId {
            session: 1,
            value: 7,
        };
        report.validate(id, "s").expect("valid");
        report.validate(id, "other").expect_err("wrong request");
        let mut bad = report.clone();
        bad.logs[0].time_sorter = 5;
        bad.validate(id, "s").expect_err("wrong source");
        bad = report.clone();
        bad.rows[0].occurrence_ratio = Some(AvoidanceNumber::Special("invalid".into()));
        bad.validate(id, "s").expect_err("invalid nonfinite");
        bad = report;
        bad.rows[0].group = "unknown".into();
        bad.validate(id, "s").expect_err("invalid group");
    }
}
