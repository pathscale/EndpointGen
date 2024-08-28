# EndpointGen

A highly opinionated template engine to help generate endpoints and related pieces for Rust projects

## Running

1. Set up according to the guide below
2. Run `cargo build` for the `gen` crate
3. Generation will run on every build where something required for generation has changed since the last build
4. On first run, `/project_root/docs/error_codes.json` will be created. This is meant to be manually edited and is used to generate the error code logic for the endpoints, as well as to generate a more readable `error_codes.md` file

### Adding an error code

Add the following to `/project_root/docs/error_codes.json`:

```json
{
  "language": "en",
  "codes": [
    {
      "code": 100400,
      "symbol": "BadRequest",
      "message": "Bad Request",
      "source": "Custom"
    }
}
```

Upon next generation, inspect the `error_codes.md` file, as well as the generated `model.rs` file, to see that the error code you have added can be seen in those files.

## Setup and Getting Started

1. In a terminal navigated to the root of your project, run: `cargo new --lib --vcs none gen`
2. Add the following to the `gen` crate's `Cargo.toml`:

```toml
[dependencies]
endpoint-gen = "*" 

[build-dependencies]
endpoint-gen = "*"
eyre = "*"
```

3. Add a `build.rs` to the `gen` directory with at least the following:

```rust
use std::{env, path::PathBuf};

use endpoint_gen::Data;

fn main() -> eyre::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../docs/error_codes/error_codes.json");

    // Set these up to match your environment
    let current_dir = env::current_dir()?;
    let root = current_dir.parent().unwrap(); // This should evaluate to the root of your project where the project Cargo.toml can be found
    let output_dir = &current_dir.join("generated"); // This should evaluate to the `<root>/gen/generated/` dir

    let data = Data {
        project_root: PathBuf::from(&root),
        output_dir: PathBuf::from(root),
        services: gen_src::services::get_services(),
        enums: gen_src::enums::get_enums(),
        pg_funcs: gen_src::proc_funcs::get_proc_functions(),
    };

    endpoint_gen::main(data)?;

    Ok(())
}
```

5. Add the `gen` crate to your project root's Cargo.toml, and add it to the workspace dependencies:

```toml
[workspace]
members = [
  "gen",
  ...
]

[workspace.dependencies]
## Internal dependencies
gen = { path = "./gen" }
```

## Adding generation sources

Add `gen_src` as a module to the `gen` project:

- Add `gen_src.rs` at the same level as `build.rs`
- Add the `gen_src` directory at the same level

Add the following as submodules to `/gen/gen_src/`

- `services.rs`
- `enums.rs`
- `proc_funcs.rs`

Declare the modules in `/gen/gen_src.rs`:

```rust
pub mod enums;
pub mod proc_funcs;
pub mod services;
```

Your directory structure should now look like the following:

```text
project_root/
├─ gen/
│  ├─ gen_src/
│  │  ├─ enums.rs
│  │  ├─ proc_funcs.rs
│  │  ├─ services.rs
│  ├─ src/
│  │  ├─ lib.rs
│  ├─ build.rs
│  ├─ Cargo.toml
│  ├─ gen_src.rs
Cargo.toml
```

### Adding Services

Add the following to `services.rs`:

```rust
use endpoint_gen::model::Service;

/// Returns a vector of the available `Service`s (e.g. `auth`, `user`, `admin`, `chatbot`).
pub fn get_services() -> Vec<Service> {
    vec![]
}
```

Now edit `build.rs` and add the following:

```rust
// ...Includes above

fn main() -> eyre::Result<()> {
    ...
    let data = Data {
        ...
        services: services::get_services(),
        ...
    }
    ...
```

### Adding enums

Add the following to `enums.rs`:

```rust
use endpoint_gen::model::Type;

/// Returns a vector of the available `Service`s (e.g. `auth`, `user`, `admin`, `chatbot`).
pub fn get_enums() -> Vec<Type> {
    vec![]
}
```

Now edit `build.rs` and add the following:

```rust
...
fn main() -> eyre::Result<()> {
    ...
    let data = Data {
        ...
        enums: enums::get_enums(),
        ...
    }
    ...
```

### Adding procedural functions

Add the following to `proc_funcs.rs`:

```rust
use endpoint_gen::model::ProceduralFunction;

/// Returns a vector of the available `ProceduralFunction`s (e.g. `auth`, `user`, `admin`, `chatbot`).
pub fn get_proc_functions() -> Vec<ProceduralFunction> {
    vec![]
}
```

