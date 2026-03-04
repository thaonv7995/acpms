# Log Aggregation Implementation

## Overview

The `LogNormalizer` struct provides log aggregation functionality to group consecutive similar tool operations, reducing timeline clutter in agent execution logs.

## Location

**File:** `/Users/thaonv/Projects/Personal/Agentic-Coding/crates/executors/src/normalization.rs`

## Implementation Details

### Core Components

#### LogNormalizer Struct

```rust
pub struct LogNormalizer;
```

A zero-sized type that implements log aggregation logic through stateless methods.

#### Key Methods

1. **`new() -> Self`**
   - Creates a new LogNormalizer instance
   - Zero-cost abstraction (no heap allocation)

2. **`aggregate_consecutive_actions(&self, entries: &[NormalizedEntry]) -> Vec<NormalizedEntry>`**
   - Main aggregation function
   - Groups consecutive Read/Grep/Glob operations (≥3 operations)
   - Maintains chronological order
   - Returns aggregated entries

3. **`flush_buffer(...)`** (private)
   - Flushes buffered actions when:
     - Tool type changes
     - Non-action entry encountered
     - End of stream reached
   - Only aggregates if buffer contains ≥3 operations
   - Otherwise, keeps individual entries

4. **`create_aggregated_action(...)`** (private)
   - Converts buffered actions into AggregatedAction
   - Captures start/end line numbers and timestamps
   - Preserves all individual operation details

### Aggregation Rules

**Aggregate These Tools:**
- `Read` - File reading operations
- `Grep` - Search operations
- `Glob` - File pattern matching

**Threshold:**
- Minimum 3 consecutive operations of the same type

**Break Aggregation On:**
- Tool type changes (e.g., Read → Grep)
- Non-aggregatable tool (e.g., Edit, Bash)
- Non-action entry (e.g., TodoItem, FileChange)

### Output Structure

**Before Aggregation:**
```
1. Action: Read file1.rs
2. Action: Read file2.rs
3. Action: Read file3.rs
4. Action: Read file4.rs
```

**After Aggregation:**
```
1. AggregatedAction: Read × 4 operations
   - file1.rs (line 1)
   - file2.rs (line 2)
   - file3.rs (line 3)
   - file4.rs (line 4)
```

## Usage Example

```rust
use acpms_executors::normalization::{LogNormalizer, NormalizedEntry};

let normalizer = LogNormalizer::new();
let entries: Vec<NormalizedEntry> = /* ... */;

let aggregated = normalizer.aggregate_consecutive_actions(&entries);
```

See `crates/executors/examples/log_aggregation.rs` for a complete working example.

## Testing

### Test Coverage

Three comprehensive tests cover the aggregation logic:

1. **`test_aggregate_consecutive_reads`**
   - Verifies 3+ consecutive Read operations aggregate correctly
   - Checks operation count and metadata preservation

2. **`test_no_aggregate_when_less_than_three`**
   - Ensures operations below threshold remain separate
   - Validates <3 operations are not aggregated

3. **`test_flush_on_tool_change`**
   - Tests buffer flushing on tool type changes
   - Verifies Read → Grep transition behavior

### Running Tests

```bash
# Run all normalization tests
cargo test -p acpms-executors --lib -- normalization

# Run only aggregation tests
cargo test -p acpms-executors --lib -- aggregation_tests
```

## Design Decisions

### Why Clone Actions?

The buffer is temporary and operations need to be moved into either:
- Individual `NormalizedEntry::Action` variants, or
- An aggregated `NormalizedEntry::AggregatedAction`

Cloning is acceptable because:
1. Buffer is short-lived (typically 3-10 items)
2. Actions are small structs (tool name, target, timestamp)
3. Avoids complex lifetime management

### Why ≥3 Threshold?

- 2 operations don't provide meaningful aggregation benefit
- 3+ operations significantly reduce timeline clutter
- Balances information density vs. detail visibility

### Why Only Read/Grep/Glob?

These are high-frequency, similar operations that benefit from aggregation:
- **Read:** File exploration phase (often reads 5-20 files)
- **Grep:** Code search phase (multiple pattern searches)
- **Glob:** File discovery (pattern matching across directories)

Other tools (Edit, Bash, Write) are typically fewer and more important to show individually.

## Performance Characteristics

- **Time Complexity:** O(n) where n = number of entries
- **Space Complexity:** O(n) for result + O(k) for buffer (k ≤ n)
- **Allocations:** Minimal - only for result vector and temporary buffer

## Future Enhancements

Potential improvements for Phase 2:

1. **Configurable Threshold**
   - Allow customization of minimum aggregation count
   - Per-tool threshold settings

2. **Time-Based Aggregation**
   - Group operations within time windows
   - Break aggregation on large time gaps

3. **Smart Aggregation**
   - Aggregate related file reads (same directory)
   - Group semantically similar operations

4. **Aggregation Statistics**
   - Track aggregation ratios
   - Monitor timeline compression metrics

## Integration Points

The `LogNormalizer` is designed to integrate with:

1. **Log Parser:** Receives `Vec<NormalizedEntry>` from parsing pipeline
2. **Timeline Renderer:** Consumes aggregated entries for UI display
3. **Database Storage:** Stores both individual and aggregated entries
4. **API Endpoints:** Returns aggregated logs for client consumption

## References

- **Normalization Module:** `/Users/thaonv/Projects/Personal/Agentic-Coding/crates/executors/src/normalization.rs`
- **Example:** `/Users/thaonv/Projects/Personal/Agentic-Coding/crates/executors/examples/log_aggregation.rs`
- **Tests:** Lines 527-642 in normalization.rs
