use super::*;
use bytes::Buf;
use std::collections::HashSet;
pub(crate) struct Reader<'a>(pub(crate) &'a [u8]);
impl Reader<'_> {
    pub(crate) fn take(&mut self, count: usize) -> Result<&[u8], Error> {
        let (value, rest) = self
            .0
            .split_at_checked(count)
            .ok_or_else(|| invalid("truncated table data"))?;
        self.0 = rest;
        Ok(value)
    }
    pub(crate) fn u32(&mut self) -> Result<u32, Error> {
        Ok(self.take(4)?.get_u32_le())
    }
    pub(crate) fn u64(&mut self) -> Result<u64, Error> {
        Ok(self.take(8)?.get_u64_le())
    }
    pub(crate) fn boolean(&mut self) -> Result<bool, Error> {
        match self.u32()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid("invalid table presence flag")),
        }
    }
    pub(crate) fn optional_int(&mut self) -> Result<Option<i32>, Error> {
        if self.boolean()? {
            Ok(Some(self.u32()?.cast_signed()))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn optional_text(&mut self, maximum: usize) -> Result<Option<String>, Error> {
        let count = self.u32()?;
        if count == u32::MAX {
            return Ok(None);
        }
        if count as usize > maximum {
            return Err(invalid("table text exceeds limit"));
        }
        let text = std::str::from_utf8(self.take(count as usize)?).map_err(invalid)?;
        if !valid_text(text, maximum, false) {
            return Err(invalid("invalid table text"));
        }
        Ok(Some(text.to_owned()))
    }
    pub(crate) fn text(&mut self, maximum: usize) -> Result<String, Error> {
        self.optional_text(maximum)?
            .ok_or_else(|| invalid("missing required table text"))
    }
    pub(crate) fn level(&mut self) -> Result<TableLevel, Error> {
        match self.u32()? {
            0 => Ok(TableLevel::Encounter),
            1 => Ok(TableLevel::Combatant),
            2 => Ok(TableLevel::Category),
            3 => Ok(TableLevel::Attack),
            4 => Ok(TableLevel::Action),
            _ => Err(invalid("invalid table level")),
        }
    }
    fn revision(&mut self, session: u64, expected: TableLevel) -> Result<TableRevision, Error> {
        let level = self.level()?;
        let value = self.u64()?;
        if level != expected || value == 0 {
            return Err(invalid("invalid table revision/level"));
        }
        Ok(TableRevision {
            session,
            level,
            value,
        })
    }
    pub(crate) fn columns(&mut self, maximum: u32) -> Result<Vec<TableColumn>, Error> {
        let count = self.u32()?;
        if count > maximum {
            return Err(invalid("too many table columns"));
        }
        let mut result = Vec::with_capacity(count as usize);
        let mut seen = HashSet::new();
        for _ in 0..count {
            let key = self.text(1024)?;
            if key.is_empty() || !seen.insert(key.clone()) {
                return Err(invalid("empty/duplicate column key"));
            }
            let label = self.text(1024)?;
            let sql_data_type = self.text(1024)?;
            let sql_data_name = self.text(1024)?;
            let flags = self.u32()?;
            if flags > 7 {
                return Err(invalid("invalid column flags"));
            }
            result.push(TableColumn {
                key,
                label,
                sql_data_type,
                sql_data_name,
                default_visible: flags & 1 != 0,
                can_read: flags & 2 != 0,
                can_sort: flags & 4 != 0,
            });
        }
        Ok(result)
    }
    fn cell(&mut self) -> Result<TableCell, Error> {
        let text = self.optional_text(4096)?;
        let foreground_argb = self.optional_int()?;
        let background_argb = self.optional_int()?;
        let error = match self.optional_text(64)?.as_deref() {
            None => None,
            Some("column-error") => Some(TableCellError::Column),
            _ => return Err(invalid("unknown cell error")),
        };
        if error.is_some()
            && (text.is_some() || foreground_argb.is_some() || background_argb.is_some())
            || error.is_none() && text.is_none()
        {
            return Err(invalid("inconsistent table cell status"));
        }
        Ok(TableCell {
            text,
            foreground_argb,
            background_argb,
            error,
        })
    }
    pub(crate) fn finish(&self) -> Result<(), Error> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(invalid("trailing table bytes"))
        }
    }
}
pub(super) fn catalog(
    bytes: &[u8],
    session: u64,
    level: TableLevel,
) -> Result<TableCatalog, Error> {
    let mut reader = Reader(bytes);
    let revision = reader.revision(session, level)?;
    let columns = reader.columns(256)?;
    let count = reader.u32()?;
    if count > 512 {
        return Err(invalid("too many table categories"));
    }
    let mut categories = Vec::with_capacity(count as usize);
    let mut seen = HashSet::new();
    let mut outgoing_count = 0;
    for _ in 0..count {
        let key = reader.text(1024)?;
        let label = reader.text(1024)?;
        let outgoing = reader.boolean()?;
        if key.is_empty() || !seen.insert((key.clone(), outgoing)) {
            return Err(invalid("duplicate/empty category key"));
        }
        outgoing_count += u32::from(outgoing);
        categories.push(TableCategory {
            key,
            label,
            outgoing,
            ally_value: reader.u32()?.cast_signed(),
            color_argb: reader.u32()?.cast_signed(),
        });
    }
    if outgoing_count > 256 || count - outgoing_count > 256 {
        return Err(invalid("category direction exceeds limit"));
    }
    reader.finish()?;
    Ok(TableCatalog {
        level,
        revision,
        columns,
        categories,
    })
}
pub(super) fn page(
    bytes: &[u8],
    session: u64,
    query: &TableQuery,
) -> Result<Option<TablePage>, Error> {
    let mut reader = Reader(bytes);
    if !reader.boolean()? {
        reader.finish()?;
        return Ok(None);
    }
    let revision = reader.revision(session, query.scope.level())?;
    if revision != query.revision {
        return Err(invalid("response revision differs from request"));
    }
    let total = reader.u32()?;
    let columns = reader.columns(32)?;
    if total > 16384
        || !columns
            .iter()
            .map(|column| &column.key)
            .eq(query.columns.iter())
    {
        return Err(invalid("invalid table total or selected-column order"));
    }
    let sort_error = match reader.optional_text(64)?.as_deref() {
        None => None,
        Some("sort-callback-error") => Some(TableSortError::Callback),
        Some("inconsistent-comparison") => Some(TableSortError::InconsistentComparison),
        Some("comparison-budget") => Some(TableSortError::ComparisonBudget),
        _ => return Err(invalid("unknown sort error")),
    };
    if sort_error.is_some() && query.sort.is_none() {
        return Err(invalid("unsolicited sort error"));
    }
    let count = reader.u32()?;
    if count != total.saturating_sub(query.offset).min(query.limit) {
        return Err(invalid("invalid table page counts"));
    }
    let mut rows = Vec::with_capacity(count as usize);
    let mut seen = HashSet::new();
    let mut text_bytes = 0;
    for ordinal in 0..count {
        let row = row(&mut reader, session, query, columns.len())?;
        if row.index >= total
            || !seen.insert(row.index)
            || (sort_error.is_some() || query.sort.is_none()) && row.index != query.offset + ordinal
        {
            return Err(invalid("invalid/duplicate original row ordinal"));
        }
        if row
            .cells
            .iter()
            .zip(&columns)
            .any(|(cell, column)| !column.can_read && cell.error.is_none())
        {
            return Err(invalid("unreadable column returned a successful cell"));
        }
        for cell in &row.cells {
            text_bytes += cell.text.as_ref().map_or(0, String::len);
        }
        if text_bytes > 8 * 1024 * 1024 {
            return Err(invalid("table text exceeds aggregate limit"));
        }
        rows.push(row);
    }
    reader.finish()?;
    Ok(Some(TablePage {
        revision,
        total,
        columns,
        rows,
        sort_error,
    }))
}
fn row(
    reader: &mut Reader<'_>,
    session: u64,
    query: &TableQuery,
    column_count: usize,
) -> Result<TableRow, Error> {
    let index = reader.u32()?;
    let id = reader.u64()?;
    let combatant = reader.optional_text(1024)?;
    let category = reader.optional_text(1024)?;
    let attack = reader.optional_text(1024)?;
    let time_sorter = reader.optional_int()?;
    let (encounter, _actor, requested_category, requested_attack) = query.scope.parts();
    let depth = query.scope.level() as u32;
    if id == 0
        || encounter.is_some_and(|expected| expected.value != id)
        || combatant.is_some() != (depth >= 1)
        || category.is_some() != (depth >= 2)
        || attack.is_some() != (depth >= 3)
        || time_sorter.is_some() != (depth >= 4)
        || requested_category.is_some_and(|value| category.as_deref() != Some(value))
        || requested_attack.is_some_and(|value| attack.as_deref() != Some(value))
    {
        return Err(invalid("table row does not match scope"));
    }
    if reader.u32()? as usize != column_count {
        return Err(invalid("invalid table cell count"));
    }
    let cells = (0..column_count)
        .map(|_| reader.cell())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TableRow {
        index,
        encounter: EncounterId { session, value: id },
        combatant,
        category,
        attack,
        time_sorter,
        cells,
    })
}
