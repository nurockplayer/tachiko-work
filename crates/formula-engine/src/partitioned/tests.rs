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

fn assert_differences(before: &PartitionedMap<i32, i32>, after: &PartitionedMap<i32, i32>) {
    for (before, after) in [(before, after), (after, before)] {
        let keys = before.keys().chain(after.keys()).collect::<BTreeSet<_>>();
        let expected = keys
            .into_iter()
            .filter_map(|key| {
                let a = before.get(key);
                let b = after.get(key);
                (a != b).then_some((key, a, b))
            })
            .collect::<Vec<_>>();
        assert_eq!(before.differing_entries(after), expected);
    }
}

#[test]
fn differing_entries_identical_clone_reads_no_entries() {
    let before = PartitionedMap::from_sorted_iter((0..10_000).map(|key| (key, key)));
    let after = before.clone();
    let mut reads = 0;
    assert_eq!(before.walk_differing_entries(&after, &mut reads), []);
    assert_eq!(reads, 0);
    assert_differences(&before, &after);
}

#[test]
fn differing_entries_scalar_change_in_one_chunk() {
    let before = PartitionedMap::from_sorted_iter((0..512).map(|key| (key, key)));
    let mut after = before.clone();
    after.insert(200, -1);
    assert_eq!(
        before.differing_entries(&after),
        [(&200, Some(&200), Some(&-1))]
    );
    assert_differences(&before, &after);
}

#[test]
fn differing_entries_insert_splits_chunk() {
    let before = PartitionedMap::from_sorted_iter((0..512).map(|key| (key * 2, key)));
    let mut after = before.clone();
    after.insert(1, -1);
    assert_eq!(after.chunks.len(), before.chunks.len() + 1);
    assert_differences(&before, &after);
}

#[test]
fn differing_entries_removal_merges_chunks() {
    let before = PartitionedMap::from_sorted_iter((0..192).map(|key| (key, key)));
    let mut after = before.clone();
    for key in 0..65 {
        after.remove(&key);
    }
    assert_eq!(after.chunks.len() + 1, before.chunks.len());
    assert_differences(&before, &after);
}

#[test]
fn differing_entries_equal_content_with_independent_layouts() {
    let before = PartitionedMap::from_sorted_iter((0..512).map(|key| (key, key)));
    let independent = PartitionedMap::from_sorted_iter((0..512).map(|key| (key, key)));
    assert_eq!(before.shared_chunks(&independent).0, 0);
    assert_eq!(before.differing_entries(&independent), []);
    let mut after = PartitionedMap::default();
    for key in 0..512 {
        after.insert(key, key);
    }
    assert_ne!(before.chunks.len(), after.chunks.len());
    assert_eq!(before.differing_entries(&after), []);
    assert_differences(&before, &after);
    after.insert(127, -1);
    after.remove(&256);
    assert_differences(&before, &after);
}

#[test]
fn differing_entries_different_lengths_and_disjoint_ranges() {
    let before = PartitionedMap::from_sorted_iter((0..512).map(|key| (key, key)));
    for after in [
        PartitionedMap::default(),
        PartitionedMap::from_sorted_iter((-5..230).map(|key| (key, -key))),
        PartitionedMap::from_sorted_iter((600..800).map(|key| (key, key))),
    ] {
        assert_differences(&before, &after);
    }
    assert_differences(&PartitionedMap::default(), &PartitionedMap::default());
}
