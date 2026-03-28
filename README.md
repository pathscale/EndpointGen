# endpoint-gen

[![Crates.io](https://img.shields.io/crates/v/endpoint-gen.svg)](https://crates.io/crates/endpoint-gen)
[![dependency status](https://deps.rs/crate/endpoint-gen/1.5.1/status.svg)](https://deps.rs/crate/endpoint-gen/1.5.1)

A CLI code generator for WebSocket API endpoints used across Pathscale projects. Reads `.ron` config files describing services, enums, and structs, and generates Rust model code and documentation.

## Installation

```sh
cargo install endpoint-gen
```

## Usage

```sh
endpoint-gen --config-dir <path/to/config> --output-dir <path/to/project>
```

- `--config-dir`: directory containing `.ron` config files and `version.toml` (defaults to current directory)
- `--output-dir`: root of the project where `generated/` will be written (defaults to current directory)

Generated files are written to `<output-dir>/generated/`.

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

## Version Compatibility

`endpoint-gen` and `endpoint-libs` are versioned together. **Minor versions must match** between all Pathscale crates in a project (`endpoint-gen`, `endpoint-libs`, `honey_id-types`).

For example, `endpoint-gen 1.3.x` must be paired with `endpoint-libs 1.3.x`.

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
