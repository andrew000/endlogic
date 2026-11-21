//! The one sort the crate uses: a heapsort, small in code and without
//! allocation, for the short lists it sorts (cells of a shape, colors,
//! cells next to a growing piece). The standard sorts are much larger and
//! are compiled again for every element type.

use alloc::vec::Vec;

/// Sorts `items` in ascending order.
pub(crate) fn sort<T: Ord + Copy>(items: &mut [T]) {
    for start in (0..items.len() / 2).rev() {
        sift_down(items, start);
    }
    for end in (1..items.len()).rev() {
        items.swap(0, end);
        sift_down(&mut items[..end], 0);
    }
}

/// Sorts `items` and removes repeated values.
pub(crate) fn sort_dedup<T: Ord + Copy>(items: &mut Vec<T>) {
    sort(items);
    items.dedup();
}

/// Moves `items[root]` down until it is not less than its children.
fn sift_down<T: Ord + Copy>(items: &mut [T], mut root: usize) {
    loop {
        let mut child = 2 * root + 1;
        if child >= items.len() {
            return;
        }
        if child + 1 < items.len() && items[child] < items[child + 1] {
            child += 1;
        }
        if items[root] >= items[child] {
            return;
        }
        items.swap(root, child);
        root = child;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn sorts_like_the_standard_sort() {
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        for len in 0..40 {
            let mut items: Vec<u64> = (0..len)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state % 12
                })
                .collect();
            let mut expected = items.clone();
            expected.sort_unstable();
            sort(&mut items);
            assert_eq!(items, expected);
        }
        let mut pairs = vec![(2, 1), (0, 3), (2, 0), (0, 3), (1, 1)];
        sort_dedup(&mut pairs);
        assert_eq!(pairs, [(0, 3), (1, 1), (2, 0), (2, 1)]);
    }
}
