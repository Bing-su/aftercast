//! Session-owned ACT encounter timeline snapshot.
use crate::{AvoidanceSummary, EncounterId, Error, HostHandle};
use bytes::Buf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TimelineOptions {
    pub grouping: u8,
    pub window: u16,
    pub rolling: bool,
    pub metrics: u8,
}
impl Default for TimelineOptions {
    fn default() -> Self {
        Self {
            grouping: 0,
            window: 10,
            rolling: true,
            metrics: 3,
        }
    }
}
impl TimelineOptions {
    fn valid(self) -> bool {
        self.grouping <= 3 && (1..=600).contains(&self.window) && self.metrics <= 15
    }
}
#[derive(Debug, Clone, Deserialize)]
pub struct TimelineSeries {
    pub metric: u8,
    #[serde(rename = "axisMaximum")]
    pub axis_maximum: i64,
    pub raw: Vec<i64>,
    pub rates: Vec<i64>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct TimelineRow {
    pub name: String,
    pub markers: Vec<u8>,
    pub series: Vec<TimelineSeries>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineLog {
    pub time: String,
    pub seconds: f64,
    #[serde(rename = "GlobalTimeSorter")]
    pub time_sorter: i32,
    #[serde(rename = "Type")]
    pub log_type: i32,
    pub text: String,
    pub source_index: i32,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineReport {
    #[serde(skip)]
    pub revision: u64,
    pub summary: AvoidanceSummary,
    pub options: TimelineOptions,
    pub duration: u32,
    pub points: u32,
    pub dropped_logs: u64,
    pub rows: Vec<TimelineRow>,
    pub logs: Vec<TimelineLog>,
}
fn invalid(message: &str) -> Error {
    Error::InvalidCombatResponse(message.into())
}
impl TimelineReport {
    fn validate(&self, id: EncounterId, expected: TimelineOptions) -> Result<(), Error> {
        if self.summary.id != id.value
            || self.summary.status > 3
            || self.options != expected
            || !self.options.valid()
            || self.duration > 360000
            || self.rows.len() > 516
            || self.logs.len() > 65536
        {
            return Err(invalid("invalid timeline identity or bounds"));
        }
        let points = if expected.rolling {
            self.duration
        } else {
            self.duration / u32::from(expected.window)
        };
        if self.points != points
            || (self.rows.len() as u64)
                * (1 + expected.metrics.count_ones() as u64)
                * u64::from(points)
                * 2
                > 2_000_000
        {
            return Err(invalid("invalid timeline point count"));
        }
        for (index, log) in self.logs.iter().enumerate() {
            if !log.seconds.is_finite()
                || log.source_index < -2
                || (log.source_index >= 0 && usize::try_from(log.source_index).ok() != Some(index))
            {
                return Err(invalid("invalid timeline log"));
            }
        }
        let mut names = std::collections::HashSet::new();
        for row in &self.rows {
            if !names.insert(&row.name) {
                return Err(invalid("duplicate timeline group"));
            }
            validate_row(row, points, expected)?;
        }
        Ok(())
    }
}
fn validate_row(row: &TimelineRow, points: u32, expected: TimelineOptions) -> Result<(), Error> {
    if row.markers.len() != points as usize || row.markers.iter().any(|&marker| marker & !7 != 0) {
        return Err(invalid("invalid timeline markers"));
    }
    let mut metrics = 0u8;
    for series in &row.series {
        if series.axis_maximum < 1 || series.raw.iter().any(|value| *value > series.axis_maximum) {
            return Err(invalid("invalid timeline maximum"));
        }
        if series.metric > 3
            || series.raw.len() != points as usize
            || series.rates.len() != points as usize
        {
            return Err(invalid("invalid timeline series"));
        }
        if series
            .raw
            .iter()
            .zip(&series.rates)
            .any(|(raw, rate)| raw / i64::from(expected.window) != *rate)
        {
            return Err(invalid("timeline rate mismatch"));
        }
        let bit = 1 << series.metric;
        if expected.metrics & bit == 0 || metrics & bit != 0 {
            return Err(invalid("invalid timeline metric"));
        }
        metrics |= bit;
    }
    if metrics != expected.metrics {
        return Err(invalid("incomplete timeline metrics"));
    }
    Ok(())
}
impl HostHandle {
    /// Capture grouped series and time-indexed logs together under the worker combat lock.
    pub async fn timeline(
        &self,
        id: EncounterId,
        options: TimelineOptions,
    ) -> Result<Option<TimelineReport>, Error> {
        if !options.valid() {
            return Err(Error::InvalidCombatRequest(
                "invalid timeline options".into(),
            ));
        }
        let input = serde_json::to_string(&options)
            .map_err(|_error| Error::InvalidCombatRequest("invalid timeline options".into()))?;
        let mut header = self
            .query_combat_input(16, Some(id), 0, 1, Some(&input))
            .await?;
        if header.len() < 4 {
            return Err(invalid("truncated timeline capture"));
        }
        match header.get_u32_le() {
            0 if header.is_empty() => return Ok(None),
            1 if header.len() == 12 => {}
            _ => return Err(invalid("invalid timeline capture")),
        }
        let revision = header.get_u64_le();
        let total = header.get_u32_le() as usize;
        if revision == 0 || total == 0 || total > 64 * 1024 * 1024 {
            return Err(invalid("invalid timeline snapshot size"));
        }
        let snapshot = EncounterId {
            session: self.session,
            value: revision,
        };
        let mut output = Vec::with_capacity(total);
        while output.len() < total {
            let limit = (total - output.len()).min(65536) as u32;
            let mut chunk = self
                .query_combat_input(17, Some(snapshot), output.len() as u32, limit, None)
                .await?;
            if chunk.len() < 8 {
                return Err(invalid("truncated timeline page"));
            }
            let reported = chunk.get_u32_le() as usize;
            let count = chunk.get_u32_le() as usize;
            if reported != total || count != limit as usize || chunk.len() != count {
                return Err(invalid("invalid timeline page"));
            }
            output.extend_from_slice(&chunk);
        }
        let mut report: TimelineReport =
            serde_json::from_slice(&output).map_err(|_error| invalid("invalid timeline JSON"))?;
        report.revision = revision;
        report.validate(id, options)?;
        Ok(Some(report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_scope_points_and_log_identity() {
        let text = r#"{"summary":{"Id":7,"IsImport":true,"Status":1,"Zone":"z","Title":"t","Character":"c","DurationTicks":1},"options":{"Grouping":0,"Window":10,"Rolling":true,"Metrics":3},"duration":2,"points":2,"droppedLogs":0,"rows":[{"name":"S","markers":[0,4],"series":[{"metric":0,"axisMaximum":20,"raw":[10,20],"rates":[1,2]},{"metric":1,"axisMaximum":1,"raw":[0,0],"rates":[0,0]}]}],"logs":[{"time":"2026-01-01","seconds":1,"GlobalTimeSorter":4,"Type":0,"text":"line","sourceIndex":0}]}"#;
        let report: TimelineReport = serde_json::from_str(text).expect("fixture");
        let id = EncounterId {
            session: 1,
            value: 7,
        };
        let options = TimelineOptions::default();
        report.validate(id, options).expect("valid");
        let mut bad = report.clone();
        bad.points = 1;
        bad.validate(id, options).expect_err("point count");
        bad = report.clone();
        bad.logs[0].source_index = 1;
        bad.validate(id, options).expect_err("log identity");
        bad = report.clone();
        bad.rows[0].series[0].rates[0] = 999;
        bad.validate(id, options).expect_err("mismatched rate");
        bad = report.clone();
        bad.rows[0].series[1].metric = 0;
        bad.validate(id, options).expect_err("duplicate metric");
        bad = report;
        bad.rows[0].markers[0] = 16;
        bad.validate(id, options).expect_err("marker");
    }
}
