#[cfg(test)]
mod tests {
    use crate::top_entries::TopEntries;
    use crate::ByteSize;

    // Test Invariant 1: Largest entries always appear first
    #[test]
    fn test_descending_order_is_always_maintained() {
        let mut top = TopEntries::new(5);
        
        // Insert in random order
        top.insert("d".to_string(), 40);
        top.insert("a".to_string(), 100);
        top.insert("c".to_string(), 60);
        top.insert("e".to_string(), 20);
        top.insert("b".to_string(), 80);
        
        let entries = top.get_entries();
        
        // Verify strict descending order
        for i in 1..entries.len() {
            assert!(entries[i-1].1 >= entries[i].1, 
                "Entry at position {} ({}) should be larger than or equal to entry at position {} ({})",
                i-1, entries[i-1].1, i, entries[i].1);
        }
        
        // Verify exact ordering
        assert_eq!(entries[0].1, 100);
        assert_eq!(entries[1].1, 80);
        assert_eq!(entries[2].1, 60);
        assert_eq!(entries[3].1, 40);
        assert_eq!(entries[4].1, 20);
    }

    // Test Invariant 2: When capacity is exceeded, only largest entries remain
    #[test]
    fn test_capacity_enforcement_keeps_largest() {
        let mut top = TopEntries::new(3);
        
        // Insert more items than capacity
        top.insert("d".to_string(), 40);
        top.insert("a".to_string(), 100);
        top.insert("c".to_string(), 60);
        top.insert("e".to_string(), 20);  // Should be dropped
        top.insert("b".to_string(), 80);
        
        let entries = top.get_entries();
        
        // Verify only capacity number of items remain
        assert_eq!(entries.len(), 3, "Should only keep top 3 entries");
        
        // Verify they're the largest ones in descending order
        assert_eq!(entries[0].1, 100, "First entry should be largest (100)");
        assert_eq!(entries[1].1, 80, "Second entry should be second largest (80)");
        assert_eq!(entries[2].1, 60, "Third entry should be third largest (60)");
        
        // Verify smaller values were dropped
        assert!(!entries.iter().any(|(_, val)| *val == 40 || *val == 20),
            "Smaller values should have been dropped");
    }

    #[test]
    fn test_order_maintenance_with_updates() {
        let mut top = TopEntries::new(3);
        
        // Fill to capacity
        top.insert("a".to_string(), 100);
        top.insert("b".to_string(), 80);
        top.insert("c".to_string(), 60);
        
        // Insert larger value
        top.insert("d".to_string(), 90);
        
        let entries = top.get_entries();
        assert_eq!(entries[0].1, 100);
        assert_eq!(entries[1].1, 90);
        assert_eq!(entries[2].1, 80);
        
        // Insert smaller value (should be ignored)
        top.insert("e".to_string(), 70);
        
        let entries = top.get_entries();
        assert_eq!(entries[0].1, 100);
        assert_eq!(entries[1].1, 90);
        assert_eq!(entries[2].1, 80);
    }

