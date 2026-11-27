//! Ordered immutable partitions shared by retained calculation revisions.

use std::collections::{BTreeMap, BTreeSet, btree_map};
use std::fmt;
use std::ops::{Bound, Index, RangeBounds};
use std::sync::Arc;

const MIN: usize = 64;
const MAX: usize = 128;

type Iter<'a, K, V> = std::iter::FlatMap<
    std::slice::Iter<'a, Arc<BTreeMap<K, V>>>,
    btree_map::Iter<'a, K, V>,
    fn(&'a Arc<BTreeMap<K, V>>) -> btree_map::Iter<'a, K, V>,
>;

type DifferingEntry<'a, K, V> = (&'a K, Option<&'a V>, Option<&'a V>);

/// Merge-compare one overlapping directory range, retaining only differences.
fn compare_entries<'a, K: Ord, V: PartialEq>(
    before: impl Iterator<Item = (&'a K, &'a V)>,
    after: impl Iterator<Item = (&'a K, &'a V)>,
    differences: &mut Vec<DifferingEntry<'a, K, V>>,
    #[cfg(test)] reads: &mut usize,
) {
    let mut before = before.peekable();
    let mut after = after.peekable();
    while before.peek().is_some() || after.peek().is_some() {
        #[cfg(test)]
        {
            *reads += usize::from(before.peek().is_some()) + usize::from(after.peek().is_some());
        }
        let order = match (before.peek(), after.peek()) {
            (Some((a, _)), Some((b, _))) => a.cmp(b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => break,
        };
        match order {
            std::cmp::Ordering::Equal => {
                let (a, av) = before.next().expect("peeked before entry");
                let (_, bv) = after.next().expect("peeked after entry");
                if av != bv {
                    differences.push((a, Some(av), Some(bv)));
                }
            }
            std::cmp::Ordering::Less => {
                let (key, value) = before.next().expect("peeked before entry");
                differences.push((key, Some(value), None));
            }
            std::cmp::Ordering::Greater => {
                let (key, value) = after.next().expect("peeked after entry");
                differences.push((key, None, Some(value)));
            }
        }
    }
}

/// An ordered borrowed view of calculation values, backed by shared partitions.
/// Mutation is restricted to formula-engine; cloning copies only the directory.
#[derive(Clone)]
pub struct PartitionedMap<K, V> {
    chunks: Vec<Arc<BTreeMap<K, V>>>,
    // Cached inclusive lower bounds; the next bound is the exclusive upper bound.
    starts: Vec<K>,
    len: usize,
}

impl<K, V> Default for PartitionedMap<K, V> {
    fn default() -> Self {
        Self {
            chunks: Vec::new(),
            starts: Vec::new(),
            len: 0,
        }
    }
}

impl<K: Ord, V> PartitionedMap<K, V> {
    fn chunk_index(&self, key: &K) -> usize {
        self.starts
            .partition_point(|first| first <= key)
            .saturating_sub(1)
    }

    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.chunks.get(self.chunk_index(key))?.get(key)
    }

    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> {
        self.into_iter()
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> {
        self.iter().map(|(key, _)| key)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> {
        self.iter().map(|(_, value)| value)
    }

    /// Borrow entries between bounds in key order.
    ///
    /// # Panics
    /// Panics if the start exceeds the end or both equal bounds are excluded.
    pub fn range<R: RangeBounds<K> + Clone>(
        &self,
        bounds: R,
    ) -> impl DoubleEndedIterator<Item = (&K, &V)> {
        let start = match bounds.start_bound() {
            Bound::Included(key) | Bound::Excluded(key) => self.chunk_index(key),
            Bound::Unbounded => 0,
        };
        let end = match bounds.end_bound() {
            Bound::Included(key) | Bound::Excluded(key) if !self.is_empty() => {
                self.chunk_index(key) + 1
            }
            _ => self.chunks.len(),
        };
        // Let BTreeMap enforce its standard invalid-bound behavior even when empty.
        if let (Bound::Included(a) | Bound::Excluded(a), Bound::Included(b) | Bound::Excluded(b)) =
            (bounds.start_bound(), bounds.end_bound())
        {
            assert!(a <= b, "range start is greater than range end");
            assert!(
                a != b
                    || !matches!(
                        (bounds.start_bound(), bounds.end_bound()),
                        (Bound::Excluded(_), Bound::Excluded(_))
                    ),
                "range excludes both equal bounds"
            );
        }
        self.chunks[start..end]
            .iter()
            .flat_map(move |chunk| chunk.range(bounds.clone()))
    }
}

impl<K: Ord, V: PartialEq> PartitionedMap<K, V> {
    /// Walk the union of directory boundaries, skipping shared immutable ranges.
    pub(crate) fn differing_entries<'a>(
        &'a self,
        after: &'a Self,
    ) -> Vec<DifferingEntry<'a, K, V>> {
        self.walk_differing_entries(
            after,
            #[cfg(test)]
            &mut 0,
        )
    }

    fn walk_differing_entries<'a>(
        &'a self,
        after: &'a Self,
        #[cfg(test)] reads: &mut usize,
    ) -> Vec<DifferingEntry<'a, K, V>> {
        let mut differences = Vec::new();
        let (mut left, mut right) = (0, 0);
        loop {
            let lower = match (self.starts.get(left), after.starts.get(right)) {
                (Some(a), Some(b)) => a.min(b),
                (Some(key), None) | (None, Some(key)) => key,
                (None, None) => break,
            };
            left += usize::from(self.starts.get(left) == Some(lower));
            right += usize::from(after.starts.get(right) == Some(lower));
            let upper = match (self.starts.get(left), after.starts.get(right)) {
                (Some(a), Some(b)) => Bound::Excluded(a.min(b)),
                (Some(key), None) | (None, Some(key)) => Bound::Excluded(key),
                (None, None) => Bound::Unbounded,
            };
            let before_chunk = left.checked_sub(1).map(|index| &self.chunks[index]);
            let after_chunk = right.checked_sub(1).map(|index| &after.chunks[index]);
            if matches!((before_chunk, after_chunk), (Some(a), Some(b)) if Arc::ptr_eq(a, b)) {
                continue;
            }
            let bounds = (Bound::Included(lower), upper);
            compare_entries(
                before_chunk
                    .into_iter()
                    .flat_map(|chunk| chunk.range::<K, _>(bounds)),
                after_chunk
                    .into_iter()
                    .flat_map(|chunk| chunk.range::<K, _>(bounds)),
                &mut differences,
                #[cfg(test)]
                reads,
            );
        }
        differences
    }
}

