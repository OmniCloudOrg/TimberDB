use crate::{CompressionAlgorithm, Error, Result};
use std::io::{Read, Write};

/// Maximum uncompressed data size to prevent memory exhaustion (256MB)
const MAX_UNCOMPRESSED_SIZE: usize = 256 * 1024 * 1024;

/// Maximum compressed data size to prevent memory exhaustion (512MB)
const MAX_COMPRESSED_SIZE: usize = 512 * 1024 * 1024;

/// Compress data using the specified algorithm
pub fn compress(data: &[u8], algorithm: CompressionAlgorithm, level: i32) -> Result<Vec<u8>> {
    // Validate input size
    if data.len() > MAX_UNCOMPRESSED_SIZE {
        return Err(Error::ResourceLimit(format!(
            "Data too large for compression: {} bytes (max: {})",
            data.len(),
            MAX_UNCOMPRESSED_SIZE
        )));
    }
    
    if data.is_empty() {
        return Ok(Vec::new());
    }
    
    match algorithm {
        CompressionAlgorithm::None => Ok(data.to_vec()),
        CompressionAlgorithm::Lz4 => compress_lz4(data),
        #[cfg(feature = "compression")]
        CompressionAlgorithm::Zstd => compress_zstd(data, level),
    }
}

/// Decompress data using the specified algorithm
pub fn decompress(data: &[u8], algorithm: CompressionAlgorithm) -> Result<Vec<u8>> {
    // Validate input size
    if data.len() > MAX_COMPRESSED_SIZE {
        return Err(Error::ResourceLimit(format!(
            "Compressed data too large: {} bytes (max: {})",
            data.len(),
            MAX_COMPRESSED_SIZE
        )));
    }
    
    if data.is_empty() {
        return Ok(Vec::new());
    }
    
    match algorithm {
        CompressionAlgorithm::None => Ok(data.to_vec()),
        CompressionAlgorithm::Lz4 => decompress_lz4(data),
        #[cfg(feature = "compression")]
        CompressionAlgorithm::Zstd => decompress_zstd(data),
    }
}

/// Compress using LZ4
fn compress_lz4(data: &[u8]) -> Result<Vec<u8>> {
    Ok(lz4_flex::compress_prepend_size(data))
}

/// Decompress using LZ4
fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>> {
    lz4_flex::decompress_size_prepended(data)
        .map_err(|e| Error::Compression(format!("LZ4 decompression failed: {}", e)))
}

/// Compress using Zstd
#[cfg(feature = "compression")]
fn compress_zstd(data: &[u8], level: i32) -> Result<Vec<u8>> {
    // Clamp level to valid range
    let level = level.clamp(1, 22);
    
    zstd::bulk::compress(data, level)
        .map_err(|e| Error::Compression(format!("Zstd compression failed: {}", e)))
}

/// Decompress using Zstd
#[cfg(feature = "compression")]
fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>> {
    zstd::bulk::decompress(data, MAX_UNCOMPRESSED_SIZE)
        .map_err(|e| Error::Compression(format!("Zstd decompression failed: {}", e)))
}

/// Calculate compression ratio (original_size / compressed_size)
pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f64 {
    if compressed_size == 0 {
        return 0.0;
    }
    
    original_size as f64 / compressed_size as f64
}

/// Estimate compressed size for a given algorithm (rough estimate)
pub fn estimate_compressed_size(data_size: usize, algorithm: CompressionAlgorithm) -> usize {
    match algorithm {
        CompressionAlgorithm::None => data_size,
        CompressionAlgorithm::Lz4 => (data_size as f64 * 0.7) as usize, // ~30% compression
        #[cfg(feature = "compression")]
        CompressionAlgorithm::Zstd => (data_size as f64 * 0.5) as usize, // ~50% compression
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_no_compression() {
        let data = b"Hello, World!";
        let compressed = compress(data, CompressionAlgorithm::None, 0).unwrap();
        let decompressed = decompress(&compressed, CompressionAlgorithm::None).unwrap();
        
        assert_eq!(data, compressed.as_slice());
        assert_eq!(data, decompressed.as_slice());
    }
    
    #[test]
    fn test_lz4_compression() {
        let data = b"Hello, World! This is a test string that should compress well with LZ4.".repeat(100);
        let compressed = compress(&data, CompressionAlgorithm::Lz4, 0).unwrap();
        let decompressed = decompress(&compressed, CompressionAlgorithm::Lz4).unwrap();
        
        assert_eq!(data, decompressed);
        assert!(compressed.len() < data.len());
        
        let ratio = compression_ratio(data.len(), compressed.len());
        assert!(ratio > 1.0);
    }
    
    #[cfg(feature = "compression")]
    #[test]
    fn test_zstd_compression() {
        let data = b"Hello, World! This is a test string that should compress well with Zstd.".repeat(100);
        let compressed = compress(&data, CompressionAlgorithm::Zstd, 3).unwrap();
        let decompressed = decompress(&compressed, CompressionAlgorithm::Zstd).unwrap();
        
        assert_eq!(data, decompressed);
        assert!(compressed.len() < data.len());
        
        let ratio = compression_ratio(data.len(), compressed.len());
        assert!(ratio > 1.0);
    }
    
    #[test]
    fn test_empty_data() {
        let data = b"";
        
        for algorithm in [CompressionAlgorithm::None, CompressionAlgorithm::Lz4] {
            let compressed = compress(data, algorithm, 0).unwrap();
            let decompressed = decompress(&compressed, algorithm).unwrap();
            
            assert_eq!(data, decompressed.as_slice());
        }
    }
    
    #[test]
    fn test_size_limits() {
        // Create data that exceeds the limit
        let large_data = vec![0u8; MAX_UNCOMPRESSED_SIZE + 1];
        
        let result = compress(&large_data, CompressionAlgorithm::Lz4, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Data too large"));
    }
}