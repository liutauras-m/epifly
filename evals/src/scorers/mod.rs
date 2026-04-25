use agent_core::pipelines::invoice::InvoiceData;
use serde_json::Value;

pub struct ScorerResult {
    pub score: f64,
    pub passed: bool,
    #[allow(dead_code)]
    pub details: String,
}

pub struct InvoiceScorer {
    pass_threshold: f64,
}

impl InvoiceScorer {
    pub fn new() -> Self {
        Self {
            pass_threshold: 0.8,
        }
    }

    pub fn score(&self, extracted: &InvoiceData, expected: &Value) -> ScorerResult {
        let extracted_val = serde_json::to_value(extracted).unwrap_or(Value::Null);
        let fields = [
            "invoice_number",
            "invoice_date",
            "issuer_name",
            "billed_to_name",
            "currency",
            "total_amount",
            "status",
        ];

        let mut hits = 0usize;
        let mut misses = Vec::new();

        for field in &fields {
            let exp = expected.get(field);
            let got = extracted_val.get(field);
            match (exp, got) {
                (Some(e), Some(g)) if values_match(e, g) => hits += 1,
                (None, _) => hits += 1, // field not in expected → skip
                _ => misses.push(field.to_string()),
            }
        }

        let score = hits as f64 / fields.len() as f64;
        let passed = score >= self.pass_threshold;
        let details = if misses.is_empty() {
            "all fields match".into()
        } else {
            format!("mismatches: {}", misses.join(", "))
        };

        ScorerResult {
            score,
            passed,
            details,
        }
    }
}

fn values_match(expected: &Value, got: &Value) -> bool {
    match (expected, got) {
        (Value::String(e), Value::String(g)) => e.to_lowercase().trim() == g.to_lowercase().trim(),
        (Value::Number(e), Value::Number(g)) => {
            let ef = e.as_f64().unwrap_or(0.0);
            let gf = g.as_f64().unwrap_or(0.0);
            (ef - gf).abs() < 0.01
        }
        (Value::Null, Value::Null) => true,
        _ => expected == got,
    }
}

impl Default for InvoiceScorer {
    fn default() -> Self {
        Self::new()
    }
}
