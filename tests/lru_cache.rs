use std::num::NonZeroU32;

use lru_cache::LRUCache;

#[test]
fn does_not_exceed_max_size() {
    let mut cache = LRUCache::with_max_len(NonZeroU32::new(2).unwrap());
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.max_len(), 2);

    // new key
    assert_eq!(cache.insert(1, 2), None);
    assert_eq!(cache.len(), 1);

    assert_eq!(cache.insert(1, 3), Some((1, 2)));
    assert_eq!(cache.len(), 1);

    assert_eq!(cache.insert(2, 4), None);
    assert_eq!(cache.len(), 2);

    assert_eq!(cache.insert(2, 4), Some((2, 4)));
    assert_eq!(cache.len(), 2);

    // new key, throws out key 1
    assert_eq!(cache.insert(3, 5), None);
    assert_eq!(cache.len(), 2);

    // moves 3 to unused position
    assert_eq!(cache.insert(2, 6), Some((2, 4)));
    assert_eq!(cache.len(), 2);

    // new key, throws out 3
    assert_eq!(cache.insert(1, 7), None);
    assert_eq!(cache.len(), 2);
}

#[test]
fn insert_get() {
    let mut cache = LRUCache::with_max_len(NonZeroU32::new(10).unwrap());
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.max_len(), 10);

    cache.insert("one", 1);
    assert_eq!(cache.len(), 1);

    cache.insert("two", 2);
    assert_eq!(cache.len(), 2);

    cache.insert("three", 3);
    assert_eq!(cache.len(), 3);

    assert_eq!(cache.get(&"one"), Some(&1));
    assert_eq!(cache.get(&"two"), Some(&2));
    assert_eq!(cache.get(&"three"), Some(&3));
    assert_eq!(cache.get(&"four"), None);
}

#[test]
fn renewal_by_get() {
    let mut cache = LRUCache::with_max_len(NonZeroU32::new(3).unwrap());

    cache.insert("one", 1);
    cache.insert("two", 2);
    cache.insert("three", 3);

    assert_eq!(cache.len(), cache.max_len());

    // the next insertion should delete 1
    cache.insert("four", 4);
    assert_eq!(cache.get(&"one"), None);

    // the next insertion would delete 2, but we'll get it first
    assert_eq!(cache.get(&"two"), Some(&2));

    cache.insert("five", 5);
    assert_eq!(cache.get(&"two"), Some(&2));
    assert_eq!(cache.get(&"five"), Some(&5));
    assert_eq!(cache.get(&"three"), None);
}

#[test]
fn resize_to_bigger() {
    let mut cache = LRUCache::with_max_len(NonZeroU32::new(3).unwrap());

    cache.insert("1 + 1", 2);
    cache.insert("2 * 3", 6);
    cache.insert("6 * 7", 42);

    // Next insertion removes "1 + 1"
    cache.insert("1 + 2", 3);
    assert_eq!(cache.get(&"1 + 2"), Some(&3));
    assert_eq!(cache.get(&"1 + 1"), None);

    cache.resize(NonZeroU32::new(4).unwrap());
    assert_eq!(cache.max_len(), 4);
    assert_eq!(cache.len(), 3);

    // Would've removed "3 * 3" without resize
    cache.insert("1 + 1", 2);
    assert_eq!(cache.get(&"1 + 1"), Some(&2));
    assert_eq!(cache.get(&"2 * 3"), Some(&6));

    // Next insert still throws out "6 * 7"
    cache.insert("7 * 6", 42);
    assert_eq!(cache.get(&"6 * 7"), None);
    assert_eq!(cache.get(&"7 * 6"), Some(&42));

    assert_eq!(cache.len(), 4);
}

#[test]
fn resize_to_smaller_without_shrink() {
    let mut cache = LRUCache::with_max_len(NonZeroU32::new(3).unwrap());

    cache.insert(1, 2);
    cache.insert(2, 3);

    assert_eq!(cache.len(), 2);
    assert_eq!(cache.max_len(), 3);

    cache.resize(NonZeroU32::new(2).unwrap());

    assert_eq!(cache.len(), 2);
    assert_eq!(cache.max_len(), 2);

    assert_eq!(cache.get(&1), Some(&2));
    assert_eq!(cache.get(&2), Some(&3));
}

#[test]
fn resize_and_shrink() {
    let mut cache = LRUCache::with_max_len(NonZeroU32::new(3).unwrap());

    cache.insert(1, 2);
    cache.insert(2, 3);
    cache.insert(4, 2);

    assert_eq!(cache.len(), 3);
    assert_eq!(cache.max_len(), 3);

    cache.resize(NonZeroU32::new(1).unwrap());

    assert_eq!(cache.max_len(), 1);
    assert_eq!(cache.len(), 1);

    assert_eq!(cache.get(&4), Some(&2));
    assert_eq!(cache.get(&2), None);
    assert_eq!(cache.get(&1), None);
}
