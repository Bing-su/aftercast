//! Session-owned, immutable combatant avoidance reports.
use crate::{EncounterId, Error, HostHandle};
use bytes::Buf;
use serde::Deserialize;

const MAX_REPORT_BYTES: usize = 64 * 1024 * 1024;

/// CLR nonfinite ratios are explicit rather than silently coerced to zero.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AvoidanceNumber {
    Finite(f64),
    Special(String),
}
impl AvoidanceNumber {
    pub(crate) fn valid(&self) -> bool {
        match self {
            Self::Finite(n) => n.is_finite(),
            Self::Special(s) => matches!(s.as_str(), "NaN" | "Infinity" | "-Infinity"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AvoidanceSummary {
    pub id: u64,
    pub is_import: bool,
    pub status: u8,
    pub zone: String,
    pub title: String,
    pub character: String,
    pub duration_ticks: i64,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AvoidanceAction {
    pub time: String,
    pub time_sorter: i32,
    pub attacker: String,
    pub victim: String,
    pub attack: String,
    pub damage: String,
    pub source_index: i32,
    pub amount: i64,
    pub swing_type: i32,
    pub critical: bool,
    pub special: Option<String>,
    pub damage_type: Option<String>,
    pub raw_label: Option<String>,
    pub raw_suffix: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AvoidanceLog {
    pub time: String,
    pub time_sorter: i32,
    #[serde(rename = "Type")]
    pub log_type: i32,
    pub text: String,
    pub source_index: i32,
}
#[derive(Debug, Clone, Deserialize)]
pub struct AvoidanceEvidence {
    pub action: AvoidanceAction,
    #[serde(rename = "Estimate")]
    pub estimate: AvoidanceNumber,
    #[serde(rename = "UsedFallback")]
    pub used_fallback: bool,
    #[serde(rename = "HasSampleAverage")]
    pub has_sample_average: bool,
    #[serde(rename = "SamplePool")]
    pub sample_pool: String,
    #[serde(rename = "PoolCount")]
    pub pool_count: u32,
    pub samples: Vec<AvoidanceAction>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvoidanceOutcome {
    #[serde(rename = "Label")]
    pub label: String,
    #[serde(rename = "Amount")]
    pub amount: i64,
    pub count: u32,
    pub percent_non_hits: Option<AvoidanceNumber>,
    pub percent_swings: Option<AvoidanceNumber>,
    pub estimated_damage: AvoidanceNumber,
    pub estimated_rate: AvoidanceNumber,
    pub evidence: Vec<AvoidanceEvidence>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvoidanceCategory {
    pub name: String,
    pub swings: Option<i32>,
    pub hits: Option<i32>,
    pub damage: i64,
    pub total_occurrences: i32,
    pub percent_swings: Option<AvoidanceNumber>,
    pub estimated_damage: AvoidanceNumber,
    pub estimated_rate: AvoidanceNumber,
    pub outcomes: Vec<AvoidanceOutcome>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvoidanceReport {
    /// Worker-local capture generation; the report itself owns all data.
    #[serde(skip)]
    pub revision: u64,
    pub summary: AvoidanceSummary,
    pub scope: String,
    pub combatant: Option<String>,
    pub dropped_logs: u64,
    pub categories: Vec<AvoidanceCategory>,
    pub logs: Vec<AvoidanceLog>,
}

fn invalid(message: &str) -> Error {
    Error::InvalidCombatResponse(message.into())
}
impl AvoidanceReport {
    fn validate(&self, id: EncounterId) -> Result<(), Error> {
        let encounter_scope = match self.scope.as_str() {
            "encounter" if self.combatant.is_none() => true,
            "combatant" if self.combatant.is_some() => false,
            _ => return Err(invalid("invalid avoidance scope")),
        };
        if self.summary.id != id.value
            || self.summary.status > 3
            || self.categories.len() > 1024
            || self.logs.len() > 65536
        {
            return Err(invalid("invalid avoidance report identity or bounds"));
        }
        for (i, log) in self.logs.iter().enumerate() {
            if log.source_index < -2
                || (log.source_index >= 0
                    && u32::try_from(log.source_index).ok() != u32::try_from(i).ok())
            {
                return Err(invalid("invalid avoidance log identity"));
            }
        }
        let valid_action = |action: &AvoidanceAction| -> bool {
            match action.source_index {
                -2 | -1 => true,
                n if n >= 0 => usize::try_from(n)
                    .ok()
                    .and_then(|i| self.logs.get(i))
                    .is_some_and(|log| {
                        log.source_index == n && log.time_sorter == action.time_sorter
                    }),
                _ => false,
            }
        };
        for category in &self.categories {
            if category.swings.is_some_and(|n| n < 0)
                || category.hits.is_some_and(|n| n < 0)
                || category.swings.is_none() != encounter_scope
                || category.hits.is_none() != encounter_scope
                || category.percent_swings.is_none() != encounter_scope
                || category.total_occurrences < 0
                || category.percent_swings.as_ref().is_some_and(|n| !n.valid())
                || !category.estimated_damage.valid()
                || !category.estimated_rate.valid()
            {
                return Err(invalid("invalid avoidance category"));
            }
        }
        let outcomes = self
            .categories
            .iter()
            .flat_map(|category| &category.outcomes);
        let mut evidence_count = 0usize;
        for outcome in outcomes.clone() {
            if usize::try_from(outcome.count).ok() != Some(outcome.evidence.len())
                || outcome.percent_non_hits.is_none() != encounter_scope
                || outcome.percent_swings.is_none() != encounter_scope
                || outcome
                    .percent_non_hits
                    .as_ref()
                    .is_some_and(|n| !n.valid())
                || outcome.percent_swings.as_ref().is_some_and(|n| !n.valid())
                || !outcome.estimated_damage.valid()
                || !outcome.estimated_rate.valid()
            {
                return Err(invalid("invalid avoidance outcome"));
            }
            evidence_count = evidence_count
                .checked_add(outcome.evidence.len())
                .ok_or_else(|| invalid("avoidance evidence overflow"))?;
        }
        let mut samples = 0usize;
        for entry in outcomes.flat_map(|outcome| &outcome.evidence) {
            samples = samples
                .checked_add(entry.samples.len())
                .ok_or_else(|| invalid("avoidance sample overflow"))?;
            if !entry.estimate.valid()
                || entry.pool_count > 16384
                || entry.samples.len() > entry.pool_count as usize
                || !valid_action(&entry.action)
                || entry.samples.iter().any(|row| !valid_action(row))
            {
                return Err(invalid("invalid avoidance evidence"));
            }
        }
        if samples > 16384 || evidence_count > 16384 {
            return Err(invalid("avoidance evidence limit"));
        }
        Ok(())
    }
}
impl HostHandle {
    /// Capture and read one frozen report; a competing capture invalidates in-flight pages.
    pub async fn combatant_avoidance(
        &self,
        id: EncounterId,
        combatant: &str,
    ) -> Result<Option<AvoidanceReport>, Error> {
        if combatant.len() > 4096 {
            return Err(Error::InvalidCombatRequest(
                "combatant name exceeds 4096 bytes".into(),
            ));
        }
        self.read_avoidance(id, 11, Some(combatant)).await
    }

    /// Capture the ACT encounter alias report under one worker-owned revision.
    pub async fn encounter_avoidance(
        &self,
        id: EncounterId,
    ) -> Result<Option<AvoidanceReport>, Error> {
        self.read_avoidance(id, 13, None).await
    }

    async fn read_avoidance(
        &self,
        id: EncounterId,
        mode: u32,
        combatant: Option<&str>,
    ) -> Result<Option<AvoidanceReport>, Error> {
        let mut header = self
            .query_combat_input(mode, Some(id), 0, 1, combatant)
            .await?;
        if header.len() < 4 {
            return Err(invalid("truncated avoidance capture"));
        }
        match header.get_u32_le() {
            0 if header.is_empty() => return Ok(None),
            1 if header.len() == 12 => {}
            _ => return Err(invalid("invalid avoidance capture")),
        }
        let revision = header.get_u64_le();
        let total = header.get_u32_le() as usize;
        if revision == 0 || total == 0 || total > MAX_REPORT_BYTES {
            return Err(invalid("invalid avoidance snapshot size"));
        }
        let snapshot = EncounterId {
            session: self.session,
            value: revision,
        };
        let mut output = Vec::with_capacity(total);
        while output.len() < total {
            let limit = (total - output.len()).min(65_536) as u32;
            let mut chunk = self
                .query_combat_input(12, Some(snapshot), output.len() as u32, limit, None)
                .await?;
            if chunk.len() < 8 {
                return Err(invalid("truncated avoidance page"));
            }
            let reported = chunk.get_u32_le() as usize;
            let count = chunk.get_u32_le() as usize;
            if reported != total || count != limit as usize || chunk.len() != count {
                return Err(invalid("invalid avoidance page"));
            }
            output.extend_from_slice(&chunk);
        }
        let mut report: AvoidanceReport =
            serde_json::from_slice(&output).map_err(|_error| invalid("invalid avoidance JSON"))?;
        report.revision = revision;
        report.validate(id)?;
        if (mode == 13) != (report.scope == "encounter") {
            return Err(invalid("avoidance scope mismatch"));
        }
        Ok(Some(report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> AvoidanceReport {
        serde_json::from_str(r#"{"summary":{"Id":7,"IsImport":true,"Status":1,"Zone":"z","Title":"t","Character":"c","DurationTicks":1},"scope":"combatant","combatant":"T","droppedLogs":0,"categories":[{"name":"D","swings":1,"hits":0,"damage":0,"totalOccurrences":1,"percentSwings":100,"estimatedDamage":0,"estimatedRate":"NaN","outcomes":[{"Label":"Miss","Amount":-1,"count":1,"percentNonHits":100,"percentSwings":100,"estimatedDamage":0,"estimatedRate":"NaN","evidence":[{"action":{"Time":"2026-01-01","TimeSorter":4,"Attacker":"S","Victim":"T","Attack":"A","Damage":"Miss","SourceIndex":0,"Amount":-1,"SwingType":1,"Critical":false},"Estimate":0,"UsedFallback":true,"HasSampleAverage":false,"SamplePool":"A","PoolCount":0,"samples":[]}]}]}],"logs":[{"Time":"2026-01-01","TimeSorter":4,"Type":0,"Text":"line","SourceIndex":0}]}"#).expect("fixture")
    }
    #[test]
    fn validates_snapshot_identity_and_nonfinite_values() {
        let id = EncounterId {
            session: 1,
            value: 7,
        };
        let report = sample();
        report.validate(id).expect("valid");
        let mut wrong_id = report.clone();
        wrong_id.summary.id = 8;
        wrong_id.validate(id).expect_err("wrong encounter");
        let mut wrong_source = report.clone();
        wrong_source.logs[0].time_sorter = 5;
        wrong_source.validate(id).expect_err("wrong log identity");
        let mut bad_number = report.clone();
        bad_number.categories[0].estimated_rate = AvoidanceNumber::Special("infinite".into());
        bad_number
            .validate(id)
            .expect_err("invalid nonfinite marker");
        let mut bad_count = report;
        bad_count.categories[0].outcomes[0].count = 2;
        bad_count.validate(id).expect_err("wrong evidence count");
    }
}
