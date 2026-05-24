3. Agent evals are under-specified

The doc mentions an evals harness and scorecard types, but it does not define a serious evaluation regime. Research on LLM-agent evaluation emphasizes planning, tool use, memory, reflection, and task-specific benchmarks, while RAG evaluation literature stresses factuality, retrieval quality, safety, and efficiency.

You need evals for the actual system, not just the model.

Add these eval suites:

Eval suite	Measures
tool-selection eval	Did router choose correct capability?
tool-argument eval	Did model produce valid/optimal JSON?
refusal/security eval	Did it avoid unsafe tool use?
tenant-isolation eval	Did tenant A ever retrieve tenant B content?
upload-pipeline eval	Did file type → extraction → indexing → plan work?
RAG/retrieval eval	recall@k, MRR, faithfulness, citation correctness
latency/cost eval	p50/p95 routing, model, tool-call, upload pipeline
regression eval	compare every capability before deploy

Minimum practical setup:

/tests/evals/
  routing/
  tool_args/
  rag/
  security/
  tenant_isolation/
  uploads/
  cost_latency/

Each eval case should include:

{
  "input": "...",
  "tenant": "tenant_a",
  "expected_capability": "invoice-processing",
  "forbidden_capabilities": ["storage-fs", "admin"],
  "expected_schema_valid": true,
  "max_latency_ms": 3000,
  "must_not_leak": ["tenant_b"]
}

This is not optional. In 2026, an agent platform without evals is just a confident slot machine.