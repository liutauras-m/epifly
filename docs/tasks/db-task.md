4. redb is risky as the central production metadata store

redb is fine for embedded local state, edge, dev, desktop, or single-node deployments. But your architecture uses it for threads, messages, workspace metadata, audit events, tenant seeding, and encrypted IAM creds.

That raises production questions:

Concern	Why it matters
HA	What happens when the node dies?
backups	How are point-in-time restores handled?
migrations	How are schema changes versioned?
scaling	Does one embedded store become the platform bottleneck?
audit durability	Audit logs need stronger guarantees than “it lives in the same embedded file.”
credential blast radius	Encrypted creds are still sitting in the same operational database.

Better architecture:

Use redb only for:

local cache
dev/test mode
edge/offline shell metadata
ephemeral route snapshots

Use Postgres for:

tenants
users
threads
messages
workspace metadata
audit log index
capability registry state
billing metadata references

Use object storage for:

large artifacts
uploaded files
generated files
audit event archive

Use Qdrant for vectors, but with strict tenant filtering.

The boring answer is Postgres. Yes, revolutionary.