use crate::scorers::ScorerResult;

pub fn print_report(results: &[ScorerResult]) {
    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let avg_score: f64 = if total == 0 {
        0.0
    } else {
        results.iter().map(|r| r.score).sum::<f64>() / total as f64
    };

    println!();
    println!("══════════════════════════════════════════════");
    println!("  EVAL REPORT");
    println!("══════════════════════════════════════════════");
    println!("  Samples:   {}", total);
    println!("  Passed:    {} / {}", passed, total);
    println!("  Avg score: {:.1}%", avg_score * 100.0);
    println!(
        "  Result:    {}",
        if passed == total {
            "✅ ALL PASS"
        } else {
            "❌ SOME FAILED"
        }
    );
    println!("══════════════════════════════════════════════");
}
