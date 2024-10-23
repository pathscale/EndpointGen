# Example project


## Installation

To install `endpoint_gen`, use the following command:

```bash
cargo install --git https://github.com/pathscale/EndpointGen.git --tag v0.4.0
```

Ensure Cargo binaries are in your `PATH`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Verify the installation:

```bash
endpoint-gen --help
```

---

## Folder Structure

Here’s the typical folder structure for working with `endpoint_gen`:

```
project/
├── config/
│   ├── test_schema.ron
│   └── s1_endpoints.ron
├── output/
└── main.rs
```

- **`config/`**: Contains RON files with endpoint definitions.
- **`output/`**: Stores generated Rust code.
- **`main.rs`**: Entry point of your Rust application.

---

## To run
- `endpoint-gen <optional>--config-dir CONFIG_DIR <optional>--output-dir OUTPUT_DIR`

EndpointGen will create directories and files in the given output dir, or the current dir if none is supplied.

## Output Directory Structure

Ensure that the **`docs/` folder** exists within the output path **before running** the tool each time. This is required because **`error_codes.json`** must be **manually edited** to populate the generated error codes in the Rust code.

### Example Folder Structure:

```
output/
├── docs/
│   ├── error_codes/
│   │   └── error_codes.json
│   └── services.json
└── generated/
    └── model.rs
```

### Sample `error_codes.json`:

```json
{
  "language": "en",
  "codes": [
    {
      "code": 404,
      "symbol": "BadRequest",
      "message": "Not page found!",
      "source": "Custom"
    },
    {
      "code": 500,
      "symbol": "InternalServerError",
      "message": "Internal Server Error",
      "source": "Custom"
    }
  ]
}
```

### Rust Code Generated from `error_codes.json`:

```rust
#[postgres(name = "enum_ErrorCode")]
pub enum EnumErrorCode {
    /// Custom Not page found!
    #[postgres(name = "BadRequest")]
    BadRequest = 404,
    /// Custom Internal Server Error
    #[postgres(name = "InternalServerError")]
    InternalServerError = 500,
}

impl From<EnumErrorCode> for ErrorCode {
    fn from(e: EnumErrorCode) -> Self {
        ErrorCode::new(e as _)
    }
}
```

---

## Generating the Output Files

To generate the files from the RON configurations, use the following command:

```bash
endpoint-gen --config-dir ./config --output-dir ./output
```

This command reads the `.ron` files from the `config/` directory and generates the output files in the `output/` directory.

---

## Example Usage of the Generated Files

### **Using `model.rs` in Your Rust Code**

```rust
use serde_json;

fn send_request<T: WsRequest>(request: T) {
    let json = serde_json::to_string(&request).unwrap();
    println!("Sending request: {}", json);
}

fn main() {
    let request = UserGetSlippage1Request {};
    send_request(request);

    let response = UserGetSlippage1Response {
        data: vec![S1Slippage {
            id: 1,
            event_id: 100,
            event_timestamp: 1234567890,
        }],
    };

    println!("Response data: {:?}", response.data);
}
```

### **Handling Errors Using `error_codes.json`**

```rust
let error = EnumErrorCode::BadRequest;
let error_code: ErrorCode = error.into();
println!("Error Code: {:?}", error_code);
```

---

## Conclusion

`endpoint_gen` provides a complete toolkit for working with WebSocket-based APIs by generating code, schemas, and documentation from RON configuration files. The organized output ensures that developers have everything they need to integrate, document, and deploy their APIs effectively. **Make sure** to manually edit the `error_codes.json` file within the `docs/error_codes/` folder before generating the code to ensure accurate error handling and documentation.

## Integration with `endpoint_libs` (TODO)
TODO: Add detailed steps for integrating `endpoint_gen` with `endpoint_libs` once the refactoring of libraries is complete.

Please ensure that a version.toml is placed in the config dir to specify the version requirements of the binary.
The version adheres to the standard semver format