//! This process's memory as the kernel accounts it (S3 A2.2a, diagnostics).
//!
//! Read from `/proc/self/status` on Linux; `None` elsewhere, and on any field
//! the kernel does not report. Read-only: it changes nothing it measures.
//!
//! What the fields mean, and what they do not:
//!
//! - `rss`: resident pages, anonymous plus file-backed plus shared memory.
//! - `rss_anon`: resident anonymous pages, which hold the heap. It includes
//!   memory the allocator holds for reuse after it was freed, so it is not a
//!   count of live allocations.
//! - `rss_file`: resident file-backed pages (the binary, mapped files).
//! - `vm_size`: reserved address space, most of it never touched.

/// One reading. Every value is in KiB, as the kernel reports it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessMemory {
    /// Resident set size.
    pub rss: Option<u64>,
    /// Resident anonymous memory.
    pub rss_anon: Option<u64>,
    /// Resident file-backed memory.
    pub rss_file: Option<u64>,
    /// Virtual address space.
    pub vm_size: Option<u64>,
}

impl ProcessMemory {
    /// Reads this process's memory, or `None` where `/proc` is unavailable.
    #[must_use]
    pub fn read() -> Option<Self> {
        std::fs::read_to_string("/proc/self/status").ok().map(|text| Self::parse(&text))
    }

    /// Parses the text of a `/proc/<pid>/status` file.
    #[must_use]
    pub fn parse(status: &str) -> Self {
        let field = |name: &str| {
            status.lines().find_map(|line| {
                let rest = line.strip_prefix(name)?.strip_prefix(':')?;
                rest.split_whitespace().next()?.parse().ok()
            })
        };
        Self {
            rss: field("VmRSS"),
            rss_anon: field("RssAnon"),
            rss_file: field("RssFile"),
            vm_size: field("VmSize"),
        }
    }
}

impl std::fmt::Display for ProcessMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let show = |v: Option<u64>| v.map_or_else(|| "n/a".to_string(), |v| v.to_string());
        write!(
            f,
            "rss_kib={} rss_anon_kib={} rss_file_kib={} vm_size_kib={}",
            show(self.rss),
            show(self.rss_anon),
            show(self.rss_file),
            show(self.vm_size)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = "Name:\tmbongo-node\nVmPeak:\t  900000 kB\nVmSize:\t  824800 kB\n\
        VmHWM:\t  450000 kB\nVmRSS:\t  435600 kB\nRssAnon:\t  433000 kB\n\
        RssFile:\t    2600 kB\nRssShmem:\t       0 kB\nThreads:\t7\n";

    #[test]
    fn the_four_fields_are_read_in_kib() {
        assert_eq!(
            ProcessMemory::parse(STATUS),
            ProcessMemory {
                rss: Some(435_600),
                rss_anon: Some(433_000),
                rss_file: Some(2_600),
                vm_size: Some(824_800),
            }
        );
    }

    #[test]
    fn a_field_the_kernel_omits_is_none_and_shown_as_such() {
        let reading = ProcessMemory::parse("VmRSS:\t 10 kB\n");
        assert_eq!(reading.rss, Some(10));
        assert_eq!(reading.rss_anon, None);
        assert_eq!(
            reading.to_string(),
            "rss_kib=10 rss_anon_kib=n/a rss_file_kib=n/a vm_size_kib=n/a"
        );
    }

    #[test]
    fn a_similar_prefix_is_not_mistaken_for_a_field() {
        // `VmRSSx` must not be read as `VmRSS`.
        assert_eq!(ProcessMemory::parse("VmRSSx:\t 5 kB\n").rss, None);
    }
}
