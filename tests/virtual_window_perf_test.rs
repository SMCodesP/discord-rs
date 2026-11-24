extern crate gpui_kit as gpui;

use std::time::Instant;

use discord_rs::chat::virtual_window::{FenwickTree, VirtualWindow};

fn naive_prefix_sum(heights: &[u32], index: usize) -> i64 {
    heights[..index.min(heights.len())]
        .iter()
        .map(|&h| h as i64)
        .sum()
}

fn naive_index_at_offset(heights: &[u32], offset: i64) -> Option<usize> {
    if heights.is_empty() {
        return None;
    }
    if offset <= 0 {
        return Some(0);
    }
    let mut acc = 0i64;
    for (i, &h) in heights.iter().enumerate() {
        acc += h as i64;
        if acc > offset {
            return Some(i);
        }
    }
    Some(heights.len() - 1)
}

#[test]
fn fenwick_prefix_sums_match_naive() {
    let heights: Vec<u32> = (1..=200).map(|i| (i * 7 % 53) as u32 + 8).collect();
    let tree = FenwickTree::from_heights(&heights);
    let total: i64 = heights.iter().map(|&h| h as i64).sum();
    assert_eq!(tree.total_height(), total);
    for i in 0..=heights.len() {
        assert_eq!(tree.prefix_sum(i), naive_prefix_sum(&heights, i), "prefix {i}");
        if i < heights.len() {
            assert_eq!(tree.offset_of(i), naive_prefix_sum(&heights, i), "offset {i}");
        }
    }
}

#[test]
fn fenwick_set_height_updates_prefix_sums() {
    let mut heights: Vec<u32> = vec![20, 30, 40, 50, 60, 70, 80];
    let mut tree = FenwickTree::from_heights(&heights);

    let updates = [(2usize, 10u32), (0, 5), (6, 120), (3, 1), (2, 99)];
    for (idx, new_h) in updates {
        heights[idx] = new_h;
        tree.set_height(idx, new_h);
        let total: i64 = heights.iter().map(|&h| h as i64).sum();
        assert_eq!(tree.total_height(), total);
        for i in 0..=heights.len() {
            assert_eq!(tree.prefix_sum(i), naive_prefix_sum(&heights, i));
        }
    }
}

#[test]
fn fenwick_push_and_extend_match_naive() {
    let mut heights = vec![10u32, 20, 15];
    let mut tree = FenwickTree::from_heights(&heights);
    for h in [33u32, 7, 18, 42] {
        heights.push(h);
        tree.push(h);
    }
    let more = [5u32, 6, 7, 8, 9];
    heights.extend_from_slice(&more);
    tree.extend(more);
    assert_eq!(tree.len(), heights.len());
    for i in 0..=heights.len() {
        assert_eq!(tree.prefix_sum(i), naive_prefix_sum(&heights, i));
    }
}

#[test]
fn fenwick_prepend_shifts_items_and_preserves_totals() {
    let original: Vec<u32> = vec![20, 30, 40, 50];
    let prepend: Vec<u32> = vec![5, 6, 7];
    let mut tree = FenwickTree::from_heights(&original);
    tree.prepend(&prepend);

    let mut combined = prepend.clone();
    combined.extend_from_slice(&original);
    assert_eq!(tree.len(), combined.len());
    assert_eq!(tree.heights(), combined.as_slice());
    for i in 0..=combined.len() {
        assert_eq!(tree.prefix_sum(i), naive_prefix_sum(&combined, i));
    }
}

#[test]
fn binary_search_index_at_offset_matches_naive() {
    let heights: Vec<u32> = vec![10, 20, 5, 30, 15, 25, 40];
    let tree = FenwickTree::from_heights(&heights);
    let total = tree.total_height();
    for offset in -5..=total + 5 {
        assert_eq!(
            tree.index_at_offset(offset),
            naive_index_at_offset(&heights, offset),
            "offset {offset}"
        );
    }

    // Internal offsets inside items.
    let mut acc = 0i64;
    for (idx, &h) in heights.iter().enumerate() {
        for d in 0..h as i64 {
            assert_eq!(tree.index_at_offset(acc + d), Some(idx));
        }
        acc += h as i64;
    }
}

#[test]
fn viewport_range_includes_overscan_and_clamps() {
    let heights: Vec<u32> = (0..20).map(|i| 20 + (i % 5) * 10).collect();
    let win = VirtualWindow::from_heights(&heights, 2);
    let range = win.viewport_range(50, 100);
    assert!(range.visible_start <= range.visible_end);
    assert!(range.start <= range.visible_start);
    assert!(range.end >= range.visible_end);
    assert_eq!(range.start, range.visible_start.saturating_sub(2));
    assert_eq!(range.end, (range.visible_end + 2).min(win.len()));
    assert!(!range.is_empty());
    assert_eq!(range.as_range(), range.start..range.end);

    let top = win.viewport_range(0, 60);
    assert_eq!(top.start, 0);

    let bottom = win.viewport_range(win.total_height() + 999, 60);
    assert_eq!(bottom.visible_end, win.len());
    assert_eq!(bottom.end, win.len());
}