impl<K: Ord + Clone, V: Clone> PartitionedMap<K, V> {
    /// Build once from entries already sorted by unique key.
    pub(crate) fn from_sorted_iter(entries: impl IntoIterator<Item = (K, V)>) -> Self {
        let mut result = Self::default();
        let mut entries = entries.into_iter().peekable();
        while entries.peek().is_some() {
            let chunk: BTreeMap<_, _> = entries.by_ref().take(MAX).collect();
            result.starts.push(
                chunk
                    .first_key_value()
                    .expect("nonempty bulk chunk")
                    .0
                    .clone(),
            );
            result.len += chunk.len();
            result.chunks.push(Arc::new(chunk));
        }
        result
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> Option<V> {
        if self.chunks.is_empty() {
            self.chunks.push(Arc::new(BTreeMap::new()));
            self.starts.push(key.clone());
        }
        let index = self.chunk_index(&key);
        if key < self.starts[index] {
            self.starts[index] = key.clone();
        }
        let previous = Arc::make_mut(&mut self.chunks[index]).insert(key, value);
        if previous.is_none() {
            self.len += 1;
        }
        if self.chunks[index].len() > MAX {
            self.split(index);
        }
        previous
    }

    fn split(&mut self, index: usize) {
        let chunk = Arc::make_mut(&mut self.chunks[index]);
        let middle = chunk
            .keys()
            .nth(chunk.len() / 2)
            .cloned()
            .expect("split nonempty chunk");
        let right = chunk.split_off(&middle);
        self.chunks.insert(index + 1, Arc::new(right));
        self.starts.insert(index + 1, middle);
    }

    pub(crate) fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let index = self.chunk_index(key);
        if !self.chunks.get(index)?.contains_key(key) {
            return None;
        }
        Arc::make_mut(&mut self.chunks[index]).get_mut(key)
    }

    pub(crate) fn get_or_default(&mut self, key: &K) -> &mut V
    where
        V: Default,
    {
        if !self.contains_key(key) {
            self.insert(key.clone(), V::default());
        }
        self.get_mut(key).expect("key was inserted")
    }

    pub(crate) fn remove(&mut self, key: &K) -> Option<V> {
        let index = self.chunk_index(key);
        if !self.chunks.get(index)?.contains_key(key) {
            return None;
        }
        let previous = Arc::make_mut(&mut self.chunks[index]).remove(key);
        self.len -= 1;
        if self.chunks[index].is_empty() {
            self.chunks.remove(index);
            self.starts.remove(index);
        } else {
            self.starts[index] = self.chunks[index]
                .first_key_value()
                .expect("nonempty remaining chunk")
                .0
                .clone();
            if self.chunks[index].len() < MIN && self.chunks.len() > 1 {
                let left = if index + 1 < self.chunks.len() {
                    index
                } else {
                    index - 1
                };
                let right = self.chunks.remove(left + 1);
                self.starts.remove(left + 1);
                Arc::make_mut(&mut self.chunks[left]).append(&mut Arc::unwrap_or_clone(right));
                if self.chunks[left].len() > MAX {
                    self.split(left);
                }
            }
        }
        previous
    }

