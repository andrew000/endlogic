//! The one sort of the crate: a stable insertion sort, small in code and
//! fast enough for the few hundred items it orders.

/// Sorts numbers ascending.
pub(crate) fn sort(values: &mut [f32]) {
    sort_by_key(values, |v| *v);
}

/// Sorts by a key, ascending; equal keys keep their order.
pub(crate) fn sort_by_key<T>(items: &mut [T], key: impl Fn(&T) -> f32) {
    for i in 1..items.len() {
        let mut j = i;
        while j > 0 && key(&items[j - 1]) > key(&items[j]) {
            items.swap(j - 1, j);
            j -= 1;
        }
    }
}
