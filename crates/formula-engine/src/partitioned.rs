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
mod tests {
    use super::*;

    fn assert_contents(map: &PartitionedMap<i32, i32>, expected: &BTreeMap<i32, i32>) {
        assert_eq!(map.len(), expected.len());
        assert_eq!(map.starts.len(), map.chunks.len());
        assert!(map.iter().eq(expected.iter()));
        assert!(map.iter().rev().eq(expected.iter().rev()));
        assert_eq!(map.is_empty(), expected.is_empty());
        for (index, chunk) in map.chunks.iter().enumerate() {
            assert!(!chunk.is_empty());
            assert_eq!(
                Some(&map.starts[index]),
                chunk.first_key_value().map(|(key, _)| key)
            );
            assert!(chunk.len() <= MAX);
            if index > 0 && index + 1 < map.chunks.len() {
                assert!(chunk.len() >= MIN);
            }
        }
    }

    #[test]
    fn bulk_build_ten_thousand_sorted_entries_has_bounded_chunks() {
        let map = PartitionedMap::from_sorted_iter((0..10_000).map(|key| (key, key)));
        assert_eq!(map.chunks.len(), 79);
        assert_contents(&map, &(0..10_000).map(|key| (key, key)).collect());
    }

    #[test]
    fn btree_set_conversion_ten_thousand_keys_has_bounded_chunks() {
        let set = PartitionedSet::from((0..10_000).collect::<BTreeSet<_>>());
        assert_eq!(set.0.chunks.len(), 79);
        assert_contents(
            &PartitionedMap::from_sorted_iter(set.iter().map(|key| (*key, *key))),
            &(0..10_000).map(|key| (key, key)).collect(),
        );
        assert!(set.0.chunks.iter().all(|chunk| chunk.len() <= MAX));
        assert!(
            set.0
                .chunks
                .iter()
                .take(set.0.chunks.len() - 1)
                .all(|chunk| chunk.len() >= MIN)
        );
    }

    #[test]
    fn edge_inserts_and_removals_split_merge_and_preserve_contents() {
        let mut map = PartitionedMap::default();
        let mut expected = BTreeMap::new();
        // Alternating ends and then interior chunk boundaries exercise both neighbors.
        for key in (0..1_024).map(|key| key * 2) {
            assert_eq!(map.insert(key, key), expected.insert(key, key));
        }
        for key in (1..2_048).step_by(2).rev() {
            assert_eq!(map.insert(key, -key), expected.insert(key, -key));
            assert_contents(&map, &expected);
        }
        for residue in 0..4 {
            for key in (residue..2_048).step_by(4) {
                assert_eq!(map.remove(&key), expected.remove(&key));
                assert_contents(&map, &expected);
            }
        }
        assert_eq!(map.remove(&9), None);
        assert_eq!(map.chunks, [] as [Arc<BTreeMap<i32, i32>>; 0]);
    }

    #[test]
    fn equality_ignores_chunk_layout_and_debug_is_map_shaped() {
        let bulk = PartitionedMap::from_sorted_iter((0..512).map(|key| (key, key)));
        let mut inserted = PartitionedMap::default();
        for key in 0..512 {
            inserted.insert(key, key);
        }
        assert_ne!(bulk.chunks.len(), inserted.chunks.len());
        assert_eq!(bulk, inserted);
        assert_eq!(
            format!("{bulk:?}"),
            format!(
                "{:?}",
                (0..512).map(|key| (key, key)).collect::<BTreeMap<_, _>>()
            )
        );
        inserted.insert(42, -1);
        assert_ne!(bulk, inserted);
    }

    #[test]
    fn clone_then_one_insert_detaches_only_one_chunk() {
        let base = PartitionedMap::from_sorted_iter((0..10_000).map(|key| (key, key)));
        let mut candidate = base.clone();
        assert_eq!(candidate.insert(5_000, -1), Some(5_000));
        let (shared, total) = candidate.shared_chunks(&base);
        assert_eq!(shared + 1, total);
        assert_eq!(base.get(&5_000), Some(&5_000));
        assert_eq!(candidate.get(&5_000), Some(&-1));
        *candidate.get_mut(&9_000).unwrap() = -2;
        assert_eq!(base.get(&9_000), Some(&9_000));
        assert_eq!(candidate.shared_chunks(&base).0 + 2, total);
    }

    #[test]
    fn ordered_ranges_and_duplicate_inputs_match_btree_reads() {
        let expected: BTreeMap<_, _> = (0..400).map(|key| (key, key)).collect();
        let map: PartitionedMap<_, _> = expected.iter().map(|(k, v)| (*k, *v)).collect();
        for start in [-1, 0, 63, 127, 128, 399, 400] {
            for end in [start, start + 1, 401] {
                assert!(map.range(start..=end).eq(expected.range(start..=end)));
                assert!(
                    map.range(start..end)
                        .rev()
                        .eq(expected.range(start..end).rev())
                );
            }
        }
        assert!(map.range(..).eq(expected.range(..)));
        let duplicates: PartitionedMap<_, _> = [(2, 1), (1, 4), (2, 9)].into_iter().collect();
        assert_eq!(duplicates.len(), 2);
        assert_eq!(duplicates.get(&2), Some(&9));
        let empty = PartitionedMap::<i32, i32>::default();
        assert_eq!(empty.range(..).next(), None);
    }

    #[test]
    fn high_fanout_sets_detach_bounded_inner_partitions() {
        let base: PartitionedSet<_> = (0..10_000).collect();
        let mut candidate = base.clone();
        assert!(candidate.remove(&5_000));
        let (shared, total) = candidate.0.shared_chunks(&base.0);
        assert_eq!(shared + 1, total);
        assert!(base.contains(&5_000));
        assert!(!candidate.contains(&5_000));
    }
}