    #[test]
    fn test_equal_values_maintain_order() {
        let mut top = TopEntries::new(4);
        
        top.insert("a".to_string(), 100);
        top.insert("b".to_string(), 100);
        top.insert("c".to_string(), 80);
        top.insert("d".to_string(), 80);
        
        let entries = top.get_entries();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].1, 100);
        assert_eq!(entries[1].1, 100);
        assert_eq!(entries[2].1, 80);
        assert_eq!(entries[3].1, 80);
    }

    #[test]
    fn test_capacity_edge_cases() {
        // Test with capacity 1
        let mut top = TopEntries::new(1);
        
        top.insert("a".to_string(), 50);
        top.insert("b".to_string(), 100);
        top.insert("c".to_string(), 75);
        
        let entries = top.get_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].1, 100, "Should keep only the largest value");
        
        // Test with repeated updates at capacity
        let mut top = TopEntries::new(2);
        
        top.insert("a".to_string(), 100);
        top.insert("b".to_string(), 80);
        top.insert("c".to_string(), 90);  // Should replace 80
        top.insert("d".to_string(), 85);  // Should replace nothing
        
        let entries = top.get_entries();
        assert_eq!(entries[0].1, 100);
        assert_eq!(entries[1].1, 90);
    }

    #[test]
    fn test_large_volume_maintains_invariants() {
        let mut top = TopEntries::new(5);
        
        // Insert 1000 items in reverse order
        for i in (0..1000).rev() {
            top.insert(format!("item_{}", i), i as u64);
        }
        
        let entries = top.get_entries();
        
        // Verify top 5 were retained 
        assert_eq!(entries.len(), 5);
        
        // Verify they're the largest in descending order
        for i in 0..5 {
            assert_eq!(entries[i].1, 999 - i as u64);
        }
        
        top.insert("new1".to_string(), 999);  // Should be first (equal to existing max)
        top.insert("new2".to_string(), 994);  // Should be last (smaller than existing values)
        
        let entries = top.get_entries();
        
        // Verify the values are still in descending order
        for i in 1..entries.len() {
            assert!(entries[i-1].1 >= entries[i].1, 
                "Values should be in descending order: {} should be >= {}", 
                entries[i-1].1, entries[i].1);
        }
        
        // Verify 5 entries 
        assert_eq!(entries.len(), 5);
        
        // Verify the range of values is correct (between 999 and 995)
        assert!(entries.iter().all(|(_, val)| *val >= 995 && *val <= 999),
            "All values should be between 995 and 999");
    }
    
    #[test]
    fn test_bytes_format() {
        assert_eq!(0_u64.format_size(), "0 bytes");
        assert_eq!(1_u64.format_size(), "1 bytes");
        assert_eq!(515_u64.format_size(), "515 bytes");
        assert_eq!(1023_u64.format_size(), "1023 bytes");
    }

    #[test]
    fn test_kilobytes_format() {
        // Exactly 1 KB
        assert_eq!((1024_u64).format_size(), "1.00 KB");
        
        // 1.5 KB
        assert_eq!((1024_u64 + 512).format_size(), "1.50 KB");
        
        // Almost 2 KB
        assert_eq!((2047_u64).format_size(), "2.00 KB");
        
        // Just under 1 MB
        assert_eq!((1024_u64 * 1024 - 1).format_size(), "1024.00 KB");
    }

    #[test]
    fn test_megabytes_format() {
        // Exactly 1 MB
        assert_eq!((1024_u64 * 1024).format_size(), "1.00 MB");
        
        // 1.5 MB
        assert_eq!((1024_u64 * 1024 + 1024 * 512).format_size(), "1.50 MB");
        
        // Almost 2 MB
        assert_eq!((2_u64 * 1024 * 1024 - 1).format_size(), "2.00 MB");
        
        // Just under 1 GB
        assert_eq!((1024_u64 * 1024 * 1024 - 1).format_size(), "1024.00 MB");
    }

    #[test]
    fn test_gigabytes_format() {
        // Exactly 1 GB
        assert_eq!((1024_u64 * 1024 * 1024).format_size(), "1.00 GB");
        
        // 1.5 GB
        assert_eq!((1024_u64 * 1024 * 1024 + 1024 * 1024 * 512).format_size(), "1.50 GB");
        
        // Almost 2 GB
        assert_eq!((2_u64 * 1024 * 1024 * 1024 - 1).format_size(), "2.00 GB");
        
        // Just under 1 TB
        assert_eq!((1024_u64 * 1024 * 1024 * 1024 - 1).format_size(), "1024.00 GB");
    }

    #[test]
    fn test_terabytes_format() {
        // Exactly 1 TB
        assert_eq!((1024_u64 * 1024 * 1024 * 1024).format_size(), "1.00 TB");
        
        // 1.5 TB
        assert_eq!(
            (1024_u64 * 1024 * 1024 * 1024 + 1024 * 1024 * 1024 * 512).format_size(),
            "1.50 TB"
        );
        
        // Test a large value
        assert_eq!(
            (15_u64 * 1024 * 1024 * 1024 * 1024).format_size(),
            "15.00 TB"
        );
    }

    #[test]
    fn test_boundary_values() {
        let kb = 1024_u64;
        let mb = kb * 1024;
        let gb = mb * 1024;
        let tb = gb * 1024;
        
        // Test values right at boundaries
        assert_eq!((kb - 1).format_size(), "1023 bytes");
        assert_eq!(kb.format_size(), "1.00 KB");
        
        assert_eq!((mb - 1).format_size(), "1024.00 KB");
        assert_eq!(mb.format_size(), "1.00 MB");
        
        assert_eq!((gb - 1).format_size(), "1024.00 MB");
        assert_eq!(gb.format_size(), "1.00 GB");
        
        assert_eq!((tb - 1).format_size(), "1024.00 GB");
        assert_eq!(tb.format_size(), "1.00 TB");
    }

    #[test]
    fn test_precise_decimal_formatting() {
        // Test that we get exactly 2 decimal places
        let size = 1024_u64 + 1; // 1 KB + 1 byte = 1.000976563... KB
        assert_eq!(size.format_size(), "1.00 KB");
        
        let size = 1024_u64 + 512; // 1.5 KB exactly
        assert_eq!(size.format_size(), "1.50 KB");
        
        // Test rounding
        let size = (1024_u64 * 1024) + 1024 * 51; // About 1.0498... MB
        assert_eq!(size.format_size(), "1.05 MB");
    }

    #[test]
    fn test_zero_and_small_values() {
        assert_eq!(0_u64.format_size(), "0 bytes");
        assert_eq!(1_u64.format_size(), "1 bytes");
        assert_eq!(10_u64.format_size(), "10 bytes");
    }
}