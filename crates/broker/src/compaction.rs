use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{SystemTime, Duration};

/// Represents a compacted log entry
#[derive(Debug, Clone)]
pub struct CompactedEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub offset: i64,
    pub timestamp: SystemTime,
}

/// Log segment for compaction
#[derive(Debug)]
pub struct LogSegment {
    pub start_offset: i64,
    pub end_offset: i64,
    pub entries: Vec<CompactedEntry>,
    pub size_bytes: usize,
}

impl LogSegment {
    pub fn new(start_offset: i64) -> Self {
        Self {
            start_offset,
            end_offset: start_offset,
            entries: Vec::new(),
            size_bytes: 0,
        }
    }

    pub fn add_entry(&mut self, entry: CompactedEntry) {
        self.size_bytes += entry.value.len();
        self.end_offset = entry.offset;
        self.entries.push(entry);
    }
}

/// Compaction strategy
#[derive(Debug, Clone, PartialEq)]
pub enum CompactionStrategy {
    /// Keep only the latest value for each key
    KeepLatest,
    /// Keep all values within a time window
    TimeWindow(Duration),
    /// Hybrid: keep latest + time window
    Hybrid(Duration),
}

/// Log Compaction Manager
pub struct LogCompactionManager {
    /// Segments per partition
    segments: Arc<RwLock<HashMap<i32, Vec<LogSegment>>>>,
    /// Compaction strategy
    strategy: CompactionStrategy,
    /// Minimum segment size before compaction (bytes)
    min_compaction_size: usize,
    /// Maximum segment size (bytes)
    max_segment_size: usize,
}

impl LogCompactionManager {
    pub fn new(strategy: CompactionStrategy) -> Self {
        Self {
            segments: Arc::new(RwLock::new(HashMap::new())),
            strategy,
            min_compaction_size: 1024 * 1024, // 1MB
            max_segment_size: 1024 * 1024 * 100, // 100MB
        }
    }

    /// Add an entry to the log
    pub async fn append(
        &self,
        partition_id: i32,
        key: String,
        value: Vec<u8>,
        offset: i64,
    ) -> Result<(), String> {
        let mut segments = self.segments.write().await;
        let partition_segments = segments.entry(partition_id).or_insert_with(Vec::new);

        // Get or create current segment
        if partition_segments.is_empty() {
            partition_segments.push(LogSegment::new(offset));
        }

        let current_segment = partition_segments.last_mut().unwrap();
        
        // Check if we need a new segment
        if current_segment.size_bytes >= self.max_segment_size {
            partition_segments.push(LogSegment::new(offset));
        }

        let entry = CompactedEntry {
            key,
            value,
            offset,
            timestamp: SystemTime::now(),
        };

        partition_segments.last_mut().unwrap().add_entry(entry);
        Ok(())
    }

    /// Compact a partition's log
    pub async fn compact_partition(&self, partition_id: i32) -> Result<usize, String> {
        let mut segments = self.segments.write().await;
        
        if let Some(partition_segments) = segments.get_mut(&partition_id) {
            let original_size: usize = partition_segments.iter().map(|s| s.size_bytes).sum();
            
            match self.strategy {
                CompactionStrategy::KeepLatest => {
                    self.compact_keep_latest(partition_segments);
                }
                CompactionStrategy::TimeWindow(duration) => {
                    self.compact_time_window(partition_segments, duration);
                }
                CompactionStrategy::Hybrid(duration) => {
                    self.compact_hybrid(partition_segments, duration);
                }
            }

            let new_size: usize = partition_segments.iter().map(|s| s.size_bytes).sum();
            Ok(original_size - new_size)
        } else {
            Err(format!("Partition {} not found", partition_id))
        }
    }

    /// Compact by keeping only latest value for each key
    fn compact_keep_latest(&self, segments: &mut Vec<LogSegment>) {
        let mut latest_entries: HashMap<String, CompactedEntry> = HashMap::new();

        // Collect all entries
        for segment in segments.iter() {
            for entry in &segment.entries {
                latest_entries
                    .entry(entry.key.clone())
                    .and_modify(|e| {
                        if entry.offset > e.offset {
                            *e = entry.clone();
                        }
                    })
                    .or_insert_with(|| entry.clone());
            }
        }

        // Rebuild segments with only latest entries
        segments.clear();
        if !latest_entries.is_empty() {
            let mut new_segment = LogSegment::new(0);
            for entry in latest_entries.into_values() {
                if new_segment.size_bytes >= self.max_segment_size {
                    segments.push(new_segment);
                    new_segment = LogSegment::new(entry.offset);
                }
                new_segment.add_entry(entry);
            }
            segments.push(new_segment);
        }
    }

    /// Compact by keeping entries within time window
    fn compact_time_window(&self, segments: &mut Vec<LogSegment>, window: Duration) {
        let cutoff_time = SystemTime::now() - window;
        
        for segment in segments.iter_mut() {
            segment.entries.retain(|entry| entry.timestamp >= cutoff_time);
            segment.size_bytes = segment.entries.iter().map(|e| e.value.len()).sum();
        }

        // Remove empty segments
        segments.retain(|s| !s.entries.is_empty());
    }

    /// Hybrid: keep latest + time window
    fn compact_hybrid(&self, segments: &mut Vec<LogSegment>, window: Duration) {
        // First apply time window
        self.compact_time_window(segments, window);
        
        // Then keep only latest for each key within the window
        self.compact_keep_latest(segments);
    }

    /// Get compaction stats
    pub async fn get_stats(&self, partition_id: i32) -> Option<(usize, usize, usize)> {
        let segments = self.segments.read().await;
        segments.get(&partition_id).map(|segs| {
            let num_segments = segs.len();
            let total_entries: usize = segs.iter().map(|s| s.entries.len()).sum();
            let total_size: usize = segs.iter().map(|s| s.size_bytes).sum();
            (num_segments, total_entries, total_size)
        })
    }

    /// Check if partition needs compaction
    pub async fn needs_compaction(&self, partition_id: i32) -> bool {
        let segments = self.segments.read().await;
        if let Some(segs) = segments.get(&partition_id) {
            let total_size: usize = segs.iter().map(|s| s.size_bytes).sum();
            total_size >= self.min_compaction_size
        } else {
            false
        }
    }
}
