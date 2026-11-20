//! Stored-order allocation for atomic row insertion (ADR-0048).

use crate::{
    Document, Entity, RowRank, SchemaId, compare_row_order, patch_lifecycle::SemanticCommand,
};

const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Assign new rows their insertion order and return exact forward/inverse rank
/// facts for existing rows. The caller publishes these with the new entities.
/// `index` addresses the schema's current stored order; callers validate anchors.
#[must_use]
pub fn plan_insertion(
    document: &Document,
    schema: &SchemaId,
    new_rows: &mut [Entity],
    index: usize,
    positional: bool,
) -> (Vec<SemanticCommand>, Vec<SemanticCommand>) {
    let mut rows = document
        .entities
        .values()
        .filter(|row| row.schema == *schema)
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| compare_row_order(left, right));
    let unranked = rows.iter().all(|row| row.rank.is_none());
    if !positional
        && unranked
        && rows
            .last()
            .into_iter()
            .map(|row| &row.id)
            .chain(new_rows.iter().map(|row| &row.id))
            .collect::<Vec<_>>()
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    {
        return (Vec::new(), Vec::new());
    }
    let left = index.checked_sub(1).map(|index| rows[index]);
    let right = rows.get(index).copied();
    // A rank cannot follow an unranked row. First positional insertion also
    // assigns every old row, even when the requested position is at an edge.
    if !(positional && unranked) && left.is_none_or(|row| row.rank.is_some()) {
        let mut lower = left.and_then(|row| row.rank.clone());
        let upper = right.and_then(|row| row.rank.as_ref());
        let mut ranks = Vec::with_capacity(new_rows.len());
        for _ in new_rows.iter() {
            let Some(rank) = between(lower.as_ref(), upper) else {
                break;
            };
            lower = Some(rank.clone());
            ranks.push(rank);
        }
        if ranks.len() == new_rows.len() {
            for (row, rank) in new_rows.iter_mut().zip(ranks) {
                row.rank = Some(rank);
            }
            return (Vec::new(), Vec::new());
        }
    }
    rebalance(&rows, new_rows, index)
}

fn between(lower: Option<&RowRank>, upper: Option<&RowRank>) -> Option<RowRank> {
    if lower
        .zip(upper)
        .is_some_and(|(lower, upper)| lower >= upper)
    {
        return None;
    }
    let lower = lower.map_or(b"".as_slice(), |rank| rank.as_str().as_bytes());
    let upper = upper.map(|rank| rank.as_str().as_bytes());
    let mut bounded = upper.is_some();
    let mut result = String::new();
    for index in 0..64 {
        let low = lower.get(index).map_or(0, |byte| digit(*byte));
        let high = if bounded {
            upper
                .and_then(|bytes| bytes.get(index))
                .map_or(62, |byte| digit(*byte))
        } else {
            62
        };
        if high > low + 1 {
            result.push(char::from(DIGITS[(low + high) / 2]));
            return Some(RowRank::new(result).expect("allocated rank is valid"));
        }
        result.push(char::from(DIGITS[low]));
        if low < high {
            bounded = false;
        }
    }
    None
}

fn digit(byte: u8) -> usize {
    DIGITS
        .iter()
        .position(|candidate| *candidate == byte)
        .expect("validated rank alphabet")
}

fn rebalance(
    rows: &[&Entity],
    new_rows: &mut [Entity],
    index: usize,
) -> (Vec<SemanticCommand>, Vec<SemanticCommand>) {
    let count = rows.len() + new_rows.len();
    let step = 62_u64.pow(8) / u64::try_from(count + 1).expect("bounded table length");
    let mut forward = Vec::new();
    let mut inverse = Vec::new();
    for (offset, row) in rows.iter().enumerate() {
        let position = if offset < index {
            offset
        } else {
            offset + new_rows.len()
        };
        let rank = Some(balanced_rank(step, position));
        if row.rank != rank {
            forward.push(SemanticCommand::SetEntityRank {
                entity: row.id.clone(),
                rank,
            });
            inverse.push(SemanticCommand::SetEntityRank {
                entity: row.id.clone(),
                rank: row.rank.clone(),
            });
        }
    }
    for (offset, row) in new_rows.iter_mut().enumerate() {
        row.rank = Some(balanced_rank(step, index + offset));
    }
    (forward, inverse)
}

fn balanced_rank(step: u64, index: usize) -> RowRank {
    let mut value = step * u64::try_from(index + 1).expect("bounded row position");
    let mut bytes = [b'0'; 9];
    bytes[8] = b'V';
    for byte in bytes[..8].iter_mut().rev() {
        *byte = DIGITS[usize::try_from(value % 62).expect("base62 digit")];
        value /= 62;
    }
    RowRank::new(String::from_utf8(bytes.to_vec()).expect("ASCII rank")).expect("balanced rank")
}
