# endpoint-gen

[![Crates.io](https://img.shields.io/crates/v/endpoint-gen.svg)](https://crates.io/crates/endpoint-gen)
[![dependency status](https://deps.rs/crate/endpoint-gen/1.13.1/status.svg)](https://deps.rs/crate/endpoint-gen/1.13.1)

Schema-first code generator for WebSocket RPC services. You describe endpoints once in
declarative RON; `endpoint-gen` produces the Rust models, the human-facing docs, the MCP
tool schemas, and — optionally — OpenAPI 3.1 and AsyncAPI 3.0 documents.

The generated code targets [`endpoint-libs`](https://github.com/pathscale/endpoint-libs),
which is the runtime that serves it.

**Shipping an MCP server is close to free.** Define an endpoint in RON, and its MCP tool
definition — name, description, `inputSchema`, `outputSchema` — is generated with
everything else. `endpoint-libs` exposes the lot over JSON-RPC 2.0 with one `enable_mcp()`
call, on the same socket the RPC protocol already uses, with `tools/list` filtered by the
caller's roles. No tool definitions to hand-write, no JSON Schema to maintain by hand, no
separate MCP server to deploy and keep in sync. `docs/<service>_mcp_tools.json` shows
exactly what agents will see, so a schema change is a reviewable diff.

The codegen arrow points **from the schema to the code**, which is the whole point: most
Rust OpenAPI crates (`utoipa`, `aide`, `poem-openapi`, `dropshot`) derive a spec *from*
hand-written handlers and so delete no boilerplate. See
[endpoint-libs' comparison doc](https://github.com/pathscale/endpoint-libs/blob/main/docs/comparison.md)
for when you should use one of those instead.

## Installation

```sh
cargo install endpoint-gen
```

## Usage

```sh
endpoint-gen --config-dir <path/to/config> --output-dir <path/to/project>
```

| Flag | Effect |
|---|---|
| `--config-dir <path>` | Directory holding the `.ron` files and `version.toml`. Defaults to the current directory. |
| `--output-dir <path>` | Project root; `generated/` is written beneath it. Defaults to the current directory. |
| `--check` | Verify instead of write — see below. |
| `--openapi` | Also emit `docs/openapi.json` (OpenAPI 3.1). Off by default. |
| `--asyncapi` | Also emit `docs/asyncapi.json` (AsyncAPI 3.0). Off by default. |
| `--public-only` | Restrict the specification documents to `frontend_facing` endpoints. |
| `--allow-empty-descriptions` | Permit missing endpoint/variant/error descriptions. Legacy escape hatch. |

### Generated artifacts

| Path | Always? | What it is |
|---|---|---|
| `generated/model.rs` | yes | Rust types, method codes, handler scaffolding. Gitignored in our repos. |
| `docs/services.json` | **yes** | **Machine-readable endpoint description in our own format** — see below. |
| `docs/<service>_mcp_tools.json` | yes | Exactly what a server reports via MCP `tools/list`. |
| `docs/README.md` | yes | Human-facing reference. |
| `docs/error_codes/error_codes.md` | yes | The error-code catalog. |
| `docs/asyncapi.json` | `--asyncapi` | AsyncAPI 3.0 — the protocol, in a standard format. |
| `docs/openapi.json` | `--openapi` | OpenAPI 3.1 — a projection for HTTP tooling. |
| `docs/openapi-README.md` | with either | Explains whichever specification documents you enabled. |

#### `services.json` and AsyncAPI are parallel, not sequential

The specification documents were added in 2.1 and **deprecate nothing**. `services.json`
remains the primary machine-readable artifact and the one to build internal tooling
against:

```json
{ "services": [ { "name": "userApi", "id": 1,
                  "endpoints": [ { "name": "...", "code": 10000, "description": "...",
                                   "parameters": [...], "returns": [...], "errors": [...],
                                   "roles": [...], "stream_response": null } ] } ],
  "enums": [...], "structs": [...] }
```

It is always written, it is a format we define and control, and it changes when we decide
it changes. For anything inside our own stack that is materially less friction than
conforming to a specification whose vocabulary only approximately fits a WebSocket RPC
protocol.

Reach for `--asyncapi` when a consumer *outside* your control needs to read the protocol
and a bespoke format would be the obstacle. Both describe the same endpoints; pick by
audience.

One behavioural difference worth knowing: `services.json` contains **only
`frontend_facing` endpoints**, always. The specification documents contain everything
unless you pass `--public-only`.

### `--check`

Regenerates everything into a temporary directory and diffs it against the committed
`docs/` tree, writing nothing and exiting non-zero on drift.

```sh
endpoint-gen --config-dir config --check
```

A committed artifact is only trustworthy if something proves it still matches its source;
this is that proof, and it belongs in CI. Only `docs/` is compared — `generated/` is
gitignored in every consumer repo, so it is not a committed artifact and cannot
meaningfully drift.

Pass the same spec flags you generate with, or the check will report the documents you
chose not to emit as missing.

### Specification documents

`--openapi` and `--asyncapi` are **opt-in**: upgrading the generator will not start adding
committed artifacts to your repository.

> The OpenAPI document is a **projection for tooling, not a servable API**. This transport
> has no URLs, so paths are synthesized as `/{serviceName}/{endpoint_snake_name}`. The
> AsyncAPI document is the accurate one of the two. Both carry that
> warning in `info.description`, and `docs/openapi-README.md` is generated alongside them.

## Config Directory

The config directory must contain a `version.toml` and any number of `.ron` files. All `.ron` files are discovered recursively.

### `version.toml`

Declares the required versions of the binary and `endpoint-libs`:

```toml
[binary]
version = ">=0.5.0"

[libs]
version = "1.3.4"
```

Generation will fail if the installed binary or the caller's `endpoint-libs` version does not satisfy these constraints.

## RON File Format

Each `.ron` file wraps a single `Definition`:

```ron
#![enable(unwrap_newtypes)]
#![enable(unwrap_variant_newtypes)]
Config(
    definition: <DefinitionVariant>( ... )
)
```

### Endpoints

Define a service's WebSocket endpoints with `EndpointSchemaList`. Each file specifies the service name, a unique numeric service ID, and a list of endpoint schemas.

```ron
Config(
    definition: EndpointSchemaList (
        "my_service",
        1,
        [
            EndpointSchema(
                name: "UserGetBalance",
                code: 10100,
                parameters: [
                    Field(name: "user_id", ty: Optional(Int)),
                ],
                returns: [
                    Field(name: "data", ty: Struct(
                        name: "Balance",
                        fields: [
                            Field(name: "amount", ty: Numeric),
                        ],
                    )),
                ],
                stream_response: None,
                description: "Returns the current balance for the user.",
                json_schema: (),
                roles: ["UserRole::Superadmin"],
            ),
        ]
    )
)
```

Endpoints with push/subscription behaviour set `stream_response` to the type streamed back to the client:

```ron
stream_response: Some(DataTable(
    name: "LivePosition",
    fields: [
        Field(name: "id", ty: BigInt),
        Field(name: "price", ty: Numeric),
    ],
)),
```

Endpoint errors can declare the public handler errors generated for an endpoint. Error codes use the same quoted enum-path style as roles and must reference `ErrorCode`:

```ron
errors: [
    EndpointErrorSchema(
        name: "WrongPassword",
        code: "ErrorCode::WrongPassword",
        message: "Wrong password",
        fields: [],
    ),
    EndpointErrorSchema(
        name: "PasswordTooShort",
        code: "ErrorCode::PasswordTooShort",
        message: "Password too short",
        fields: [
            Field(name: "min_length", ty: Int32),
            Field(name: "actual_length", ty: Int32),
        ],
    ),
],
```

#### Available field types

| Type | Description |
|---|---|
| `UInt32` | Unsigned 32-bit integer |
| `Int32` | Signed 32-bit integer |
| `Int64` | Signed 64-bit integer |
| `Float64` | 64-bit float |
| `Boolean` | Boolean |
| `String` | UTF-8 string |
| `Bytea` | Byte array |
| `UUID` | UUID |
| `IpAddr` | IP address |
| `TimeStampMs` | Timestamp in milliseconds |
| `Object` | Arbitrary JSON object |
| `Unit` | No value |
| `Optional(T)` | Nullable field |
| `Vec(T)` | List of `T` |
| `Struct(name, fields)` | Inline named struct |
| `StructRef(name)` | Reference to a named struct |
| `StructTable(struct_ref)` | List of a named struct (tabular data) |
| `Enum(name, variants)` | Inline enum definition |
| `EnumRef(name, prefixed_name)` | Reference to a named enum |
| `BlockchainDecimal` | Blockchain decimal value |
| `BlockchainAddress` | Blockchain address |
| `BlockchainTransactionHash` | Blockchain transaction hash |

### Enums

Define enums with `EnumList`:

```ron
Config(
    definition: EnumList (
        [
            Enum(
                name: "UserRole",
                variants: [
                    EnumVariant(name: "Superadmin", value: 1, comment: "Full access."),
                    EnumVariant(name: "Support",    value: 2, comment: "Read-only access."),
                ],
            ),
        ]
    )
)
```

### Structs

Shared struct types can be declared with `Struct` or `StructList` and will be emitted as top-level types in the generated model.

### Error Codes

Built-in `endpoint-libs` error codes such as `ErrorCode::BadRequest` and `ErrorCode::Unauthorized` are always available. Project-specific codes are declared with `ErrorCodeList`, commonly in `config/errors.ron`:

```ron
Config(
    definition: ErrorCodeList(
        codes: [
            ErrorCodeSchema(
                name: "WrongPassword",
                code: 200001,
                description: "The submitted password does not match the user account.",
            ),
        ],
    ),
)
```

The generated model emits these as `EnumErrorCode`, and generation fails if an endpoint references an unknown `ErrorCode::Variant`.

### JSON Schema Generation

Enable JSON schema generation for enums and structs by setting the `json_schema_gen` configuration option on the parent element:

```ron
Config(
    definition: EnumList(
        config: (json_schema_gen: true),
        enum_elements: [
            EnumElement(
                inner: Enum(
                    name: "MyEnum",
                    variants: [
                        EnumVariant(name: "Variant1", value: 0, description: "..."),
                    ],
                ),
            ),
        ],
    ),
)
```

This applies the configuration to all child elements. When enabled, the generated code will include `schemars::JsonSchema` derives and imports. Your project must include the `schemars` crate as a dependency to use this feature.

## Version compatibility

`endpoint-gen` and `endpoint-libs` are **not** versioned together and their minor versions
need not match — as of this writing endpoint-gen 1.13 works with endpoint-libs 2.1 and
honey_id-types 2.0.

What is enforced, at generation time:

- `endpoint-gen` is built against a specific `endpoint-libs` requirement (currently `^2.1`)
  and compares it against `[libs] version` in your `config/version.toml`.
- Your `config/version.toml` declares a requirement on the binary via `[binary] version`.

A mismatch on either fails generation with an explicit message rather than emitting subtly
wrong code. Keep `[libs] version` in step with what `Cargo.lock` actually resolves — a
stale declaration is the usual cause of a confusing refusal to run.

```toml
# config/version.toml
[binary]
version = "1.13.0"   # a caret range: any 1.x >= 1.13.0

[libs]
version = "2.1.0"    # the concrete endpoint-libs version this repo builds against
```

The release order across the whole chain — endpoint-libs first, then honey_id-types and
endpoint-gen, then the backends — is documented in
[endpoint-libs' release-order runbook](https://github.com/pathscale/endpoint-libs/blob/main/docs/release-order.md).

## Releasing

Releases are managed with [`cargo-release`](https://github.com/crate-ci/cargo-release) and [`git-cliff`](https://github.com/orhun/git-cliff). Both must be installed:

```sh
cargo install cargo-release git-cliff
```

To cut a release:

```sh
./scripts/release.sh [--skip-bump] <patch|minor|major>
```

The script will:
1. Run `cargo release --execute <level>` — bumps the version in `Cargo.toml`, updates the deps.rs badge in this README, regenerates `CHANGELOG.md`, and commits everything as `chore(release): vX.Y.Z`.
2. Open your `$EDITOR` with the auto-generated tag notes (from `git-cliff`) for review.
3. Create an annotated tag using the edited notes as the tag body (shown as GitHub Release notes).
4. Push the commit and tag.
5. Prompt whether to publish to crates.io.

To preview what `cargo-release` would do without making changes:

```sh
cargo release patch  # omit --execute for a dry run
```
