7. Capability self-registration needs provenance

Self-registration is great for plugins. It is also how you accidentally build npm with tool execution privileges.

Your doc says TOML manifests can expose tools and hot-reload through notify. Add:

capability provenance:
  author
  signing key id
  signature
  schema hash
  permissions requested
  network egress requested
  storage scopes requested
  risk class
  approval status

Then enforce:

Capability type	Required approval
chain-only, no external I/O	automatic in dev, approval in prod
native	signed build artifact required
WASM	signed + sandbox limits
MCP remote	signed + schema pin + egress policy
job-backed	signed + queue/resource limits