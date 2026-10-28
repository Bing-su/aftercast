//! ACT death-window calculations over a frozen incoming action snapshot.
use crate::ClassifiedAction;

const SECOND: u64 = 10_000_000;
const MAX_TICKS: u64 = 3_155_378_975_999_999_999;

#[derive(Debug, Clone, Default)]
pub struct DeathBucket {
    pub damage: i64,
    pub healing: i64,
    /// Indices into the sorted incoming actions, once per matching registration.
    pub evidence: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct DeathReport {
    pub death_action: usize,
    pub anchor_ticks: u64,
    pub buckets: [DeathBucket; 10],
}

impl DeathReport {
    /// ACT text totals traverse T-0 to T-9; graph net traverses T-9 to T-0.
    pub fn cumulative_damage(&self) -> [i64; 10] {
        let mut value = 0_i64;
        std::array::from_fn(|i| {
            value = value.wrapping_add(self.buckets.get(i).map_or(0, |b| b.damage));
            value
        })
    }
    pub fn chronological_net(&self) -> [i64; 10] {
        let mut value = 0_i64;
        std::array::from_fn(|i| {
            let net = self
                .buckets
                .get(9 - i)
                .map_or(0, |b| b.damage.wrapping_sub(b.healing));
            value = value.wrapping_add(net);
            value
        })
    }
}

/// Sort using ACT's comparer; exact ties have no guaranteed ordering in ACT.
pub fn sort_incoming_actions(actions: &mut [ClassifiedAction]) {
    actions.sort_unstable_by_key(|a| (a.action.time_sorter, a.action.time.ticks));
}

fn add_dnum(acc: i64, amount: i64) -> i64 {
    let acc = if acc < -10 { -9 } else { acc };
    match (acc >= 0, amount >= 0) {
        (true, true) => acc.wrapping_add(amount),
        (true, false) => acc,
        (false, true) => amount,
        (false, false) => 0,
    }
}

/// Build the ten buckets for one death action. Returns None for a non-death or an
/// ACT DateTime boundary where the original AddSeconds would fail.
pub fn death_report(actions: &[ClassifiedAction], death_action: usize) -> Option<DeathReport> {
    let death = actions.get(death_action)?;
    if !death.is_death {
        return None;
    }
    let anchor = death.action.time.ticks / SECOND * SECOND;
    anchor.checked_sub(9 * SECOND)?;
    if anchor.checked_add(SECOND)? > MAX_TICKS {
        return None;
    }
    let mut buckets: [DeathBucket; 10] = std::array::from_fn(|_| DeathBucket::default());
    for (index, row) in actions.iter().enumerate() {
        let t = row.action.time.ticks;
        if t < anchor - 9 * SECOND || t >= anchor + SECOND {
            continue;
        }
        let bucket = buckets.get_mut(((anchor + SECOND - 1 - t) / SECOND) as usize)?;
        if row.registered_damage {
            bucket.damage = add_dnum(bucket.damage, row.action.amount);
            bucket.evidence.push(index);
        }
        if row.registered_healing {
            bucket.healing = add_dnum(bucket.healing, row.action.amount);
            bucket.evidence.push(index);
        }
    }
    Some(DeathReport {
        death_action,
        anchor_ticks: anchor,
        buckets,
    })
}

#[cfg(test)]
mod tests {
    use super::add_dnum;
    #[test]
    fn dnum_special_and_overflow() {
        assert_eq!(add_dnum(100, -10), 100);
        assert_eq!(add_dnum(i64::MAX, 1), i64::MIN);
        assert_eq!(add_dnum(i64::MIN, 5), 5);
        assert_eq!(add_dnum(-9, -1), 0);
    }
    #[test]
    fn matches_original_fixture() {
        use crate::{ClassifiedAction, EncounterAction, LogTimeKind, LogTimestamp, SourceLogLink};
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../managed/tests/fixtures/deaths/buckets.json"
        ))
        .unwrap();
        let base = 638_712_864_000_000_000_u64;
        for scenario in fixture["scenarios"].as_array().unwrap() {
            let make = |ticks: u64,
                        sorter: i32,
                        amount: i64,
                        damage: bool,
                        healing: bool,
                        is_death: bool| ClassifiedAction {
                action: EncounterAction {
                    time: LogTimestamp {
                        ticks,
                        utc_ticks: None,
                        kind: LogTimeKind::Unspecified,
                    },
                    time_sorter: sorter,
                    attacker: "Source".into(),
                    victim: "Target".into(),
                    attack: "Strike".into(),
                    damage: String::new(),
                    source: SourceLogLink::Missing,
                    amount,
                    swing_type: 1,
                    critical: false,
                    special: None,
                    damage_type: None,
                    raw_label: None,
                    raw_suffix: None,
                },
                registered_damage: damage,
                registered_healing: healing,
                is_death,
                numeric_tooltip: String::new(),
            };
            let mut actions = scenario["actions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    let kind = v["swingType"].as_i64().unwrap();
                    make(
                        u64::try_from(base as i64 + v["offsetTicks"].as_i64().unwrap()).unwrap(),
                        v["sorter"].as_i64().unwrap() as i32,
                        v["amount"].as_str().unwrap().parse().unwrap(),
                        kind == 1 || kind == 3,
                        kind == 2 || kind == 3,
                        false,
                    )
                })
                .collect::<Vec<_>>();
            actions.push(make(base + 7_500_000, i32::MAX, -10, false, false, true));
            super::sort_incoming_actions(&mut actions);
            let report = super::death_report(&actions, actions.len() - 1).unwrap();
            let cumulative = report.cumulative_damage();
            let net = report.chronological_net();
            assert_eq!(
                cumulative[9],
                net[9].wrapping_add(
                    report
                        .buckets
                        .iter()
                        .fold(0_i64, |sum, bucket| sum.wrapping_add(bucket.healing))
                )
            );
            for expected in scenario["expected"].as_array().unwrap() {
                let bucket = &report.buckets[expected["index"].as_u64().unwrap() as usize];
                assert_eq!(
                    bucket.damage,
                    expected["damage"].as_str().unwrap().parse::<i64>().unwrap()
                );
                assert_eq!(
                    bucket.healing,
                    expected["healing"]
                        .as_str()
                        .unwrap()
                        .parse::<i64>()
                        .unwrap()
                );
                let sorters: Vec<_> = bucket
                    .evidence
                    .iter()
                    .map(|i| actions[*i].action.time_sorter)
                    .collect();
                let expected_sorters: Vec<_> = expected["tooltipSorters"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap() as i32)
                    .collect();
                assert_eq!(sorters, expected_sorters);
            }
        }
    }
}

