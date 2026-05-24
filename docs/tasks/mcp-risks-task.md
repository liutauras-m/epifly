2. MCP is treated as an integration format, not a hostile supply chain

Your design supports remote MCP providers and has CONUSAI_MCP_ALLOWED_HOSTS, which is good. But 2025–2026 research has made MCP security ugly. Multiple papers now discuss tool poisoning, prompt injection through MCP metadata, and vulnerabilities in real MCP clients.

The weak point is not only “malicious user input.” It is malicious tool descriptions, malicious schemas, malicious tool responses, and changed remote server behavior.

Fixes to add:

Risk	Required improvement
Tool poisoning	Sign capability manifests and pin remote MCP tool schema hashes
Tool rug-pull	Store last-known schema hash and require admin approval on schema drift
Prompt injection via tool output	Mark all tool output as untrusted data, never instruction text
Overbroad MCP access	Per-capability egress allowlist, not only global host allowlist
Remote MCP compromise	Capability-level kill switch + degraded routing status
Prompt/schema injection	Static scanner for descriptions, schemas, examples, and hidden instructions

Add this to the architecture as a required trust boundary:

Remote MCP server
→ schema fetch
→ signature/hash verification
→ static policy scan
→ admin approval if changed
→ registered capability
→ runtime sandbox/egress policy

Without this, “self-registering capabilities” becomes “self-registering attack surface.” Lovely feature, terrible obituary.