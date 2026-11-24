//! Variable-height message viewport virtualization.
//!
//! Provides a Fenwick/prefix-sum index for O(log N) height queries and updates,
//! binary search for offset-to-index lookup, and anchor preservation across
//! prepends so chat scroll position remains stable when older messages load.

/// Fenwick tree (Binary Indexed Tree) over item heights.
///
/// Supports point updates and prefix sums in O(log N). Total height is cached
/// for O(1) access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenwickTree {
    tree: Vec<i64>,
    heights: Vec<u32>,
    total: i64,
}

impl FenwickTree {
    /// Creates an empty Fenwick tree.
    pub fn new() -> Self {
        Self {
            tree: Vec::new(),
            heights: Vec::new(),
            total: 0,
        }
    }

    /// Builds a Fenwick tree from item heights in O(N).
    pub fn from_heights(heights: &[u32]) -> Self {
        let n = heights.len();
        let mut tree = vec![0i64; n + 1];
        let mut total = 0i64;
        for (i, &h) in heights.iter().enumerate() {
            let h = h as i64;
            total += h;
            let idx = i + 1;
            tree[idx] += h;
            let parent = idx + lsb(idx);
            if parent <= n {
                tree[parent] += tree[idx];
            }
        }
        Self {
            tree,
            heights: heights.to_vec(),
            total,
        }
    }

    /// Number of indexed items.
    #[inline]
    pub fn len(&self) -> usize {
        self.heights.len()
    }

    /// Returns true when there are no items.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.heights.is_empty()
    }

    /// Total height of all items in O(1).
    #[inline]
    pub fn total_height(&self) -> i64 {
        self.total
    }

    /// Height of item at `index`.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    #[inline]
    pub fn height(&self, index: usize) -> u32 {
        self.heights[index]
    }

    /// Returns item height, or None if out of bounds.
    #[inline]
    pub fn get_height(&self, index: usize) -> Option<u32> {
        self.heights.get(index).copied()
    }

    /// Prefix sum of heights for items `[0, index)` in O(log N).
    /// Returns 0 for index 0.
    pub fn prefix_sum(&self, index: usize) -> i64 {
        let mut i = index.min(self.heights.len());
        let mut sum = 0i64;
        while i > 0 {
            sum += self.tree[i];
            i -= lsb(i);
        }
        sum
    }

    /// Top offset of item at `index` in O(log N).
    ///
    /// # Panics
    /// Panics if `index > len()`.
    pub fn offset_of(&self, index: usize) -> i64 {
        assert!(index <= self.heights.len(), "index out of bounds");
        self.prefix_sum(index)
    }

    /// Sets the height of item at `index` in O(log N).
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    pub fn set_height(&mut self, index: usize, height: u32) {
        assert!(index < self.heights.len(), "index out of bounds");
        let old = self.heights[index] as i64;
        let new = height as i64;
        let delta = new - old;
        if delta == 0 {
            return;
        }
        self.heights[index] = height;
        self.total += delta;
        let mut i = index + 1;
        let n = self.heights.len();
        while i <= n {
            self.tree[i] += delta;
            i += lsb(i);
        }
    }

    /// Appends an item height in O(log N).
    pub fn push(&mut self, height: u32) {
        let n = self.heights.len() + 1;
        self.heights.push(height);
        self.tree.push(0);
        let h = height as i64;
        self.total += h;
        // Initialize tree[n] as sum of its range.
        self.tree[n] = h;
        let lower = n - lsb(n);
        let mut i = n - 1;
        while i > lower {
            self.tree[n] += self.tree[i];
            i -= lsb(i);
        }
        let parent = n + lsb(n);
        // parent is > n, so no propagation needed for append; future appends/updates handle it.
        let _ = parent;
    }

    /// Extends with multiple heights in O(M log N) where M is appended count.
    pub fn extend(&mut self, heights: impl IntoIterator<Item = u32>) {
        for h in heights {
            self.push(h);
        }
    }

    /// Prepends heights to the front in O(N + M).
    ///
    /// Fenwick trees are indexed from the start, so prepend shifts all indices.
    /// We rebuild the tree structure over the new combined heights. This preserves
    /// correctness and gives O(N+M) for the batch prepend, which is optimal for
    /// chat history loads that prepend contiguous chunks.
    pub fn prepend(&mut self, heights: &[u32]) {
        if heights.is_empty() {
            return;
        }
        let mut combined = Vec::with_capacity(heights.len() + self.heights.len());
        combined.extend_from_slice(heights);
        combined.extend_from_slice(&self.heights);
        *self = Self::from_heights(&combined);
    }

    /// Finds the item index containing `offset` using binary search over prefix sums.
    ///
    /// Returns `None` for empty trees. Offsets below 0 clamp to 0. Offsets at or
    /// beyond total height return the last index.
    ///
    /// Complexity: O(log^2 N) via binary search + Fenwick prefix sums.
    pub fn index_at_offset(&self, offset: i64) -> Option<usize> {
        let n = self.len();
        if n == 0 {
            return None;
        }
        if offset <= 0 {
            return Some(0);
        }
        let total = self.total_height();
        if offset >= total {
            return Some(n - 1);
        }

        // Binary search: first index whose end offset > offset.
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let end = self.prefix_sum(mid + 1);
            if end <= offset {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        Some(lo.min(n - 1))
    }

    /// Returns a snapshot of heights.
    pub fn heights(&self) -> &[u32] {
        &self.heights
    }
}

