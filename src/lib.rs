use std::{
    borrow::Borrow, cell::RefCell, collections::HashSet, fmt::Debug, hash::Hash, num::NonZeroU32,
    ops::Deref, ptr, rc::Rc,
};

use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};

#[derive(Debug)]
struct Node<K, V> {
    key: K,
    value: V,
    link: LinkedListLink,
}

intrusive_adapter!(NodeAdapter<K, V> = Rc<Node<K, V>>: Node<K, V> { link: LinkedListLink });

#[derive(Debug)]
struct RefNode<K, V> {
    ref_count: Rc<Node<K, V>>,
}

impl<K, V> RefNode<K, V> {
    fn new(key: K, value: V) -> Self {
        Self {
            ref_count: Rc::new(Node {
                key,
                value,
                link: LinkedListLink::new(),
            }),
        }
    }

    fn key(&self) -> &K {
        &self.ref_count.key
    }

    fn value(&self) -> &V {
        &self.ref_count.value
    }

    fn into_pair(self) -> (K, V)
    where
        K: Debug,
        V: Debug,
    {
        assert_eq!(Rc::strong_count(&self.ref_count), 1);
        let Node { key, value, .. } = Rc::try_unwrap(self.ref_count).unwrap();
        (key, value)
    }

    fn strong_ref_count(&self) -> usize {
        Rc::strong_count(&self.ref_count)
    }
}

impl<K: Hash, V> Hash for RefNode<K, V> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

impl<K: PartialEq, V> PartialEq for RefNode<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl<K: Eq, V> Eq for RefNode<K, V> {}

impl<K, V> Borrow<K> for RefNode<K, V> {
    fn borrow(&self) -> &K {
        self.key()
    }
}

impl<K, V> Clone for RefNode<K, V> {
    fn clone(&self) -> Self {
        Self {
            ref_count: self.ref_count.clone(),
        }
    }
}

#[derive(Debug)]
pub struct LRUCache<K, V> {
    kv_storage: HashSet<RefNode<K, V>>,
    recency_queue: RefCell<LinkedList<NodeAdapter<K, V>>>,
    max_len: NonZeroU32,
}

impl<K, V> Default for LRUCache<K, V> {
    fn default() -> Self {
        let max_len = NonZeroU32::new(1);
        assert!(max_len.is_some());
        let max_size = max_len.unwrap();
        Self {
            kv_storage: Default::default(),
            recency_queue: Default::default(),
            max_len: max_size,
        }
    }
}

