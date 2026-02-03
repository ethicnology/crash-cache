use flate2::Compression;
use flate2::read::{GzDecoder, GzEncoder};
use std::io::Read;

use crate::shared::domain::DomainError;

#[derive(Clone)]
pub struct GzipCompressor {
    level: Compression,
}

impl GzipCompressor {
    pub fn new() -> Self {
        Self {
            level: Compression::default(),
        }
    }

    pub fn with_level(level: u32) -> Self {
        Self {
            level: Compression::new(level),
        }
    }

    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>, DomainError> {
        let mut encoder = GzEncoder::new(data, self.level);
        let mut compressed = Vec::new();
        encoder
            .read_to_end(&mut compressed)
            .map_err(|e| DomainError::Compression(e.to_string()))?;
        Ok(compressed)
    }

    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, DomainError> {
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| DomainError::Decompression(e.to_string()))?;
        Ok(decompressed)
    }
}

impl Default for GzipCompressor {
    fn default() -> Self {
        Self::new()
    }
}
