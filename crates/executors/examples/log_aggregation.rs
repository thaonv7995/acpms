// Example demonstrating log aggregation functionality
//
// Run with: cargo run -p acpms-executors --example log_aggregation

use acpms_executors::normalization::{ActionType, LogNormalizer, NormalizedEntry};
use chrono::Utc;

fn main() {
    println!("Log Aggregation Example\n");

    let normalizer = LogNormalizer::new();

    // Simulate a series of consecutive Read operations
    let entries = vec![
        NormalizedEntry::Action(ActionType {
            tool_name: "Read".into(),
            action: "read".into(),
            target: Some("src/main.rs".into()),
            timestamp: Utc::now(),
            line_number: 1,
        }),
        NormalizedEntry::Action(ActionType {
            tool_name: "Read".into(),
            action: "read".into(),
            target: Some("src/lib.rs".into()),
            timestamp: Utc::now(),
            line_number: 2,
        }),
        NormalizedEntry::Action(ActionType {
            tool_name: "Read".into(),
            action: "read".into(),
            target: Some("Cargo.toml".into()),
            timestamp: Utc::now(),
            line_number: 3,
        }),
        NormalizedEntry::Action(ActionType {
            tool_name: "Read".into(),
            action: "read".into(),
            target: Some("README.md".into()),
            timestamp: Utc::now(),
            line_number: 4,
        }),
        // Different tool - will cause flush and separate entry
        NormalizedEntry::Action(ActionType {
            tool_name: "Edit".into(),
            action: "edit".into(),
            target: Some("src/main.rs".into()),
            timestamp: Utc::now(),
            line_number: 5,
        }),
        // More Reads
        NormalizedEntry::Action(ActionType {
            tool_name: "Read".into(),
            action: "read".into(),
            target: Some("tests/test1.rs".into()),
            timestamp: Utc::now(),
            line_number: 6,
        }),
        NormalizedEntry::Action(ActionType {
            tool_name: "Read".into(),
            action: "read".into(),
            target: Some("tests/test2.rs".into()),
            timestamp: Utc::now(),
            line_number: 7,
        }),
    ];

    println!("Input: {} entries", entries.len());
    println!("- 4 consecutive Read operations");
    println!("- 1 Edit operation (breaks aggregation)");
    println!("- 2 more Read operations (too few to aggregate)\n");

    let aggregated = normalizer.aggregate_consecutive_actions(&entries);

    println!("Output: {} entries", aggregated.len());
    for (i, entry) in aggregated.iter().enumerate() {
        match entry {
            NormalizedEntry::AggregatedAction(agg) => {
                println!(
                    "{}. AggregatedAction: {} × {} operations (lines {}-{})",
                    i + 1,
                    agg.tool_name,
                    agg.total_count,
                    agg.start_line,
                    agg.end_line
                );
                for (j, op) in agg.operations.iter().enumerate() {
                    println!(
                        "   {}. {} (line {})",
                        j + 1,
                        op.target.as_ref().unwrap_or(&"<no target>".to_string()),
                        op.line_number
                    );
                }
            }
            NormalizedEntry::Action(action) => {
                println!(
                    "{}. Action: {} {} (line {})",
                    i + 1,
                    action.tool_name,
                    action.target.as_ref().unwrap_or(&"<no target>".to_string()),
                    action.line_number
                );
            }
            _ => {}
        }
    }

    println!("\n✓ Aggregation successful!");
    println!("  - 4 consecutive Reads → 1 AggregatedAction");
    println!("  - 1 Edit → kept as individual Action");
    println!("  - 2 Reads → kept as 2 individual Actions (< 3 threshold)");
}
