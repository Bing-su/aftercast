//! ACT recast interval calculations over an immutable attack snapshot.
use crate::Error;
use std::collections::BTreeMap;

const SECOND: i64 = 10_000_000;
const MAX_TICKS: u64 = 3_155_378_975_999_999_999;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecastTime {
    pub ticks: u64,
    /// None means no modifier attack exists; Some records whether it applies at this start.
    pub modified: Option<bool>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecastCalculation {
    /// Every adjacent checked interval, before the histogram filter.
    pub intervals: Vec<i32>,
    pub histogram: Vec<(i32, u32)>,
    /// ACT's minimum greater than one and less than 12345; None means its zero handoff fallback.
    pub minimum: Option<i32>,
}
fn invalid(message: &str) -> Error {
    Error::InvalidCombatRequest(message.into())
}
fn seconds(start: u64, end: u64) -> Result<i32, Error> {
    if start > MAX_TICKS || end > MAX_TICKS {
        return Err(invalid("recast time outside DateTime range"));
    }
    // TimeSpan.TotalSeconds divides ticks as f64 before ACT truncates to int.
    let delta = ((end as i64 - start as i64) as f64 / SECOND as f64).trunc();
    if delta < f64::from(i32::MIN) || delta > f64::from(i32::MAX) {
        return Err(invalid("recast interval outside supported range"));
    }
    Ok(delta as i32)
}
/// ACT checks the first row and then compares each row to the preceding raw row.
pub fn recast_initial_selection(rows: &[RecastTime], restrict: bool) -> Result<Vec<bool>, Error> {
    if rows.len() > 16384 {
        return Err(invalid("recast exceeds 16384 actions"));
    }
    let mut selected = Vec::with_capacity(rows.len());
    let Some(first) = rows.first() else {
        return Ok(selected);
    };
    if first.ticks > MAX_TICKS {
        return Err(invalid("recast time outside DateTime range"));
    }
    selected.push(true);
    for pair in rows.windows(2) {
        if let [before, after] = pair {
            selected.push(seconds(before.ticks, after.ticks)? > if restrict { 11 } else { 1 });
        }
    }
    Ok(selected)
}
fn adjusted(delay: i32, modified: bool) -> Result<i32, Error> {
    let divisor = if modified { 1.5_f32 } else { 1.0_f32 };
    let value = (delay as f32) / divisor;
    if !(f64::from(i32::MIN)..f64::from(i32::MAX) + 1.0).contains(&f64::from(value)) {
        return Err(invalid("adjusted recast interval outside supported range"));
    }
    Ok(value as i32)
}
/// ACT applies the optional 50% modifier with float32 truncation to checked pairs.
pub fn recast_calculate(
    rows: &[RecastTime],
    selected: &[bool],
    restrict: bool,
    use_modifier: bool,
) -> Result<RecastCalculation, Error> {
    if rows.len() > 16384 || rows.len() != selected.len() {
        return Err(invalid("invalid recast selection"));
    }
    let mut previous: Option<RecastTime> = None;
    let mut intervals = Vec::new();
    let mut minimum = 12345;
    let mut histogram = BTreeMap::<i32, u32>::new();
    for (row, checked) in rows.iter().zip(selected) {
        if row.ticks > MAX_TICKS {
            return Err(invalid("recast time outside DateTime range"));
        }
        if !checked {
            continue;
        }
        if let Some(start) = previous {
            let mut delay = seconds(start.ticks, row.ticks)?;
            if use_modifier && let Some(modified) = start.modified {
                delay = adjusted(delay, modified)?;
            }
            intervals.push(delay);
            if delay > 1 && delay < minimum {
                minimum = delay;
            }
            if delay > 1 && (!restrict || delay < 121) {
                *histogram.entry(delay).or_default() += 1;
            }
        }
        previous = Some(*row);
    }
    Ok(RecastCalculation {
        intervals,
        histogram: histogram.into_iter().collect(),
        minimum: (minimum != 12345).then_some(minimum),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_act_interval_fixtures_and_bounds() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../managed/tests/fixtures/recast/intervals.json"
        ))
        .expect("fixture");
        for case in fixture["cases"].as_array().expect("cases") {
            let times = case["seconds"].as_array().expect("times");
            let modified = case["modified"].as_array().expect("modifiers");
            let rows: Vec<_> = times
                .iter()
                .zip(modified)
                .map(|(time, modifier)| RecastTime {
                    ticks: 621_355_968_000_000_000 + time.as_u64().expect("seconds") * 10_000_000,
                    modified: modifier.as_bool(),
                })
                .collect();
            let selected: Vec<_> = case["selected"]
                .as_array()
                .expect("selected")
                .iter()
                .map(|value| value.as_bool().expect("bool"))
                .collect();
            let initial: Vec<_> = case["initial"]
                .as_array()
                .expect("initial")
                .iter()
                .map(|value| value.as_bool().expect("bool"))
                .collect();
            let restrict = case["restrict"].as_bool().expect("restrict");
            assert_eq!(
                recast_initial_selection(&rows, restrict).expect("initial"),
                initial,
                "{}",
                case["name"]
            );
            let actual = recast_calculate(
                &rows,
                &selected,
                restrict,
                case["useModifier"].as_bool().expect("modifier"),
            )
            .expect("calculation");
            let intervals: Vec<_> = case["intervals"]
                .as_array()
                .expect("intervals")
                .iter()
                .map(|value| value.as_i64().expect("int") as i32)
                .collect();
            let histogram: Vec<_> = case["histogram"]
                .as_array()
                .expect("histogram")
                .iter()
                .map(|row| {
                    (
                        row[0].as_i64().expect("key") as i32,
                        row[1].as_u64().expect("count") as u32,
                    )
                })
                .collect();
            assert_eq!(actual.intervals, intervals, "{}", case["name"]);
            assert_eq!(actual.histogram, histogram, "{}", case["name"]);
            assert_eq!(
                actual.minimum,
                case["minimum"].as_i64().map(|value| value as i32),
                "{}",
                case["name"]
            );
        }
    }
    #[test]
    fn time_precision_and_bounds() {
        let extreme = [
            RecastTime {
                ticks: 0,
                modified: None,
            },
            RecastTime {
                ticks: 10_000_000_999_999_999,
                modified: None,
            },
        ];
        assert_eq!(
            recast_calculate(&extreme, &[true, true], false, false)
                .expect("f64 truncation")
                .intervals,
            [1_000_000_100]
        );
        let fractional = [
            RecastTime {
                ticks: 0,
                modified: None,
            },
            RecastTime {
                ticks: 19_000_000,
                modified: None,
            },
            RecastTime {
                ticks: 39_000_000,
                modified: None,
            },
        ];
        assert_eq!(
            recast_initial_selection(&fractional, false).expect("fractional"),
            [true, false, true]
        );
        let row = RecastTime {
            ticks: 0,
            modified: None,
        };
        recast_initial_selection(&vec![row; 16385], false).expect_err("action limit");
        recast_calculate(&[row], &[], false, false).expect_err("selection length");
        adjusted(i32::MAX, false).expect_err("float32 conversion overflow");
    }
}

