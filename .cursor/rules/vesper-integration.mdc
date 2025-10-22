---
alwaysApply: true
---
# Vesper Vector Database Integration Standards

## Architecture Overview
- L0 Tier (HNSW): Hot memory, sub-3ms P50 latency for real-time queries
- L1 Tier (IVF-PQ): Project memory, SSD-friendly with PQ/OPQ quantization  
- L2 Tier (Disk-graph): Historical storage, billion-scale with DiskANN patterns
- Performance targets: P50 ≤ 1-3ms, P99 ≤ 10-20ms for 1536D embeddings

## API Usage Patterns
// Always use vesper::collection with proper error handling
auto collection_result = vesper::collection::open(path);
if (!collection_result) {
return std::unexpected(collection_result.error());
}
auto& collection = collection_result.value();

// Multi-modal embedding storage (1536D: 512+256+256+256+256)
struct MultiModalEmbedding {
std::array<float, 512> semantic;
std::array<float, 256> structural;
std::array<float, 256> performance;
std::array<float, 256> context;
std::array<float, 256> quality;
};


## Performance Integration
- Use Roaring bitmaps for metadata filtering during ANN search
- Leverage SIMD kernels (AVX2/AVX-512) for distance computations
- Implement proper cross-tier data movement with optimal quantization
- Cache-friendly data layouts with 64-byte alignment for vector operations

## Error Handling with Vesper
- Always check std::expected return values from Vesper APIs
- Use vesper::core::error enum for consistent error propagation
- Implement proper cleanup and resource management with RAII patterns
- Handle crash recovery via WAL replay and snapshot restoration