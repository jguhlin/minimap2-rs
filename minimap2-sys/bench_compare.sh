#!/bin/bash

# Threading Implementation Benchmark Comparison Script
# This script runs benchmarks for both pthread and rust-threading implementations
# and generates a comparative analysis.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "🧪 Threading Implementation Benchmark Comparison"
echo "=================================================="
echo

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Create results directory
RESULTS_DIR="benchmark_results/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

echo -e "${BLUE}📁 Results will be saved to: $RESULTS_DIR${NC}"
echo

# Function to run benchmark with specific features
run_benchmark() {
    local impl_name=$1
    local features=$2
    local output_dir=$3
    
    echo -e "${YELLOW}🔥 Running $impl_name benchmarks...${NC}"
    
    # Clean previous builds to ensure feature changes take effect
    cargo clean -q
    
    # Run benchmark with specified features
    # Set CRITERION_HOME to control output location
    export CRITERION_HOME="$output_dir"
    if [ -n "$features" ]; then
        echo "   Using features: $features"
        cargo bench --features "$features" --bench threading
    else
        echo "   Using default features (pthread)"
        cargo bench --bench threading
    fi
    
    echo -e "${GREEN}✅ $impl_name benchmarks completed${NC}"
    echo
}

# Function to check system info
print_system_info() {
    echo -e "${BLUE}🖥️  System Information:${NC}"
    echo "   OS: $(uname -s -r)"
    echo "   CPU: $(grep 'model name' /proc/cpuinfo | head -1 | cut -d: -f2 | xargs || echo 'Unknown')"
    echo "   CPU Cores: $(nproc)"
    echo "   Memory: $(free -h | awk '/^Mem:/ {print $2}' || echo 'Unknown')"
    echo "   Rust Version: $(rustc --version)"
    echo
}

# Function to generate summary report
generate_summary() {
    local results_dir=$1
    
    cat > "$results_dir/README.md" << EOF
# Threading Implementation Benchmark Results

Generated on: $(date)

## System Information
- OS: $(uname -s -r)
- CPU: $(grep 'model name' /proc/cpuinfo | head -1 | cut -d: -f2 | xargs || echo 'Unknown')
- CPU Cores: $(nproc)
- Memory: $(free -h | awk '/^Mem:/ {print $2}' || echo 'Unknown')
- Rust Version: $(rustc --version)

## Benchmark Results

### pthread Implementation (Default)
- Results: [pthread/index.html](pthread/index.html)
- Uses the original C kthread implementation with pthreads
- Maximum compatibility and battle-tested performance

### rust-threading Implementation
- Results: [rust-threading/index.html](rust-threading/index.html)
- Uses Rayon (kt_for) and Crossbeam channels (kt_pipeline)
- Enables Windows/WASM support, no pthread dependency

## Interpretation Guide

### kt_for Benchmarks
- Tests parallel for-loop performance with different work sizes and thread counts
- Look for: throughput (elements/second), latency, scaling with thread count
- Rayon's work-stealing should excel with uneven workloads

### kt_pipeline Benchmarks
- Tests pipeline threading with 3-stage producer-consumer chain
- Look for: overall pipeline throughput, thread coordination efficiency
- Crossbeam channels should show low-latency message passing

### threading_overhead Benchmarks
- Tests raw threading overhead with minimal work per task
- Look for: overhead of thread creation/management
- Shows pure threading performance without computational load

## Key Metrics to Compare
1. **Throughput**: Elements processed per second
2. **Latency**: Time per operation
3. **Scalability**: Performance improvement with more threads
4. **Overhead**: Performance with minimal work (threading_overhead test)

## Expected Trade-offs
- **pthread**: Lower overhead, proven performance, POSIX-only
- **rust-threading**: Better work distribution, cross-platform, slightly higher overhead
EOF
}

# Main execution
echo -e "${BLUE}🚀 Starting benchmark comparison...${NC}"
print_system_info

# Run pthread implementation benchmarks
echo -e "${YELLOW}==================== pthread Implementation ====================${NC}"
run_benchmark "pthread" "" "$RESULTS_DIR/pthread"

# Run rust-threading implementation benchmarks  
echo -e "${YELLOW}================= rust-threading Implementation =================${NC}"
run_benchmark "rust-threading" "rust-threading" "$RESULTS_DIR/rust-threading"

# Generate summary
generate_summary "$RESULTS_DIR"

echo -e "${GREEN}🎉 Benchmark comparison completed!${NC}"
echo
echo -e "${BLUE}📊 View results:${NC}"
echo "   Summary: $RESULTS_DIR/README.md"
echo "   pthread HTML: $RESULTS_DIR/pthread/index.html"
echo "   rust-threading HTML: $RESULTS_DIR/rust-threading/index.html"
echo
echo -e "${YELLOW}💡 Tips:${NC}"
echo "   - Open HTML files in browser for interactive charts"
echo "   - Look for throughput differences in high-thread-count scenarios"
echo "   - Compare threading_overhead results for raw performance"
echo "   - Check pipeline benchmarks for message-passing efficiency"