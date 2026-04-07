//! Stream manager: holds all named streams.

use std::sync::Arc;

use bytes::Bytes;
use dashmap::DashMap;

use crate::error::StreamError;
use crate::entry::StreamEntry;
use crate::{Stream, StreamId};

/// Manages all streams in the KAYA instance.
pub struct StreamManager {
    streams: DashMap<String, Arc<Stream>>,
    default_max_entries: usize,
}

impl StreamManager {
    pub fn new(default_max_entries: usize) -> Self {
        Self {
            streams: DashMap::new(),
            default_max_entries,
        }
    }

    /// Get or create a stream by name.
    fn get_or_create(&self, name: &str) -> Arc<Stream> {
        self.streams
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Stream::new(name.to_string(), self.default_max_entries)))
            .value()
            .clone()
    }

    /// Get a stream, returning error if not found.
    fn get_stream(&self, name: &str) -> Result<Arc<Stream>, StreamError> {
        self.streams
            .get(name)
            .map(|s| s.value().clone())
            .ok_or_else(|| StreamError::StreamNotFound(name.into()))
    }

    /// XADD: append entry to a stream (auto-creates stream if needed).
    pub fn xadd(
        &self,
        stream_name: &str,
        id_hint: Option<&str>,
        fields: Vec<(Bytes, Bytes)>,
    ) -> Result<StreamId, StreamError> {
        let stream = self.get_or_create(stream_name);
        stream.xadd(id_hint, fields)
    }

    /// XLEN: get the length of a stream.
    pub fn xlen(&self, stream_name: &str) -> Result<usize, StreamError> {
        let stream = self.get_stream(stream_name)?;
        Ok(stream.xlen())
    }

    /// XREAD: read from one or more streams.
    pub fn xread(
        &self,
        streams: &[(String, StreamId)],
        count: Option<usize>,
    ) -> Result<Vec<(String, Vec<StreamEntry>)>, StreamError> {
        let mut results = Vec::new();
        for (name, after) in streams {
            if let Some(stream_ref) = self.streams.get(name) {
                let entries = stream_ref.xread(*after, count);
                if !entries.is_empty() {
                    results.push((name.clone(), entries));
                }
            }
        }
        Ok(results)
    }

    /// XREADGROUP: read from a stream as part of a consumer group.
    pub fn xreadgroup(
        &self,
        stream_name: &str,
        group_name: &str,
        consumer_name: &str,
        count: Option<usize>,
    ) -> Result<Vec<StreamEntry>, StreamError> {
        let stream = self.get_stream(stream_name)?;
        stream.xreadgroup(group_name, consumer_name, count)
    }

    /// XACK: acknowledge entries.
    pub fn xack(
        &self,
        stream_name: &str,
        group_name: &str,
        ids: &[StreamId],
    ) -> Result<u64, StreamError> {
        let stream = self.get_stream(stream_name)?;
        stream.xack(group_name, ids)
    }

    /// XGROUP CREATE
    pub fn xgroup_create(
        &self,
        stream_name: &str,
        group_name: &str,
        start_id: StreamId,
    ) -> Result<(), StreamError> {
        let stream = self.get_or_create(stream_name);
        stream.xgroup_create(group_name, start_id)
    }

    /// XTRIM: trim a stream.
    pub fn xtrim(
        &self,
        stream_name: &str,
        max_len: usize,
    ) -> Result<usize, StreamError> {
        let stream = self.get_stream(stream_name)?;
        Ok(stream.xtrim(max_len))
    }

    /// XRANGE
    pub fn xrange(
        &self,
        stream_name: &str,
        start: StreamId,
        end: StreamId,
        count: Option<usize>,
    ) -> Result<Vec<StreamEntry>, StreamError> {
        let stream = self.get_stream(stream_name)?;
        Ok(stream.xrange(start, end, count))
    }

    /// XGROUP DELCONSUMER
    pub fn xgroup_delconsumer(
        &self,
        stream_name: &str,
        group_name: &str,
        consumer_name: &str,
    ) -> Result<u64, StreamError> {
        let stream = self.get_stream(stream_name)?;
        stream.xgroup_delconsumer(group_name, consumer_name)
    }

    /// Number of streams.
    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new(0)
    }
}
