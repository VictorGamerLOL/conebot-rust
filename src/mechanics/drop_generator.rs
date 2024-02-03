use std::collections::HashMap;

use anyhow::Result;
use rand::distributions::{ uniform::SampleRange, Distribution };

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DropGenerator<T> where T: SampleRange<i64> + Clone {
    drops: Vec<Droppable<T>>,
    amount: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Droppable<T> where T: SampleRange<i64> + Clone {
    name: String,
    kind: DroppableKind,
    range: T,
    weight: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DroppableKind {
    Item,
    Currency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DropResult<'a> {
    pub result: DropResultKind<'a>,
    pub quantity: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DropResultKind<'a> {
    Item(&'a str),
    Currency(&'a str),
}

impl DropResult<'_> {
    pub const fn name(&self) -> &str {
        match self.result {
            DropResultKind::Currency(name) | DropResultKind::Item(name) => name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct DroppableSmall<'a> {
    name: &'a str,
    kind: DroppableKind,
}

impl<T> Default for DropGenerator<T> where T: SampleRange<i64> + Clone {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> DropGenerator<T> where T: SampleRange<i64> + Clone {
    pub const fn new() -> Self {
        Self {
            drops: Vec::new(),
            amount: 1,
        }
    }

    pub fn add_droppable(&mut self, droppable: Droppable<T>) {
        self.drops.push(droppable);
    }

    #[allow(clippy::option_if_let_else)] // cannot borrow mutable more than once
    pub fn generate(&self, times: i64) -> Result<Vec<DropResult<'_>>> {
        let mut rng = rand::rngs::OsRng;
        let (choices, weights): (Vec<_>, Vec<_>) = self.drops
            .iter()
            .map(|d| (d.as_small(), d.weight))
            .unzip();

        let dist = rand::distributions::WeightedIndex::new(weights)?;
        let mut results: HashMap<DroppableSmall<'_>, i64> = HashMap::new();
        for _ in 0..self.amount * times {
            let choice = dist.sample(&mut rng);
            let droppable = choices[choice];
            let amount = self.drops[choice].range.clone().sample_single(&mut rng);
            if let Some(amount_stored) = results.get_mut(&droppable) {
                *amount_stored += amount;
            } else {
                results.insert(droppable, amount);
            }
        }

        Ok(
            results
                .into_iter()
                .map(|(d, a)| DropResult {
                    result: match d.kind {
                        DroppableKind::Item => DropResultKind::Item(d.name),
                        DroppableKind::Currency => DropResultKind::Currency(d.name),
                    },
                    quantity: a,
                })
                .collect()
        )
    }
}

impl<T> Droppable<T> where T: SampleRange<i64> + Clone {
    pub const fn new(name: String, kind: DroppableKind, range: T, weight: i64) -> Self {
        Self {
            name,
            kind,
            range,
            weight,
        }
    }
    fn as_small(&self) -> DroppableSmall<'_> {
        DroppableSmall {
            name: &self.name,
            kind: self.kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::{ Range, RangeInclusive };

    use super::*;

    #[test]
    fn test_drop_generator_new() {
        let drop_gen: DropGenerator<Range<i64>> = DropGenerator::new();
        assert_eq!(drop_gen.drops.len(), 0);
        assert_eq!(drop_gen.amount, 1);
    }

    #[test]
    fn test_drop_generator_add_droppable() {
        let mut drop_gen: DropGenerator<RangeInclusive<i64>> = DropGenerator::new();
        let droppable = Droppable {
            name: "Test Item".to_string(),
            kind: DroppableKind::Item,
            range: 1..=10,
            weight: 1,
        };
        drop_gen.add_droppable(droppable.clone());
        assert_eq!(drop_gen.drops.len(), 1);
        assert_eq!(drop_gen.drops[0], droppable);
    }

    #[test]
    fn test_drop_generator_generate() {
        let mut drop_gen: DropGenerator<RangeInclusive<i64>> = DropGenerator::new();
        let droppable = Droppable {
            name: "Test Item".to_string(),
            kind: DroppableKind::Item,
            range: 1..=10,
            weight: 1,
        };
        drop_gen.add_droppable(droppable);
        let result = drop_gen.generate(1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_drop_generator_weights() {
        let mut drop_gen: DropGenerator<RangeInclusive<i64>> = DropGenerator::new();
        let droppable1 = Droppable {
            name: "Test Item 1".to_string(),
            kind: DroppableKind::Item,
            range: 1..=10,
            weight: 1,
        };
        let droppable2 = Droppable {
            name: "Test Item 2".to_string(),
            kind: DroppableKind::Item,
            range: 1..=10,
            weight: 10,
        };
        drop_gen.add_droppable(droppable1);
        drop_gen.add_droppable(droppable2);

        let mut count1 = 0;
        let mut count2 = 0;
        for _ in 0..1000 {
            let results = drop_gen.generate(1).unwrap();
            for result in results {
                if result.name() == "Test Item 1" {
                    count1 += 1;
                } else if result.name() == "Test Item 2" {
                    count2 += 1;
                }
            }
        }
        assert!(count2 > count1, "Item with higher weight should drop more frequently");
    }

    #[test]
    fn test_drop_generator_range() {
        let mut drop_gen: DropGenerator<RangeInclusive<i64>> = DropGenerator::new();
        let droppable = Droppable {
            name: "Test Item".to_string(),
            kind: DroppableKind::Item,
            range: 5..=10,
            weight: 1,
        };
        drop_gen.add_droppable(droppable);

        for _ in 0..1000 {
            let results = drop_gen.generate(1).unwrap();
            for result in results {
                assert!(
                    result.quantity >= 5 && result.quantity <= 10,
                    "Quantity should be within the specified range"
                );
            }
        }
    }

    #[test]
    fn test_drop_generator_single_item_range() {
        let mut drop_gen: DropGenerator<RangeInclusive<i64>> = DropGenerator::new();
        let droppable = Droppable {
            name: "Test Item".to_string(),
            kind: DroppableKind::Item,
            range: 5..=5,
            weight: 1,
        };
        drop_gen.add_droppable(droppable);

        for _ in 0..1000 {
            let results = drop_gen.generate(1).unwrap();
            for result in results {
                assert!(
                    result.quantity == 5,
                    "Quantity should be equal to the single item in the range"
                );
            }
        }
    }
}