Now edit `build.rs` and add the following:

```rust
...
fn main() -> eyre::Result<()> {
    ...
    let data = Data {
        ...
        pg_funcs: proc_funcs::get_proc_functions(),
        ...
    }
    ...
```

## Defining generation sources

### Services

"Services" correspond to individual binaries that are intended to run as services within a Linux environment. Defining these allows endpoint-gen to create the corresponding service files that can be deployed to the target system and run.

A service has:

- A name
- An ID
- A list of Websocket endpoints that the service exposes

Add the following to `services.rs`:

```rust
use endpoint_gen::model::{EndpointSchema, Field, Service, Type};

pub fn get_services() -> Vec<Service> {
    vec![
        Service::new("service_1", 1, get_service_endpoints()),
    ]
}

pub fn get_service_endpoints() -> Vec<EndpointSchema> {
    vec![example_endpoint()]
}

pub fn example_endpoint() -> EndpointSchema {
    
}
```

#### Defining an endpoint

An endpoint, defined by `EndpointSchema`, is defined by:

- A name
- A unique numeric code
- A list of input parameters, defined by name and `Type`
- A list of return values, defined by name and `Type`

Add the following to the `example_endpoint` function:

```rust
EndpointSchema::new(
        "Authorize", // name
        10030, // code
        vec![  // input params
            Field::new("username", Type::String),
            Field::new("token", Type::UUID),
            Field::new("service", Type::enum_ref("service")),
            Field::new("device_id", Type::String),
            Field::new("device_os", Type::String),
        ],
        vec![Field::new("success", Type::Boolean)], // returns
    )
```

### Enums

An enum in this context refers to an enum used within the database (currently postgres) for enumerating various types of objects that may be required for logic or frontend display purposes

An enum (which is a variant of the actual Rust enum, `Type`, defined in `endpoint-gen/src/model/types.rs`) is defined by the following:

- A name
- A list of variants, defined by `EnumVariant`

#### Defining an enum

Add the following to the `get_enums` function in `enums.rs`:

```rust
use endpoint_gen::model::{EnumVariant, Type};

pub fn get_enums() -> Vec<Type> {
    vec![Type::enum_(
        "role".to_owned(),
        vec![
            EnumVariant::new("guest", 0),
            EnumVariant::new("user", 1),
            EnumVariant::new("admin", 2),
            EnumVariant::new("developer", 3),
        ],
    )]
}
```

As can be seen, an `EnumVariant` just consists of a name and an ordinal

### Procedural functions

A procedural function bundles and wraps an actual database query (currently SQL with Postgres) in a strongly typed and identifiable structure that contains:

- A name
- A list of parameters that the function accepts
- A return row type of the function
- The raw SQL of the function

Add the following to `get_proc_functions` and `proc_funcs.rs`:

```rust
use endpoint_gen::model::{Field, ProceduralFunction, Type};

pub fn get_proc_functions() -> Vec<ProceduralFunction> {
    vec![
        get_example_func(),
    ]
    .concat()
}

fn get_example_func() -> Vec<ProceduralFunction> {

}
```

#### Defining a procedural function

Add the following to `get_example_func`:

```rust
// TODO: Add simpler 'mock' proc func that contains the bare minimum to show how it works
fn get_example_func() -> Vec<ProceduralFunction> {
    vec![ProceduralFunction::new(
        "fun_user_add_event", // Proc func name
        vec![
            // Proc func input params
            Field::new("kind", Type::Int),
            Field::new("chain_id", Type::Int),
            Field::new("block_id", Type::BigInt),
            Field::new("block_time", Type::BigInt),
            Field::new("transaction_hash", Type::BlockchainTransactionHash),
            Field::new("from_address", Type::BlockchainAddress),
            Field::new("contract_address", Type::BlockchainAddress),
            Field::new("severity", Type::Int),
            Field::new("detail", Type::Object), // JSON object
            Field::new("signals", Type::Object),
        ],
        vec![Field::new("success", Type::Boolean)], // Proc func returns
        // Raw sql
        r#"
        BEGIN
            -- delete same kind of event for the same address
            DELETE FROM tbl.event WHERE from_address = a_from_address AND kind = a_kind;
            INSERT INTO tbl.event (kind, chain_id, block_id, block_time, transaction_hash, from_address, contract_address, severity, detail, signals)
            VALUES (a_kind, a_chain_id, a_block_id, a_block_time, a_transaction_hash, a_from_address, a_contract_address, a_severity, a_detail, a_signals);
        END
        "#,
    )]
}
```
