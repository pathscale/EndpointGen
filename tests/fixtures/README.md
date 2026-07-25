# Golden fixtures

## `api_support_cafe_openapi.golden.yaml`

The OpenAPI 3.1 projection of the `api.support.cafe` endpoint surface — 26 operations
across 7 services (`adminApi`, `appAdminApi`, `appApi`, `appConnect`, `authApi`,
`supportApi`, `userApi`), which is exactly the set exposed in that repo's
`docs/*_mcp_tools.json`.

**Provenance.** Originally committed to `api.support.cafe` as `docs/openapi.yaml` in
[`3b50c57`](https://github.com/pathscale/api.support.cafe/commit/3b50c57) ("Add OpenAPI
projection of the endpoint surface"), hand-produced ahead of the emitter existing. It was
removed from that repo because a checked-in spec sitting next to generator output is a
competing source of truth — the RON is the source of truth, and once
`endpointgen`'s `openapi.rs` lands, the document must be generated, not maintained.

It lives here instead because this is the repo that will emit it. `PLAN-2.1.md` §3.1/§3.2
were revised (2026-07-25 review) to adopt its conventions verbatim — plan and fixture
agree on all of these:

| | Convention |
|---|---|
| Document | one merged `docs/openapi.json` covering all services, grouped by tag |
| Path | `POST /{serviceName}/{endpoint_snake_name}` — e.g. `/adminApi/delete_app` |
| operationId | `{serviceName}_{endpoint_snake_name}` — e.g. `adminApi_delete_app` |
| Tags | `[{serviceName}]` |
| Vendor extensions | `x-endpoint-code`, `x-roles`, `x-frontend-facing` |

The path scheme is structural, not cosmetic: the RON namespace is
`(service_name, service_id)`, so a flat `/rpc/{Name}` scheme collides the moment two
services reuse an endpoint name or documents get merged.

**Tiebreak rule.** If any further divergence between plan and fixture turns up during
implementation, the *fixture's* convention wins unless it is demonstrably wrong — it is
the deployed artifact.

**How to use it.** This is a *reference for the shape*, not a byte-exact expectation. It
was produced by hand from an older RON state, so it will not match emitter output
verbatim. Diff against it to confirm the emitter reproduces the path scheme,
operationId scheme, tagging and vendor extensions; once the emitter is trusted,
regenerate this file from the emitter and it becomes a true golden.

It also documents one downstream consumer: it was the input to the Auto-MCP `rq2`
experiment (`api.support.cafe/rq2_out/.automcp_config.json`, and the vendored copy in
`mcp-replication-package/rq2/`). That replication is pinned by git SHA
(`MANIFEST.json` → `api.support.cafe` @ `88ef163`), so removing the file from the
service repo's working tree does not affect it.
