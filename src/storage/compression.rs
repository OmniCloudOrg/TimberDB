// TimberDB: A high-performance distributed log database
// storage/compression.rs - Compression algorithms implementation

use std::io;
use thiserror::Error;

use crate::config::CompressionAlgorithm;

// Compression errors
#[derive(Error, Debug)]
pub enum CompressionError {
    #[error("Compression error: {0}")]
    Compression(String),
    
    #[error("Decompression error: {0}")]
    Decompression(String),
    
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Unsupported compression algorithm: {0:?}")]
    UnsupportedAlgorithm(CompressionAlgorithm),
}

/// Compress data using the specified algorithm
pub fn compress_data(
    data: &[u8],
    algorithm: CompressionAlgorithm,
    level: i32,
) -> Result<Vec<u8>, CompressionError> {
    match algorithm {
        CompressionAlgorithm::None => {
            // No compression, just return the data
            Ok(data.to_vec())
        }
        CompressionAlgorithm::LZ4 => {
            // Use LZ4 compression
            Ok(lz4_flex::block::compress(data))
        }
        CompressionAlgorithm::Zstd => {
            // Use Zstd compression
            let level = level.clamp(1, 22); // Valid zstd levels: 1-22
            
            zstd::bulk::compress(data, level)
                .map_err(|e| CompressionError::Compression(e.to_string()))
        }
        CompressionAlgorithm::Snappy => {
            // Use Snappy compression
            snap::raw::Encoder::new().compress_vec(data)
                .map_err(|e| CompressionError::Compression(e.to_string()))
        }
        CompressionAlgorithm::Gzip => {
            // Use Gzip compression
            let mut encoder = flate2::write::GzEncoder::new(
                Vec::new(),
                flate2::Compression::new(level.clamp(0, 9) as u32),
            );
            
            match std::io::copy(&mut &data[..], &mut encoder) {
                Ok(_) => encoder.finish()
                    .map_err(|e| CompressionError::Compression(e.to_string())),
                Err(e) => Err(CompressionError::Compression(e.to_string())),
            }
        }
    }
}

/// Decompress data using the specified algorithm
pub fn decompress_data(
    data: &[u8],
    algorithm: CompressionAlgorithm,
) -> Result<Vec<u8>, CompressionError> {
    match algorithm {
        CompressionAlgorithm::None => {
            // No compression, just return the data
            Ok(data.to_vec())
        }
        CompressionAlgorithm::LZ4 => {
            // Use LZ4 decompression
            // For decompression we need to specify max decompressed size as a safety feature
            let max_size = 1024 * 1024 * 1024; // 1GB max size as a reasonable limit
            lz4_flex::block::decompress(data, max_size)
                .map_err(|e| CompressionError::Decompression(e.to_string()))
        }
        CompressionAlgorithm::Zstd => {
            // Use Zstd decompression
            let max_size = 1024 * 1024 * 1024; // 1GB max size as a reasonable limit
            zstd::bulk::decompress(data, max_size)
                .map_err(|e| CompressionError::Decompression(e.to_string()))
        }
        CompressionAlgorithm::Snappy => {
            // Use Snappy decompression
            snap::raw::Decoder::new().decompress_vec(data)
                .map_err(|e| CompressionError::Decompression(e.to_string()))
        }
        CompressionAlgorithm::Gzip => {
            // Use Gzip decompression
            let mut decoder = flate2::read::GzDecoder::new(data);
            let mut result = Vec::new();
            
            match io::copy(&mut decoder, &mut result) {
                Ok(_) => Ok(result),
                Err(e) => Err(CompressionError::Decompression(e.to_string())),
            }
        }
    }
}

/// Calculate the compression ratio (original size / compressed size)
pub fn compression_ratio(original_size: u64, compressed_size: u64) -> f64 {
    if compressed_size == 0 {
        return 0.0;
    }
    
    original_size as f64 / compressed_size as f64
}