impl Default for FenwickTree {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn lsb(i: usize) -> usize {
    i & i.wrapping_neg()
}

/// Visible item range in the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportRange {
    /// First visible item index, including overscan.
    pub start: usize,
    /// Exclusive end index, including overscan.
    pub end: usize,
    /// First visible item index without overscan.
    pub visible_start: usize,
    /// Exclusive end index without overscan.
    pub visible_end: usize,
    /// Scroll offset used for this computation.
    pub scroll_offset: i64,
    /// Viewport height used for this computation.
    pub viewport_height: i64,
    /// Total content height.
    pub total_height: i64,
}

impl ViewportRange {
    /// Number of items in the overscanned range.
    #[inline]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns true when the range contains no items.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Returns the overscanned range as a Rust range.
    #[inline]
    pub fn as_range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

/// Anchor captured before mutations that shift indices/heights, such as prepend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollAnchor {
    /// Item index at capture time.
    pub index: usize,
    /// Pixel offset from the top of the anchored item to the viewport top.
    pub offset_within_item: i64,
    /// Scroll offset at capture time.
    pub scroll_offset: i64,
}

/// Variable-height virtual window over a Fenwick prefix-sum index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualWindow {
    fenwick: FenwickTree,
    overscan_items: usize,
}

impl VirtualWindow {
    /// Creates an empty virtual window with the given overscan item count.
    pub fn new(overscan_items: usize) -> Self {
        Self {
            fenwick: FenwickTree::new(),
            overscan_items,
        }
    }

    /// Builds a virtual window from heights.
    pub fn from_heights(heights: &[u32], overscan_items: usize) -> Self {
        Self {
            fenwick: FenwickTree::from_heights(heights),
            overscan_items,
        }
    }

    /// Returns the overscan item count.
    #[inline]
    pub fn overscan_items(&self) -> usize {
        self.overscan_items
    }

    /// Sets overscan item count.
    #[inline]
    pub fn set_overscan_items(&mut self, overscan_items: usize) {
        self.overscan_items = overscan_items;
    }

    /// Number of items.
    #[inline]
    pub fn len(&self) -> usize {
        self.fenwick.len()
    }

