//! Frozen encounter data and pure ACT VCR frame/playback calculations.
use crate::{AvoidanceAction, AvoidanceLog, AvoidanceSummary, EncounterId, Error, HostHandle};
use bytes::Buf;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

const SECOND: u64 = 10_000_000;
const MAX_TICKS: u64 = 3_155_378_975_999_999_999;
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VcrAction {
    pub ticks: u64,
    pub action: AvoidanceAction,
    pub is_death: bool,
}
#[derive(Debug, Clone, Deserialize)]
pub struct VcrCategory {
    pub name: String,
    pub actions: Vec<VcrAction>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VcrActor {
    #[serde(rename = "Name")]
    pub name: String,
    pub allied: bool,
    #[serde(rename = "type")]
    pub actor_type: u8,
    pub damage_taken: i64,
    pub heals_taken: i64,
    pub incoming_present: bool,
    pub incoming: Vec<VcrAction>,
    pub outgoing: Vec<VcrAction>,
    pub categories: Vec<VcrCategory>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VcrSnapshot {
    #[serde(skip)]
    pub revision: u64,
    pub summary: AvoidanceSummary,
    pub start_ticks: u64,
    pub duration: u32,
    pub damage_types: Vec<i32>,
    pub healing_types: Vec<i32>,
    pub categories: Vec<String>,
    pub dropped_logs: u64,
    pub actors: Vec<VcrActor>,
    pub logs: Vec<AvoidanceLog>,
}
fn invalid(message: &str) -> Error {
    Error::InvalidCombatResponse(message.into())
}
fn valid_source(action: &VcrAction, logs: &[AvoidanceLog]) -> bool {
    if action.ticks > MAX_TICKS {
        return false;
    }
    match action.action.source_index {
        -2 | -1 => true,
        n if n >= 0 => usize::try_from(n)
            .ok()
            .and_then(|i| logs.get(i))
            .is_some_and(|log| {
                log.source_index == n && log.time_sorter == action.action.time_sorter
            }),
        _ => false,
    }
}
impl VcrSnapshot {
    fn validate(&self, id: EncounterId) -> Result<(), Error> {
        if self.summary.id != id.value
            || self.summary.status > 3
            || self.start_ticks > MAX_TICKS
            || self.duration > 360000
            || self.actors.len() > 512
            || self.categories.len() > 1024
            || self.damage_types.len() > 4096
            || self.healing_types.len() > 4096
            || self.logs.len() > 65536
        {
            return Err(invalid("invalid VCR identity or bounds"));
        }
        if self
            .start_ticks
            .checked_add(u64::from(self.duration) * SECOND)
            .is_none_or(|time| time > MAX_TICKS)
        {
            return Err(invalid("invalid VCR time range"));
        }
        for (index, log) in self.logs.iter().enumerate() {
            if log.source_index < -2
                || (log.source_index >= 0 && usize::try_from(log.source_index).ok() != Some(index))
            {
                return Err(invalid("invalid VCR log identity"));
            }
        }
        let catalog: HashSet<_> = self.categories.iter().collect();
        if catalog.len() != self.categories.len() {
            return Err(invalid("duplicate VCR categories"));
        }
        let mut names = HashSet::new();
        let mut total = 0usize;
        for actor in &self.actors {
            if !names.insert(&actor.name)
                || actor.actor_type > 4
                || (actor.actor_type == 0) == actor.allied
            {
                return Err(invalid("invalid VCR actor"));
            }
            total = validate_actor(actor, &catalog, &self.logs, total)?;
        }
        Ok(())
    }
}
fn validate_actor(
    actor: &VcrActor,
    catalog: &HashSet<&String>,
    logs: &[AvoidanceLog],
    mut total: usize,
) -> Result<usize, Error> {
    let mut keys = HashSet::new();
    for group in &actor.categories {
        if !catalog.contains(&group.name) || !keys.insert(&group.name) {
            return Err(invalid("invalid VCR actor category"));
        }
    }
    for action in actor
        .incoming
        .iter()
        .chain(&actor.outgoing)
        .chain(actor.categories.iter().flat_map(|group| &group.actions))
    {
        total = total
            .checked_add(1)
            .ok_or_else(|| invalid("VCR action count overflow"))?;
        if total > 65536 || !valid_source(action, logs) {
            return Err(invalid("invalid VCR action"));
        }
    }
    Ok(total)
}
impl HostHandle {
    /// Copy a retained encounter; never reparses input or emits combat events.
    pub async fn vcr_snapshot(&self, id: EncounterId) -> Result<Option<VcrSnapshot>, Error> {
        let mut header = self.query_combat_input(20, Some(id), 0, 1, None).await?;
        if header.len() < 4 {
            return Err(invalid("truncated VCR capture"));
        }
        match header.get_u32_le() {
            0 if header.is_empty() => return Ok(None),
            1 if header.len() == 12 => {}
            _ => return Err(invalid("invalid VCR capture")),
        }
        let revision = header.get_u64_le();
        let total = header.get_u32_le() as usize;
        if revision == 0 || total == 0 || total > 64 * 1024 * 1024 {
            return Err(invalid("invalid VCR snapshot size"));
        }
        let snapshot = EncounterId {
            session: self.session,
            value: revision,
        };
        let mut output = Vec::with_capacity(total);
        while output.len() < total {
            let limit = (total - output.len()).min(65536) as u32;
            let mut chunk = self
                .query_combat_input(21, Some(snapshot), output.len() as u32, limit, None)
                .await?;
            if chunk.len() < 8 {
                return Err(invalid("truncated VCR page"));
            }
            let reported = chunk.get_u32_le() as usize;
            let count = chunk.get_u32_le() as usize;
            if reported != total || count != limit as usize || chunk.len() != count {
                return Err(invalid("invalid VCR page"));
            }
            output.extend_from_slice(&chunk);
        }
        let mut result: VcrSnapshot =
            serde_json::from_slice(&output).map_err(|_error| invalid("invalid VCR JSON"))?;
        result.revision = revision;
        result.validate(id)?;
        Ok(Some(result))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcrMode {
    Paused,
    Play,
    Rewind,
    FastForward,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VcrClock {
    pub position: u32,
    pub duration: u32,
    pub mode: VcrMode,
    pub linked: bool,
    ticks: u64,
}
impl VcrClock {
    pub fn new(duration: u32) -> Self {
        Self {
            position: 0,
            duration,
            mode: VcrMode::Paused,
            linked: false,
            ticks: 0,
        }
    }
    pub fn seek(&mut self, position: u32) {
        self.position = position.min(self.duration);
    }
    /// One 250 ms native timer tick. Returns true on a linked refresh tick.
    pub fn tick(&mut self) -> bool {
        self.ticks = self.ticks.wrapping_add(1);
        match self.mode {
            VcrMode::Paused => self.linked = false,
            VcrMode::Rewind if self.position > 0 => {
                self.position -= 1;
                self.linked = false;
            }
            VcrMode::FastForward if self.position < self.duration => {
                self.position += 1;
                self.linked = false;
            }
            VcrMode::Rewind | VcrMode::FastForward => {}
            VcrMode::Play if self.ticks.is_multiple_of(4) => {
                if self.linked {
                    return true;
                }
                self.position = self.position.saturating_add(1).min(self.duration);
            }
            VcrMode::Play => {}
        }
        false
    }
}
#[derive(Debug, Clone)]
pub struct VcrNode {
    pub name: String,
    pub allied: bool,
    pub actor_type: u8,
    pub active: bool,
    pub instances: usize,
    pub alive: i64,
    pub max_health: i64,
    pub health: f32,
    pub damage_taken: f32,
    pub healing_taken: f32,
}
#[derive(Debug, Clone)]
pub struct VcrEdge {
    pub from: String,
    pub to: String,
    pub category: String,
    pub amount: i64,
}
#[derive(Debug, Clone)]
pub struct VcrFrame {
    pub nodes: Vec<VcrNode>,
    pub edges: Vec<VcrEdge>,
}
fn add_dnum(left: i64, right: i64) -> i64 {
    let left = if left < -10 { -9 } else { left };
    match (left >= 0, right >= 0) {
        (true, true) => left.wrapping_add(right),
        (false, true) => right,
        (true, false) => left,
        (false, false) => 0,
    }
}
fn recent(action: &VcrAction, time: u64, lookback: u32) -> bool {
    action.ticks <= time && time - action.ticks <= u64::from(lookback) * SECOND
}
fn active(actor: &VcrActor, time: u64) -> bool {
    actor.outgoing.iter().any(|action| recent(action, time, 15))
}
fn actor_node(
    actor: &VcrActor,
    damage: &[i32],
    healing: &[i32],
    time: u64,
    sorted_health: bool,
) -> Result<VcrNode, Error> {
    let mut sorted: Vec<_> = actor.incoming.iter().collect();
    sorted.sort_unstable_by_key(|row| (row.action.time_sorter, row.ticks));
    let mut maximum = 0i64;
    let mut health = 0i64;
    for row in &sorted {
        let amount = row.action.amount;
        if amount > 0 && damage.contains(&row.action.swing_type) {
            health = health.wrapping_sub(amount);
        }
        if amount > 0 && healing.contains(&row.action.swing_type) {
            health = add_dnum(health, amount);
        }
        health = health.min(0);
        maximum = maximum.min(health);
    }
    let maximum = if actor.incoming_present {
        maximum
            .checked_abs()
            .ok_or_else(|| invalid("VCR maximum health overflow"))?
    } else {
        actor.damage_taken.wrapping_sub(actor.heals_taken)
    };
    health = maximum;
    let mut damage_sum = 0i64;
    let mut heal_sum = 0i64;
    let ordered: Vec<_> = if sorted_health {
        sorted.clone()
    } else {
        actor.incoming.iter().collect()
    };
    for row in ordered {
        let amount = row.action.amount;
        if row.ticks <= time && amount > 0 {
            if damage.contains(&row.action.swing_type) {
                health = health.wrapping_sub(amount);
            }
            if healing.contains(&row.action.swing_type) {
                health = add_dnum(health, amount);
            }
        }
        health = health.min(maximum);
    }
    // ACT's Instances property sorts incoming actions before these ratios are read.
    for row in &sorted {
        let amount = row.action.amount;
        if row.ticks <= time && amount > 0 {
            for _ in damage.iter().filter(|&&kind| kind == row.action.swing_type) {
                damage_sum = add_dnum(damage_sum, amount);
            }
            for _ in healing
                .iter()
                .filter(|&&kind| kind == row.action.swing_type)
            {
                heal_sum = add_dnum(heal_sum, amount);
            }
        }
    }
    let mut deaths = 0usize;
    let mut last = 0usize;
    for (index, row) in sorted.iter().enumerate() {
        if row.is_death {
            deaths += 1;
            last = index;
        }
    }
    let instances = (deaths + usize::from(last < sorted.len().saturating_sub(1))).max(1);
    let past_deaths = sorted
        .iter()
        .filter(|row| row.is_death && row.ticks <= time)
        .count();
    let ratio = |value: i64, total: i64| {
        if total == 0 || value == 0 {
            0.0
        } else {
            (value as f32) / (total as f32)
        }
    };
    Ok(VcrNode {
        name: actor.name.clone(),
        allied: actor.allied,
        actor_type: actor.actor_type,
        active: active(actor, time),
        instances,
        alive: instances as i64 - past_deaths as i64,
        max_health: maximum,
        health: ratio(health, maximum),
        damage_taken: ratio(damage_sum, actor.damage_taken),
        healing_taken: ratio(heal_sum, actor.heals_taken),
    })
}
/// Build a bounded frame from copied data. The graph never executes parser/plugin callbacks.
pub fn vcr_frame(
    snapshot: &VcrSnapshot,
    position: u32,
    lookback: u32,
    selected: &[String],
    persistent: bool,
    sorted_health: bool,
) -> Result<VcrFrame, Error> {
    if position > snapshot.duration || lookback > 30 || selected.len() > 1024 {
        return Err(invalid("invalid VCR frame request"));
    }
    let selection: HashSet<_> = selected.iter().collect();
    if selection.len() != selected.len()
        || selected
            .iter()
            .any(|name| !snapshot.categories.contains(name))
    {
        return Err(invalid("invalid VCR category selection"));
    }
    let time = snapshot
        .start_ticks
        .checked_add(u64::from(position) * SECOND)
        .filter(|&value| value <= MAX_TICKS)
        .ok_or_else(|| invalid("invalid VCR frame time"))?;
    let mut visible: Vec<usize> = (0..snapshot.actors.len()).collect();
    let mut work = 0usize;
    if !persistent {
        for index in (0..visible.len()).rev() {
            let Some(actor) = visible
                .get(index)
                .and_then(|&actor_index| snapshot.actors.get(actor_index))
            else {
                return Err(invalid("invalid VCR actor index"));
            };
            work = work.saturating_add(actor.outgoing.len());
            if work > 4_000_000 {
                return Err(invalid("VCR frame work limit exceeded"));
            }
            if actor.allied || active(actor, time) {
                continue;
            }
            work = visible
                .iter()
                .filter_map(|&i| snapshot.actors.get(i))
                .fold(work, |sum, other| sum.saturating_add(other.outgoing.len()));
            if work > 4_000_000 {
                return Err(invalid("VCR frame work limit exceeded"));
            }
            let targeted = visible
                .iter()
                .filter_map(|&i| snapshot.actors.get(i))
                .any(|other| {
                    other
                        .outgoing
                        .iter()
                        .any(|row| recent(row, time, 15) && row.action.victim == actor.name)
                });
            if !targeted {
                visible.remove(index);
            }
        }
    }
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for index in visible {
        let actor = snapshot
            .actors
            .get(index)
            .ok_or_else(|| invalid("invalid VCR actor"))?;
        work = work.saturating_add(
            actor.incoming.len().saturating_mul(
                snapshot
                    .damage_types
                    .len()
                    .saturating_add(snapshot.healing_types.len())
                    .max(1),
            ),
        );
        if work > 4_000_000 {
            return Err(invalid("VCR frame work limit exceeded"));
        }
        nodes.push(actor_node(
            actor,
            &snapshot.damage_types,
            &snapshot.healing_types,
            time,
            sorted_health,
        )?);
        let mut targets: Vec<&str> = Vec::new();
        for row in &actor.outgoing {
            work = work.saturating_add(targets.len().saturating_add(1));
            if work > 4_000_000 {
                return Err(invalid("VCR frame work limit exceeded"));
            }
            if recent(row, time, lookback) && !targets.contains(&row.action.victim.as_str()) {
                targets.push(row.action.victim.as_str());
            }
        }
        append_edges(
            actor, &selection, &targets, time, lookback, &mut work, &mut edges,
        )?;
    }
    Ok(VcrFrame { nodes, edges })
}

fn append_edges(
    actor: &VcrActor,
    selection: &HashSet<&String>,
    targets: &[&str],
    time: u64,
    lookback: u32,
    work: &mut usize,
    edges: &mut Vec<VcrEdge>,
) -> Result<(), Error> {
    for group in actor
        .categories
        .iter()
        .filter(|group| selection.contains(&group.name))
    {
        let mut sums: HashMap<&str, i64> = HashMap::new();
        for row in &group.actions {
            *work = work.saturating_add(1);
            if *work > 4_000_000 {
                return Err(invalid("VCR frame work limit exceeded"));
            }
            if row.action.amount > 0 && recent(row, time, lookback) {
                let value = sums.entry(row.action.victim.as_str()).or_default();
                *value = add_dnum(*value, row.action.amount);
            }
        }
        *work = work.saturating_add(targets.len());
        if *work > 4_000_000 {
            return Err(invalid("VCR frame work limit exceeded"));
        }
        for target in targets {
            if let Some(&amount) = sums.get(target)
                && amount > 0
            {
                edges.push(VcrEdge {
                    from: actor.name.clone(),
                    to: (*target).to_owned(),
                    category: group.name.clone(),
                    amount,
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn action(
        second: u64,
        amount: i64,
        swing_type: i32,
        sorter: i32,
        attacker: &str,
        victim: &str,
        death: bool,
    ) -> VcrAction {
        VcrAction {
            ticks: second * SECOND,
            action: AvoidanceAction {
                time: format!("{second}"),
                time_sorter: sorter,
                attacker: attacker.into(),
                victim: victim.into(),
                attack: "Strike".into(),
                damage: amount.to_string(),
                source_index: -1,
                amount,
                swing_type,
                critical: false,
                special: None,
                damage_type: None,
                raw_label: None,
                raw_suffix: None,
            },
            is_death: death,
        }
    }
    fn sample() -> VcrSnapshot {
        let rows = vec![
            action(0, 10, 1, 0, "A", "B", false),
            action(5, 20, 1, 1, "A", "B", false),
            action(6, 5, 2, 2, "A", "B", false),
            action(8, -10, 1, 3, "A", "B", true),
        ];
        VcrSnapshot {
            revision: 1,
            summary: AvoidanceSummary {
                id: 7,
                is_import: true,
                status: 1,
                zone: "z".into(),
                title: "t".into(),
                character: "A".into(),
                duration_ticks: 80_000_000,
            },
            start_ticks: 0,
            duration: 8,
            damage_types: vec![1, 1],
            healing_types: vec![2],
            categories: vec!["Damage".into(), "Healing".into()],
            dropped_logs: 0,
            actors: vec![
                VcrActor {
                    name: "A".into(),
                    allied: true,
                    actor_type: 3,
                    damage_taken: 0,
                    heals_taken: 0,
                    incoming_present: false,
                    incoming: vec![],
                    outgoing: rows.clone(),
                    categories: vec![
                        VcrCategory {
                            name: "Damage".into(),
                            actions: vec![
                                rows[0].clone(),
                                rows[1].clone(),
                                rows[1].clone(),
                                rows[3].clone(),
                            ],
                        },
                        VcrCategory {
                            name: "Healing".into(),
                            actions: vec![rows[2].clone()],
                        },
                    ],
                },
                VcrActor {
                    name: "B".into(),
                    allied: false,
                    actor_type: 0,
                    damage_taken: 30,
                    heals_taken: 5,
                    incoming_present: true,
                    incoming: rows,
                    outgoing: vec![],
                    categories: vec![],
                },
            ],
            logs: vec![],
        }
    }
    #[test]
    fn frame_preserves_inclusive_edges_damage_healing_and_death() {
        let snapshot = sample();
        let id = EncounterId {
            session: 1,
            value: 7,
        };
        snapshot.validate(id).expect("valid");
        let frame = vcr_frame(&snapshot, 6, 1, &["Damage".into()], false, true).expect("frame");
        let target = frame
            .nodes
            .iter()
            .find(|node| node.name == "B")
            .expect("target");
        assert_eq!(
            (target.max_health, target.instances, target.alive),
            (30, 1, 1)
        );
        assert_eq!(target.health, 5.0 / 30.0);
        assert_eq!((target.damage_taken, target.healing_taken), (2.0, 1.0));
        assert_eq!(frame.edges.len(), 1);
        assert_eq!(frame.edges[0].amount, 40);
        let at_end = vcr_frame(&snapshot, 8, 0, &["Damage".into()], true, true).expect("death");
        assert_eq!(
            at_end
                .nodes
                .iter()
                .find(|node| node.name == "B")
                .expect("target")
                .alive,
            0
        );
        assert!(at_end.edges.is_empty());
        vcr_frame(&snapshot, 9, 1, &[], true, true).expect_err("outside");
        vcr_frame(&snapshot, 1, 31, &[], true, true).expect_err("lookback");
    }
    #[test]
    fn equal_time_rows_remain_distinct_at_inclusive_boundary() {
        let mut snapshot = sample();
        let row = action(5, 7, 1, 42, "A", "B", false);
        let source = snapshot
            .actors
            .iter_mut()
            .find(|a| a.name == "A")
            .expect("source");
        source.outgoing.push(row.clone());
        source
            .categories
            .iter_mut()
            .find(|c| c.name == "Damage")
            .expect("category")
            .actions
            .push(row.clone());
        snapshot
            .actors
            .iter_mut()
            .find(|a| a.name == "B")
            .expect("target")
            .incoming
            .push(row);
        let frame = vcr_frame(&snapshot, 5, 0, &["Damage".into()], true, true).expect("frame");
        assert!(frame.edges.iter().any(|edge| edge.amount == 47));
        let frame = vcr_frame(&snapshot, 4, 0, &["Damage".into()], true, true).expect("before");
        assert!(frame.edges.is_empty());
    }
    #[test]
    fn clock_obeys_native_tick_rate_seek_and_link() {
        let mut clock = VcrClock::new(4);
        clock.mode = VcrMode::Play;
        for _ in 0..3 {
            assert!(!clock.tick());
            assert_eq!(clock.position, 0);
        }
        assert!(!clock.tick());
        assert_eq!(clock.position, 1);
        clock.mode = VcrMode::FastForward;
        clock.tick();
        assert_eq!(clock.position, 2);
        clock.mode = VcrMode::Rewind;
        clock.tick();
        assert_eq!(clock.position, 1);
        clock.seek(100);
        assert_eq!(
            (clock.position, clock.mode, clock.linked),
            (4, VcrMode::Rewind, false)
        );
        clock.mode = VcrMode::Play;
        clock.linked = true;
        assert!(!clock.tick());
        assert!(clock.tick());
        assert_eq!(clock.position, 4);
        clock.mode = VcrMode::Paused;
        clock.tick();
        assert!(!clock.linked);
    }
}
