/// A data structure that maintains a fixed-size collection of entries sorted by numeric value in descending order.
///
/// `TopEntries` keeps track of the `max_entries` largest values it has seen, along with associated filepath.
/// When a new entry is inserted, it is automatically placed in the correct position to maintain the descending order,
/// and if the collection exceeds its capacity, the smallest value is dropped.
///
/// # Examples
///
/// ```
/// # use ferris_files::top_entries::TopEntries;
/// let mut top = TopEntries::new(2);
///
/// // Insert some entries
/// top.insert("file_a.txt".to_string(), 100);
/// top.insert("file_b.txt".to_string(), 200);
/// top.insert("file_c.txt".to_string(), 50);  // This will be dropped as it's the smallest
///
/// let entries = top.get_entries();
/// assert_eq!(entries.len(), 2);
/// assert_eq!(entries[0].1, 200);  // Largest value first
/// assert_eq!(entries[1].1, 100);  // Second largest value
/// ```
#[derive(Debug)]
pub struct TopEntries {
    pub entries: Vec<(String, u64)>,
    pub max_entries: usize,
}

impl TopEntries {
    /// Creates a new `TopEntries` instance.
    ///
    /// The internal vector is pre-allocated with capacity `max_entries + 1` to optimize
    /// for the case where we temporarily need to hold an extra entry before dropping the smallest one.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ferris_files::top_entries::TopEntries;
    /// let top = TopEntries::new(3);
    /// assert_eq!(top.get_entries().len(), 0);
    /// ```
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries + 1),
            max_entries,
        }
    }

    /// Inserts a new entry into the collection, maintaining the descending order by size.
    ///
    /// If the new entry's size is larger than the smallest current entry (or if the collection
    /// isn't at capacity), the entry is inserted in the correct position to maintain descending order.
    /// If this causes the collection to exceed its capacity, the smallest entry is dropped.
    ///
    /// # Arguments
    ///
    /// * `path` - A String identifier for the entry
    /// * `size` - The numeric value associated with the entry
    ///
    /// # Examples
    ///
    /// ```
    /// # use ferris_files::top_entries::TopEntries;
    /// let mut top = TopEntries::new(2);
    ///
    /// // Insert entries in arbitrary order
    /// top.insert("medium".to_string(), 50);
    /// top.insert("largest".to_string(), 100);
    /// top.insert("smallest".to_string(), 25);  // This will be dropped
    ///
    /// let entries = top.get_entries();
    /// assert_eq!(entries[0], ("largest".to_string(), 100));
    /// assert_eq!(entries[1], ("medium".to_string(), 50));
    /// ```
    ///
    /// # Notes
    ///
    /// * If the collection is at capacity and the new entry's size is smaller than or equal to
    ///   the smallest current entry, the new entry is not inserted at all.
    /// * The insertion uses binary search (`partition_point`) to efficiently find the correct
    ///   position while maintaining the descending order.
    pub fn insert(&mut self, path: String, size: u64) {
        if self.entries.len() < self.max_entries
            || size > self.entries.last().map(|(_, s)| *s).unwrap_or(0)
        {
            let idx = self.entries.partition_point(|(_, s)| *s > size);
            self.entries.insert(idx, (path, size));

            if self.entries.len() > self.max_entries {
                self.entries.pop();
            }
        }
    }

    /// Returns a reference to the slice containing all entries in descending order by size.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ferris_files::top_entries::TopEntries;
    /// let mut top = TopEntries::new(2);
    /// top.insert("a".to_string(), 100);
    /// top.insert("b".to_string(), 200);
    ///
    /// let entries = top.get_entries();
    /// assert_eq!(entries.len(), 2);
    /// assert!(entries[0].1 > entries[1].1);  // Verifies descending order
    /// ```
    #[allow(dead_code)]
    pub fn get_entries(&self) -> &[(String, u64)] {
        &self.entries
    }
}