    /// Returns true when there are no items.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.fenwick.is_empty()
    }

    /// Total content height in O(1).
    #[inline]
    pub fn total_height(&self) -> i64 {
        self.fenwick.total_height()
    }

    /// Height of item at index.
    #[inline]
    pub fn height(&self, index: usize) -> u32 {
        self.fenwick.height(index)
    }

    /// Top offset of item at index in O(log N).
    #[inline]
    pub fn offset_of(&self, index: usize) -> i64 {
        self.fenwick.offset_of(index)
    }

    /// Prefix sum of heights for `[0, index)` in O(log N).
    #[inline]
    pub fn prefix_sum(&self, index: usize) -> i64 {
        self.fenwick.prefix_sum(index)
    }

    /// Sets item height in O(log N).
    pub fn set_height(&mut self, index: usize, height: u32) {
        self.fenwick.set_height(index, height);
    }

    /// Appends an item height in O(log N).
    pub fn push(&mut self, height: u32) {
        self.fenwick.push(height);
    }

    /// Extends with heights.
    pub fn extend(&mut self, heights: impl IntoIterator<Item = u32>) {
        self.fenwick.extend(heights);
    }

    /// Prepends heights and returns how many items were added.
    ///
    /// Use with [`Self::capture_anchor`] / [`Self::restore_anchor`] to preserve
    /// visual scroll position.
    pub fn prepend(&mut self, heights: &[u32]) -> usize {
        self.fenwick.prepend(heights);
        heights.len()
    }

    /// Finds item index at scroll offset via binary search in O(log^2 N).
    pub fn index_at_offset(&self, offset: i64) -> Option<usize> {
        self.fenwick.index_at_offset(offset)
    }

    /// Captures a scroll anchor for the current scroll offset.
    ///
    /// The anchor stores the topmost visible item and the pixel delta from that
    /// item's top to the scroll offset. After prepending, pass the anchor to
    /// [`Self::restore_anchor`] with the prepend count.
    pub fn capture_anchor(&self, scroll_offset: i64) -> Option<ScrollAnchor> {
        let index = self.index_at_offset(scroll_offset)?;
        let item_top = self.offset_of(index);
        Some(ScrollAnchor {
            index,
            offset_within_item: scroll_offset - item_top,
            scroll_offset,
        })
    }

    /// Restores scroll offset after `prepended_count` items were inserted at front.
    ///
    /// Returns the new scroll offset that keeps the anchored item at the same
    /// visual position. If the anchor is invalid, returns the original anchor scroll offset.
    pub fn restore_anchor(&self, anchor: ScrollAnchor, prepended_count: usize) -> i64 {
        let new_index = anchor.index.saturating_add(prepended_count);
        if new_index > self.len() {
            return anchor.scroll_offset;
        }
        let new_item_top = if new_index == self.len() {
            self.total_height()
        } else {
            self.offset_of(new_index)
        };
        let restored = new_item_top + anchor.offset_within_item;
        restored.clamp(0, self.max_scroll_offset(0))
    }

    /// Maximum scroll offset for a viewport height. `viewport_height` of 0 returns total height.
    pub fn max_scroll_offset(&self, viewport_height: i64) -> i64 {
        (self.total_height() - viewport_height.max(0)).max(0)
    }

    /// Computes the visible range for a viewport, including overscan.
    pub fn viewport_range(&self, scroll_offset: i64, viewport_height: i64) -> ViewportRange {
        let n = self.len();
        let total_height = self.total_height();
        if n == 0 || viewport_height <= 0 {
            return ViewportRange {
                start: 0,
                end: 0,
                visible_start: 0,
                visible_end: 0,
                scroll_offset: scroll_offset.clamp(0, 0),
                viewport_height: viewport_height.max(0),
                total_height,
            };
        }

        let max_scroll = self.max_scroll_offset(viewport_height);
        let clamped_scroll = scroll_offset.clamp(0, max_scroll);
        let bottom = clamped_scroll.saturating_add(viewport_height).min(total_height);

        let visible_start = self.index_at_offset(clamped_scroll).unwrap_or(0);
        let visible_end = self
            .index_at_offset(bottom.saturating_sub(1))
            .map(|i| (i + 1).min(n))
            .unwrap_or(visible_start);

        let start = visible_start.saturating_sub(self.overscan_items);
        let end = (visible_end + self.overscan_items).min(n);

        ViewportRange {
            start,
            end,
            visible_start,
            visible_end: visible_end.max(visible_start),
            scroll_offset: clamped_scroll,
            viewport_height,
            total_height,
        }
    }

    /// Returns heights slice.
    #[inline]
    pub fn heights(&self) -> &[u32] {
        self.fenwick.heights()
    }

    /// Returns a reference to the underlying Fenwick tree.
    #[inline]
    pub fn fenwick(&self) -> &FenwickTree {
        &self.fenwick
    }
}

impl Default for VirtualWindow {
    fn default() -> Self {
        Self::new(0)
    }
}
