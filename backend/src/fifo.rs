//! FIFO valuation of one part's stock ledger: whatever went out is taken
//! from the oldest pieces still on the shelf, at their purchase cost, so the
//! cost of what was used plus the value of what is left always equals what
//! was paid.
//!
//! The ledger has no purchase batches; the layers are rebuilt from the
//! movements every time, in the order they were recorded. Two cases need a
//! cost from elsewhere, the part's nearest known cost (the last one recorded
//! at or before, else the first one recorded later):
//! - pieces that came in without a cost (a stock count without one);
//! - pieces that went out while the stock was at zero or below (fitted
//!   before their delivery was typed in). The next receipt covers them
//!   first, at its own cost; whatever is never covered takes the nearest
//!   cost of the moment it went out.
//!
//! A part that never had any cost has no value at all: `None`.

use std::collections::{HashMap, VecDeque};

use serde::Serialize;
use sqlx::Postgres;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AppResult;

/// One stock movement of the part, as recorded.
pub struct Move {
    pub id: Uuid,
    pub at: OffsetDateTime,
    pub quantity_milli: i64,
    pub unit_cost_minor: Option<i64>,
}

/// Stock of the part just before a moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub quantity_milli: i64,
    /// Value of the pieces on stock; 0 while the stock is not above zero,
    /// `None` if the part never had a cost.
    pub value_minor: Option<i64>,
}