use crate::{AvoidanceAction, AvoidanceLog, AvoidanceSummary, EncounterId, HostHandle};
use bytes::Buf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RecastRequest {
    pub combatant: String,
    pub category: String,
    pub attack: String,
    pub outgoing: bool,
    pub modifier: bool,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecastRow {
    pub ticks: u64,
    pub action: AvoidanceAction,
    pub modified: Option<bool>,
    pub modifier_evidence: Option<AvoidanceAction>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecastSnapshot {
    #[serde(skip)]
    pub revision: u64,
    pub summary: AvoidanceSummary,
    pub request: RecastRequest,
    pub combatant: String,
    pub attack_name: Option<String>,
    pub first_attacker: Option<String>,
    pub dropped_logs: u64,
    pub rows: Vec<RecastRow>,
    pub logs: Vec<AvoidanceLog>,
}
fn bad(message: &str) -> Error {
    Error::InvalidCombatResponse(message.into())
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
impl RecastSnapshot {
    fn validate(&self, id: EncounterId, request: &RecastRequest) -> Result<(), Error> {
        if self.summary.id != id.value
            || self.summary.status > 3
            || &self.request != request
            || self.rows.len() > 16384
            || self.logs.len() > 65536
            || self.first_attacker.is_none() != self.rows.is_empty()
        {
            return Err(bad("invalid recast identity or bounds"));
        }
        for (index, log) in self.logs.iter().enumerate() {
            if log.source_index < -2
                || (log.source_index >= 0 && usize::try_from(log.source_index).ok() != Some(index))
            {
                return Err(bad("invalid recast log"));
            }
        }
        let mut previous = None;
        let modifier_present = self.rows.first().is_some_and(|row| row.modified.is_some());
        for row in &self.rows {
            if row.ticks > MAX_TICKS
                || !valid_source(&row.action, &self.logs)
                || row.modifier_evidence.is_some() != (row.modified == Some(true))
                || row.modified.is_some() != modifier_present
                || (!request.modifier && row.modified.is_some())
                || row
                    .modifier_evidence
                    .as_ref()
                    .is_some_and(|value| !valid_source(value, &self.logs))
            {
                return Err(bad("invalid recast row"));
            }
            let key = (row.action.time_sorter, row.ticks);
            if previous.is_some_and(|value| value > key) {
                return Err(bad("invalid recast ordering"));
            }
            previous = Some(key);
        }
        Ok(())
    }
    pub fn times(&self) -> Vec<RecastTime> {
        self.rows
            .iter()
            .map(|row| RecastTime {
                ticks: row.ticks,
                modified: row.modified,
            })
            .collect()
    }
}
impl HostHandle {
    /// Capture one exact category/attack and its optional modifier evidence under the combat lock.
    pub async fn recast_snapshot(
        &self,
        id: EncounterId,
        request: &RecastRequest,
    ) -> Result<Option<RecastSnapshot>, Error> {
        let input =
            serde_json::to_string(request).map_err(|_error| invalid("invalid recast request"))?;
        if input.len() > 4096 {
            return Err(invalid("recast request exceeds 4096 bytes"));
        }
        let mut header = self
            .query_combat_input(18, Some(id), 0, 1, Some(&input))
            .await?;
        if header.len() < 4 {
            return Err(bad("truncated recast capture"));
        }
        match header.get_u32_le() {
            0 if header.is_empty() => return Ok(None),
            1 if header.len() == 12 => {}
            _ => return Err(bad("invalid recast capture")),
        }
        let revision = header.get_u64_le();
        let total = header.get_u32_le() as usize;
        if revision == 0 || total == 0 || total > 64 * 1024 * 1024 {
            return Err(bad("invalid recast snapshot size"));
        }
        let snapshot = EncounterId {
            session: self.session,
            value: revision,
        };
        let mut output = Vec::with_capacity(total);
        while output.len() < total {
            let limit = (total - output.len()).min(65536) as u32;
            let mut chunk = self
                .query_combat_input(19, Some(snapshot), output.len() as u32, limit, None)
                .await?;
            if chunk.len() < 8 {
                return Err(bad("truncated recast page"));
            }
            let reported = chunk.get_u32_le() as usize;
            let count = chunk.get_u32_le() as usize;
            if reported != total || count != limit as usize || chunk.len() != count {
                return Err(bad("invalid recast page"));
            }
            output.extend_from_slice(&chunk);
        }
        let mut report: RecastSnapshot =
            serde_json::from_slice(&output).map_err(|_error| bad("invalid recast JSON"))?;
        report.revision = revision;
        report.validate(id, request)?;
        Ok(Some(report))
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    #[test]
    fn validates_owned_request_and_evidence() {
        let text = r#"{"summary":{"Id":7,"IsImport":true,"Status":1,"Zone":"z","Title":"t","Character":"c","DurationTicks":1},"request":{"Combatant":"Source","Category":"Damage","Attack":"Strike","Outgoing":true,"Modifier":true},"combatant":"Source","attackName":"Strike","firstAttacker":"Source","droppedLogs":0,"rows":[{"ticks":10,"action":{"Time":"2026-01-01","TimeSorter":4,"Attacker":"Source","Victim":"T","Attack":"Strike","Damage":"1","SourceIndex":0,"Amount":1,"SwingType":1,"Critical":false},"modified":false,"modifierEvidence":null}],"logs":[{"Time":"2026-01-01","TimeSorter":4,"Type":0,"Text":"line","SourceIndex":0}]}"#;
        let report: RecastSnapshot = serde_json::from_str(text).expect("fixture");
        let id = EncounterId {
            session: 1,
            value: 7,
        };
        report.validate(id, &report.request).expect("valid");
        let mut bad_report = report.clone();
        bad_report.logs[0].time_sorter += 1;
        bad_report
            .validate(id, &report.request)
            .expect_err("wrong source");
        bad_report = report.clone();
        bad_report.rows[0].modified = Some(true);
        bad_report
            .validate(id, &report.request)
            .expect_err("missing modifier evidence");
        let mut wrong = report.request.clone();
        wrong.outgoing = false;
        report.validate(id, &wrong).expect_err("wrong request");
    }
}
