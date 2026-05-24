6. Public /metrics, /docs, /openapi.json need environment-specific policy

The doc lists /metrics, /docs, and /openapi.json as public/no-auth. That is convenient in dev and often wrong in production.

Endpoint	Production recommendation
/metrics	internal network only or auth-gated
/docs	disabled or admin-gated
/openapi.json	public only if this is intended as external API contract
/admin/capabilities/register	never in public router mentally; even with token, isolate and rate-limit

Prometheus metrics can leak route names, tenant patterns, error rates, internal services, model failures, and infrastructure topology. Swagger docs help attackers build a menu. Very hospitable. Too hospitable.