/// A complete, session-owned incoming snapshot for a combatant's death analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeathAnalysis {
    pub snapshot: crate::ActionSnapshot,
    pub actions: Vec<ClassifiedAction>,
    pub deaths: Vec<usize>,
    pub dropped_logs: u64,
    pub retained_logs: u32,
}

impl crate::HostHandle {
    /// Captures the selected combatant and reads its bounded pages. Concurrent
    /// captures invalidate this operation; the error is returned to the caller.
    pub async fn death_analysis(
        &self,
        encounter: crate::EncounterId,
        combatant: &str,
    ) -> Result<Option<DeathAnalysis>, crate::Error> {
        let Some(snapshot) = self
            .capture_incoming_action_snapshot(encounter, combatant)
            .await?
        else {
            return Ok(None);
        };
        let mut actions = Vec::with_capacity(snapshot.total as usize);
        let mut offset = 0;
        while offset < snapshot.total {
            let page = self.action_snapshot_page(snapshot, offset, 256).await?;
            offset += page.items.len() as u32;
            actions.extend(page.items);
        }
        let logs = self.action_snapshot_logs(snapshot, 0, 1).await?;
        sort_incoming_actions(&mut actions);
        let deaths = actions
            .iter()
            .enumerate()
            .filter_map(|(i, action)| action.is_death.then_some(i))
            .collect();
        Ok(Some(DeathAnalysis {
            snapshot,
            actions,
            deaths,
            dropped_logs: logs.dropped,
            retained_logs: logs.page.total,
        }))
    }
}