impl<K, V> LRUCache<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create cache with maximum of `max_size` elements.
    ///
    /// Allocates capacity beforehand.
    pub fn with_max_len(max_len: NonZeroU32) -> Self {
        let capacity = max_len.get() as usize;
        let kv_storage = HashSet::with_capacity(capacity);
        let recency_queue = RefCell::new(LinkedList::new(NodeAdapter::new()));
        Self {
            kv_storage,
            recency_queue,
            max_len,
        }
    }

    /// Adds an element to the queue.
    ///
    /// If the `key` is new, returns [None] and adds it to cache.
    /// If `len()` exceeds `max_size()`, the least recently accessed key is removed.
    ///
    /// If the `key` was present, returns previous key-value pair,
    /// the key's considered the last used one.
    pub fn insert(&mut self, key: K, val: V) -> Option<(K, V)>
    where
        K: Hash + Eq + Debug,
        V: Debug,
    {
        assert!(self.len() <= self.max_len());

        let removed_val = self.drop_before_insertion(&key);
        self.push_entry(key, val);

        assert!(self.len() <= self.max_len());

        if let Some(removed_val) = removed_val.as_ref() {
            assert_eq!(removed_val.strong_ref_count(), 1);
        }

        removed_val.map(|key_val| key_val.into_pair())
    }

    /// Retrieves a value associated with `key`.
    /// The key is considered most-recently used afterwards
    pub fn get(&self, key: &K) -> Option<&V>
    where
        K: Hash + Eq,
    {
        self.kv_storage.get(key).map(|entry| {
            self.drop_from_queue(entry);
            let mut borrowed_queue = self.recency_queue.borrow_mut();
            borrowed_queue.push_back(entry.ref_count.clone());
            entry.value()
        })
    }

    pub fn max_len(&self) -> usize {
        let as_usize = self.max_len.get() as usize;
        assert!(self.len() <= as_usize);
        as_usize
    }

    pub fn len(&self) -> usize {
        self.kv_storage.len()
    }

    pub fn resize(&mut self, new_max_len: NonZeroU32) -> Vec<(K, V)>
    where
        K: Hash + Eq + Debug,
        V: Debug,
    {
        if new_max_len >= self.max_len {
            self.kv_storage.reserve(new_max_len.get() as usize - self.max_len());
            self.max_len = new_max_len;
            return Vec::new();
        }

        if self.kv_storage.is_empty() {
            return Vec::new()
        }

        let mut borrowed_queue = self.recency_queue.borrow_mut();

        let mut all_removed = Vec::new();
        for _ in new_max_len.get() as usize..self.len() {
            let removed = borrowed_queue.pop_front();
            assert!(removed.is_some());
            let removed = RefNode { ref_count: removed.unwrap() };

            let was_removed = self.kv_storage.remove(removed.key());
            assert!(was_removed);
            assert_eq!(removed.strong_ref_count(), 1);

            all_removed.push(removed.into_pair());
        }
        self.max_len = new_max_len;
        all_removed
    }

    /// Iterate over elements in an unspecified order.
    /// Does not affect order of elements removal.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)>
    where
        K: Hash + Eq
    {
        self.kv_storage.iter()
            .map(|elem| (elem.key(), elem.value()))
    }

    /// If key is present in storage, remove it from queue and storage and return removed node.
    ///
    /// If `len()` equals to `max_size()`, drop the first value from queue and storage and return [None].
    ///
    /// Just return [None] otherwise.
    ///
    /// Cache has a place to insert new entry.after call
    fn drop_before_insertion(&mut self, key: &K) -> Option<RefNode<K, V>>
    where
        K: Hash + Eq,
    {
        enum DropReason {
            HasCollision,
            FirstInQueue,
        }

        let init_len = self.len();
        let (to_remove, reason) = match self.kv_storage.get(key) {
            Some(to_remove) => {
                self.drop_from_queue(to_remove);
                (to_remove.clone(), DropReason::HasCollision)
            }
            None if self.len() == self.max_len() => {
                let to_remove = self.recency_queue.borrow_mut().front_mut().remove();
                // since [max_size] is not less than 1, there is at least one element in the queue,
                //   thus, we've removed something
                assert!(to_remove.is_some());
                (
                    RefNode {
                        ref_count: to_remove.unwrap(),
                    },
                    DropReason::FirstInQueue,
                )
            }
            None => {
                assert!(self.len() < self.max_len());
                return None;
            }
        };

        let was_removed = self.kv_storage.remove(to_remove.key());

        assert!(was_removed);

        assert_eq!(init_len - 1, self.len());
        assert!(self.len() < self.max_len());

        match reason {
            DropReason::HasCollision => Some(to_remove),
            DropReason::FirstInQueue => None,
        }
    }

    fn drop_from_queue(&self, entry: &RefNode<K, V>) {
        assert!(entry.ref_count.link.is_linked());
        assert_eq!(entry.strong_ref_count(), 2);
        {
            let mut borrowed_queue = self.recency_queue.borrow_mut();
            let mut entry_cursor =
                unsafe { borrowed_queue.cursor_mut_from_ptr(entry.ref_count.deref()) };
            entry_cursor.remove();
        }
        assert_eq!(entry.strong_ref_count(), 1);
    }

    /// Requires Cache to have free space for insertion
    /// Puts new key-value pair, pushes `key` to the end of the queue
    fn push_entry(&mut self, key: K, val: V)
    where
        K: Hash + Eq,
    {
        assert!(self.len() < self.max_len());

        let entry = RefNode::new(key, val);
        assert_eq!(entry.strong_ref_count(), 1);

        self.kv_storage.insert(entry.clone());
        self.recency_queue.borrow_mut().push_back(entry.ref_count);

        assert!(self.len() <= self.max_len());

        let borrowed_queue = self.recency_queue.borrow();
        let pushed_to_queue = borrowed_queue.back().get();
        assert!(pushed_to_queue.is_some());

        let pushed_to_stg = self.kv_storage.get(&pushed_to_queue.unwrap().key);
        assert!(pushed_to_stg.is_some());

        assert_eq!(pushed_to_stg.unwrap().strong_ref_count(), 2);
        assert!(ptr::eq(
            pushed_to_queue.unwrap(),
            pushed_to_stg.unwrap().ref_count.deref()
        ));
    }
}