#[test]
fn viewport_range_empty_and_tiny_viewports() {
    let empty = VirtualWindow::new(3);
    let r = empty.viewport_range(0, 100);
    assert!(r.is_empty());
    assert_eq!(r.start, 0);
    assert_eq!(r.end, 0);

    let win = VirtualWindow::from_heights(&[30, 40, 50], 1);
    let zero_h = win.viewport_range(10, 0);
    assert!(zero_h.is_empty());
}

#[test]
fn anchor_preservation_after_prepend_keeps_visual_position() {
    let heights: Vec<u32> = (0..100).map(|i| 24 + (i % 7) * 6).collect();
    let mut win = VirtualWindow::from_heights(&heights, 3);
    let scroll_offset = 350i64;
    let anchor = win.capture_anchor(scroll_offset).expect("anchor");
    let visual_before = scroll_offset - win.offset_of(anchor.index);

    let prepend: Vec<u32> = vec![20, 22, 18, 30, 26];
    let added = win.prepend(&prepend);
    assert_eq!(added, prepend.len());

    let restored = win.restore_anchor(anchor, added);
    let new_index = anchor.index + added;
    let visual_after = restored - win.offset_of(new_index);
    assert_eq!(visual_before, visual_after);
    assert_eq!(
        win.capture_anchor(restored).unwrap().offset_within_item,
        anchor.offset_within_item
    );
}

#[test]
fn anchor_preservation_with_variable_height_prepend() {
    let heights: Vec<u32> = (0..50).map(|i| 30 + (i % 11) * 3).collect();
    let mut win = VirtualWindow::from_heights(&heights, 4);
    let scroll = 420i64;
    let anchor = win.capture_anchor(scroll).unwrap();
    let before_top = win.offset_of(anchor.index);
    let delta = scroll - before_top;

    let prepend: Vec<u32> = (0..13).map(|i| 15 + (i % 4) * 9).collect();
    let prepend_height: i64 = prepend.iter().map(|&h| h as i64).sum();
    win.prepend(&prepend);
    let restored = win.restore_anchor(anchor, prepend.len());

    assert_eq!(restored, before_top + prepend_height + delta);
    let new_anchor = win.capture_anchor(restored).unwrap();
    assert_eq!(new_anchor.index, anchor.index + prepend.len());
    assert_eq!(new_anchor.offset_within_item, anchor.offset_within_item);
}

#[test]
fn large_dataset_prefix_and_lookup_are_correct() {
    let n = 20_000usize;
    let heights: Vec<u32> = (0..n).map(|i| 16 + (i % 40) as u32).collect();
    let tree = FenwickTree::from_heights(&heights);
    let total: i64 = heights.iter().map(|&h| h as i64).sum();
    assert_eq!(tree.total_height(), total);

    for i in (0..=n).step_by(137) {
        assert_eq!(tree.prefix_sum(i), naive_prefix_sum(&heights, i));
    }
    for offset in (0..=total).step_by(311) {
        assert_eq!(tree.index_at_offset(offset), naive_index_at_offset(&heights, offset));
    }
}

#[test]
fn large_dataset_update_is_o_log_n_in_practice() {
    let n = 50_000usize;
    let heights: Vec<u32> = (0..n).map(|i| 20 + (i % 17) as u32).collect();
    let mut tree = FenwickTree::from_heights(&heights);

    let start = Instant::now();
    let mut idx = 0usize;
    for i in 0..20_000u32 {
        tree.set_height(idx, 20 + (i % 31));
        idx = (idx * 1664525 + 1013904223) % n;
    }
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() < 1500, "updates too slow: {elapsed:?}");

    // Spot-check correctness after updates.
    let mut naive = heights;
    let mut idx = 0usize;
    for i in 0..20_000u32 {
        naive[idx] = 20 + (i % 31);
        idx = (idx * 1664525 + 1013904223) % naive.len();
    }
    let total: i64 = naive.iter().map(|&h| h as i64).sum();
    assert_eq!(tree.total_height(), total);
    for i in (0..=n).step_by(997) {
        assert_eq!(tree.prefix_sum(i), naive_prefix_sum(&naive, i));
    }
}

#[test]
fn prepend_anchor_is_o_n_plus_m_not_catastrophic() {
    let base: Vec<u32> = (0..20_000).map(|i| 18 + (i % 23) as u32).collect();
    let mut win = VirtualWindow::from_heights(&base, 5);
    let anchor = win.capture_anchor(12_345).unwrap();
    let prepend: Vec<u32> = (0..2_000).map(|i| 16 + (i % 9) as u32).collect();

    let start = Instant::now();
    let added = win.prepend(&prepend);
    let restored = win.restore_anchor(anchor, added);
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() < 1500, "prepend too slow: {elapsed:?}");

    let mut combined = prepend;
    combined.extend_from_slice(&base);
    assert_eq!(win.len(), combined.len());
    assert_eq!(win.total_height(), naive_prefix_sum(&combined, combined.len()));
    let new_anchor = win.capture_anchor(restored).unwrap();
    assert_eq!(new_anchor.index, anchor.index + added);
}
