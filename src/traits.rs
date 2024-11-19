/// Provides functionality to format numeric sizes into human-readable strings with appropriate units.
/// 
/// This trait is particularly useful for displaying file sizes, memory usage, or any other
/// byte-based measurements in a user-friendly format. The output automatically scales from
/// bytes to terabytes based on the size of the number.
/// 
/// # Examples
/// 
/// ```
/// use ferris_files::traits::ByteSize;
/// let size: u64 = 1024;
/// assert_eq!(size.format_size(), "1.00 KB");
/// 
/// let large_size: u64 = 1024 * 1024 * 1024;
/// assert_eq!(large_size.format_size(), "1.00 GB");
/// ```
pub trait ByteSize {
    /// Formats the number into a human-readable string with appropriate size units.
    /// 
    /// The output will use one of the following units based on the size:
    /// - bytes (0 B to 1023 B)
    /// - kilobytes (1.00 KB to 1023.99 KB)
    /// - megabytes (1.00 MB to 1023.99 MB)
    /// - gigabytes (1.00 GB to 1023.99 GB)
    /// - terabytes (1.00 TB and above)
    /// 
    /// Values are formatted with two decimal places for KB and above,
    /// and no decimal places for bytes.
    /// 
    /// # Returns
    /// 
    /// A `String` containing the formatted size with appropriate units.
    /// 
    /// # Examples
    /// 
    /// ```
    /// use ferris_files::traits::ByteSize;
    /// 
    /// // Bytes
    /// assert_eq!(50_u64.format_size(), "50 bytes");
    /// 
    /// // Kilobytes
    /// assert_eq!((1024_u64).format_size(), "1.00 KB");
    /// assert_eq!((1536_u64).format_size(), "1.50 KB");
    /// 
    /// // Megabytes
    /// assert_eq!((1024 * 1024_u64).format_size(), "1.00 MB");
    /// 
    /// // Gigabytes
    /// assert_eq!((1024 * 1024 * 1024_u64).format_size(), "1.00 GB");
    /// 
    /// // Terabytes
    /// assert_eq!((1024 * 1024 * 1024 * 1024_u64).format_size(), "1.00 TB");
    /// ```
    fn format_size(&self) -> String;
}

impl ByteSize for u64 {
    /// Formats a u64 number as a human-readable size string.
    /// 
    /// Uses binary prefixes (1024 bytes = 1 KB) and automatically selects
    /// the most appropriate unit based on the size of the number.
    /// 
    /// # Examples
    /// 
    /// ```
    /// use ferris_files::traits::ByteSize;
    /// let bytes = 1024 * 1024 + 1024 * 512_u64; // 1.5 MB
    /// assert_eq!(bytes.format_size(), "1.50 MB");
    /// ```
    fn format_size(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        const TB: u64 = GB * 1024;

        match self {
            bytes if *bytes >= TB => format!("{:.2} TB", *bytes as f64 / TB as f64),
            bytes if *bytes >= GB => format!("{:.2} GB", *bytes as f64 / GB as f64),
            bytes if *bytes >= MB => format!("{:.2} MB", *bytes as f64 / MB as f64),
            bytes if *bytes >= KB => format!("{:.2} KB", *bytes as f64 / KB as f64),
            bytes => format!("{} bytes", bytes),
        }
    }
}