    #[cfg(test)]
    pub(crate) fn shared_chunks(&self, other: &Self) -> (usize, usize) {
        let shared = self
            .chunks
            .iter()
            .filter(|chunk| other.chunks.iter().any(|old| Arc::ptr_eq(chunk, old)))
            .count();
        (shared, self.chunks.len())
    }
}

impl<K: Ord + Clone, V: Clone> FromIterator<(K, V)> for PartitionedMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Self::from_sorted_iter(iter.into_iter().collect::<BTreeMap<_, _>>())
    }
}

impl<K: Ord + Clone, V: Clone> PartitionedMap<K, V> {
    pub(crate) fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

impl<'a, K, V> IntoIterator for &'a PartitionedMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.chunks.iter().flat_map(|chunk| chunk.iter())
    }
}

impl<K: Ord, V: PartialEq> PartialEq for PartitionedMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len
            && ((self.chunks.len() == other.chunks.len()
                && self
                    .chunks
                    .iter()
                    .zip(&other.chunks)
                    .all(|(a, b)| Arc::ptr_eq(a, b)))
                || self.iter().eq(other.iter()))
    }
}
impl<K: Ord, V: Eq> Eq for PartitionedMap<K, V> {}

impl<K: Ord + fmt::Debug, V: fmt::Debug> fmt::Debug for PartitionedMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self).finish()
    }
}

impl<K: Ord, V> Index<&K> for PartitionedMap<K, V> {
    type Output = V;
    fn index(&self, key: &K) -> &V {
        self.get(key).expect("key is absent")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionedSet<K: Ord>(PartitionedMap<K, ()>);

impl<K: Ord> Default for PartitionedSet<K> {
    fn default() -> Self {
        Self(PartitionedMap::default())
    }
}

impl<K: Ord + Clone> PartitionedSet<K> {
    #[cfg(test)]
    pub(crate) fn shared_chunks(&self, other: &Self) -> (usize, usize) {
        self.0.shared_chunks(&other.0)
    }

    pub(crate) fn insert(&mut self, key: K) -> bool {
        self.0.insert(key, ()).is_none()
    }
    pub(crate) fn remove(&mut self, key: &K) -> bool {
        self.0.remove(key).is_some()
    }
    pub(crate) fn contains(&self, key: &K) -> bool {
        self.0.contains_key(key)
    }
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &K> {
        self.0.keys()
    }

    pub(crate) fn intersection<'a>(
        &'a self,
        other: &'a BTreeSet<K>,
    ) -> impl Iterator<Item = &'a K> {
        let mut own = self.iter();
        let mut other_keys = other.iter();
        std::iter::from_fn(move || {
            if self.len() <= other.len() {
                own.find(|key| other.contains(*key))
            } else {
                other_keys.find(|key| self.contains(*key))
            }
        })
    }
}

impl<K: Ord + Clone> FromIterator<K> for PartitionedSet<K> {
    fn from_iter<T: IntoIterator<Item = K>>(iter: T) -> Self {
        Self(iter.into_iter().map(|key| (key, ())).collect())
    }
}

impl<'a, K: Ord + Clone> IntoIterator for &'a PartitionedSet<K> {
    type Item = &'a K;
    type IntoIter = std::iter::Map<Iter<'a, K, ()>, fn((&'a K, &'a ())) -> &'a K>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().map(|(key, ())| key)
    }
}

impl<K: Ord + Clone> IntoIterator for PartitionedSet<K> {
    type Item = K;
    type IntoIter = std::iter::FlatMap<
        std::vec::IntoIter<Arc<BTreeMap<K, ()>>>,
        btree_map::IntoKeys<K, ()>,
        fn(Arc<BTreeMap<K, ()>>) -> btree_map::IntoKeys<K, ()>,
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.0
            .chunks
            .into_iter()
            .flat_map(|chunk| Arc::unwrap_or_clone(chunk).into_keys())
    }
}

impl<K: Ord + Clone> From<BTreeSet<K>> for PartitionedSet<K> {
    fn from(keys: BTreeSet<K>) -> Self {
        Self(PartitionedMap::from_sorted_iter(
            keys.into_iter().map(|key| (key, ())),
        ))
    }
}

#[cfg(test)]
mod tests;