pub struct Valuation {
    /// Cost of every outgoing movement, by its id; `None` if the part never
    /// had a cost.
    pub out_cost: HashMap<Uuid, Option<i64>>,
    /// One per moment asked for, in the same order: movements recorded
    /// strictly before it count.
    pub snapshots: Vec<Snapshot>,
    /// What is on the shelf after the last movement, oldest first: the next
    /// pieces to go out. Empty if the part never had a cost.
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Layer {
    pub quantity_milli: i64,
    pub unit_cost_minor: i64,
}

/// Cost per milli-unit summed up: quantity_milli × unit cost.
type Acc = i128;

fn round_milli(acc: Acc) -> i64 {
    // Half away from zero, as PostgreSQL's round() on numeric.
    let r = if acc >= 0 {
        (acc + 500) / 1000
    } else {
        (acc - 500) / 1000
    };
    r as i64
}

/// `moves` must be in the order they were recorded; `moments` ascending.
pub fn value(moves: &[Move], moments: &[OffsetDateTime]) -> Valuation {
    let costed: Vec<(OffsetDateTime, i64)> = moves
        .iter()
        .filter(|m| m.quantity_milli > 0)
        .filter_map(|m| m.unit_cost_minor.map(|c| (m.at, c)))
        .collect();
    let nearest = |at: OffsetDateTime| -> Option<i64> {
        costed
            .iter()
            .rev()
            .find(|(t, _)| *t <= at)
            .or(costed.first())
            .map(|(_, c)| *c)
    };
    let valued = !costed.is_empty();

    // Pieces on the shelf, oldest first: (quantity, unit cost).
    let mut layers: VecDeque<(i64, i64)> = VecDeque::new();
    // Pieces that went out uncovered, oldest first: (movement, quantity left).
    let mut owed: VecDeque<(usize, i64)> = VecDeque::new();
    let mut acc: HashMap<usize, Acc> = HashMap::new();
    let mut snapshots = Vec::with_capacity(moments.len());
    let mut next_moment = 0;

    let snapshot = |layers: &VecDeque<(i64, i64)>, owed: &VecDeque<(usize, i64)>| {
        let on_shelf: i64 = layers.iter().map(|(q, _)| q).sum();
        let missing: i64 = owed.iter().map(|(_, q)| q).sum();
        let value: Acc = layers.iter().map(|(q, c)| *q as Acc * *c as Acc).sum();
        Snapshot {
            quantity_milli: on_shelf - missing,
            value_minor: valued.then(|| round_milli(value)),
        }
    };

    for (i, m) in moves.iter().enumerate() {
        while next_moment < moments.len() && moments[next_moment] <= m.at {
            snapshots.push(snapshot(&layers, &owed));
            next_moment += 1;
        }
        if m.quantity_milli > 0 {
            let cost = m.unit_cost_minor.or_else(|| nearest(m.at)).unwrap_or(0);
            let mut left = m.quantity_milli;
            while left > 0 {
                let Some((idx, q)) = owed.front_mut() else {
                    break;
                };
                let take = left.min(*q);
                *acc.entry(*idx).or_default() += take as Acc * cost as Acc;
                *q -= take;
                left -= take;
                if *q == 0 {
                    owed.pop_front();
                }
            }
            if left > 0 {
                layers.push_back((left, cost));
            }
        } else if m.quantity_milli < 0 {
            let mut left = -m.quantity_milli;
            let a = acc.entry(i).or_default();
            while left > 0 {
                let Some((q, cost)) = layers.front_mut() else {
                    break;
                };
                let take = left.min(*q);
                *a += take as Acc * *cost as Acc;
                *q -= take;
                left -= take;
                if *q == 0 {
                    layers.pop_front();
                }
            }
            if left > 0 {
                owed.push_back((i, left));
            }
        }
    }
    while next_moment < moments.len() {
        snapshots.push(snapshot(&layers, &owed));
        next_moment += 1;
    }
    // Never covered: at the nearest cost of the moment they went out.
    for (idx, q) in owed {
        let cost = nearest(moves[idx].at).unwrap_or(0);
        *acc.entry(idx).or_default() += q as Acc * cost as Acc;
    }

    let out_cost = moves
        .iter()
        .enumerate()
        .filter(|(_, m)| m.quantity_milli < 0)
        .map(|(i, m)| {
            let cost = acc.get(&i).copied().unwrap_or(0);
            (m.id, valued.then(|| round_milli(cost)))
        })
        .collect();
    let layers = if valued {
        layers
            .into_iter()
            .map(|(q, c)| Layer {
                quantity_milli: q,
                unit_cost_minor: c,
            })
            .collect()
    } else {
        Vec::new()
    };
    Valuation {
        out_cost,
        snapshots,
        layers,
    }
}

/// Valuations of the given parts as they stand now.
pub async fn for_parts<'e, E>(db: E, part_ids: &[Uuid]) -> AppResult<HashMap<Uuid, Valuation>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        part_id: Uuid,
        quantity_milli: i64,
        unit_cost_minor: Option<i64>,
        created_at: OffsetDateTime,
    }
    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, part_id, quantity_milli, unit_cost_minor, created_at
           FROM stock_movements
          WHERE part_id = ANY($1)
          ORDER BY part_id, created_at, id",
    )
    .bind(part_ids)
    .fetch_all(db)
    .await?;
    Ok(rows
        .chunk_by(|a, b| a.part_id == b.part_id)
        .map(|chunk| {
            let moves: Vec<Move> = chunk
                .iter()
                .map(|r| Move {
                    id: r.id,
                    at: r.created_at,
                    quantity_milli: r.quantity_milli,
                    unit_cost_minor: r.unit_cost_minor,
                })
                .collect();
            (chunk[0].part_id, value(&moves, &[]))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    fn t(day: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::days(day)
    }

    fn mv(day: i64, qty: i64, cost: Option<i64>) -> Move {
        Move {
            id: Uuid::new_v4(),
            at: t(day),
            quantity_milli: qty * 1000,
            unit_cost_minor: cost,
        }
    }

    #[test]
    fn takes_the_oldest_pieces_first() {
        // 56, 80 and 100 paid; one fitted.
        let moves = [
            mv(1, 1, Some(5600)),
            mv(2, 1, Some(8000)),
            mv(3, 1, Some(10000)),
            mv(4, -1, None),
        ];
        let v = value(&moves, &[t(5)]);
        assert_eq!(v.out_cost[&moves[3].id], Some(5600));
        assert_eq!(
            v.layers,
            [
                Layer {
                    quantity_milli: 1000,
                    unit_cost_minor: 8000
                },
                Layer {
                    quantity_milli: 1000,
                    unit_cost_minor: 10000
                }
            ]
        );
        assert_eq!(
            v.snapshots[0],
            Snapshot {
                quantity_milli: 2000,
                value_minor: Some(18000)
            }
        );
    }

    #[test]
    fn spans_layers_and_keeps_the_money_whole() {
        let moves = [mv(1, 2, Some(1000)), mv(2, 3, Some(1500)), mv(3, -4, None)];
        let v = value(&moves, &[t(3), t(4)]);
        assert_eq!(v.out_cost[&moves[2].id], Some(2 * 1000 + 2 * 1500));
        // Before the fitting everything is there; after it one at 15.00.
        assert_eq!(v.snapshots[0].value_minor, Some(6500));
        assert_eq!(v.snapshots[1].value_minor, Some(1500));
    }

    #[test]
    fn fitted_before_delivery_takes_the_delivery_cost() {
        let moves = [mv(1, -1, None), mv(2, 1, Some(4200)), mv(3, 1, Some(5000))];
        let v = value(&moves, &[t(2), t(4)]);
        assert_eq!(v.out_cost[&moves[0].id], Some(4200));
        assert_eq!(
            v.snapshots[0],
            Snapshot {
                quantity_milli: -1000,
                value_minor: Some(0)
            }
        );
        // The first receipt covered the missing piece; the second is on stock.
        assert_eq!(
            v.snapshots[1],
            Snapshot {
                quantity_milli: 1000,
                value_minor: Some(5000)
            }
        );
    }

    #[test]
    fn never_covered_uses_the_nearest_cost() {
        let moves = [mv(1, 1, Some(3000)), mv(2, -2, None)];
        let v = value(&moves, &[]);
        assert_eq!(v.out_cost[&moves[1].id], Some(6000));
    }

    #[test]
    fn a_count_without_cost_borrows_the_nearest_one() {
        let moves = [mv(1, 5, None), mv(2, 1, Some(2000)), mv(3, -1, None)];
        let v = value(&moves, &[t(4)]);
        // The counted pieces are older, valued at the first cost known later.
        assert_eq!(v.out_cost[&moves[2].id], Some(2000));
        assert_eq!(v.snapshots[0].value_minor, Some(10000));
    }

    #[test]
    fn no_cost_at_all_means_no_value() {
        let moves = [mv(1, 3, None), mv(2, -1, None)];
        let v = value(&moves, &[t(3)]);
        assert_eq!(v.out_cost[&moves[1].id], None);
        assert_eq!(
            v.snapshots[0],
            Snapshot {
                quantity_milli: 2000,
                value_minor: None
            }
        );
    }

    #[test]
    fn fractions_round_half_up() {
        // 2 l at 33.33: 1.5 l used (49.995), 0.5 l left (16.665).
        let moves = [
            Move {
                id: Uuid::new_v4(),
                at: t(1),
                quantity_milli: 2000,
                unit_cost_minor: Some(3333),
            },
            Move {
                id: Uuid::new_v4(),
                at: t(2),
                quantity_milli: -1500,
                unit_cost_minor: None,
            },
        ];
        let v = value(&moves, &[t(3)]);
        assert_eq!(v.out_cost[&moves[1].id], Some(5000)); // 4999.5 rounds up
        assert_eq!(v.snapshots[0].value_minor, Some(1667)); // 1666.5
    }
}
