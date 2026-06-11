# Example project

## To run

- `cargo install --git https://github.com/pathscale/EndpointGen.git --tag v0.4.4`, ideally latest tag

- `endpoint-gen <optional>--config-dir CONFIG_DIR <optional>--output-dir OUTPUT_DIR`

EndpointGen will create directories and files in the given output dir, or the current dir if none is supplied.

Please ensure that a version.toml is placed in the config dir to specify the version requirements of the binary.
The version adheres to the standard semver format

## Typed endpoint errors

Custom error codes live in `config/errors.ron`:

```ron
Config(
    definition: ErrorCodeList(
        codes: [
            ErrorCodeSchema(
                name: "TooManyLoginAttempts",
                code: 200001,
                description: "The account has too many failed login attempts.",
            ),
        ],
    ),
)
```

Endpoints can declare public handler errors directly in schema and reference codes with the same enum-path style used by roles:

```ron
errors: [
    EndpointErrorSchema(
        name: "WrongPassword",
        code: "ErrorCode::Unauthorized",
        message: "Wrong password",
        fields: [],
    ),
    EndpointErrorSchema(
        name: "PasswordTooShort",
        code: "ErrorCode::BadRequest",
        message: "Password too short",
        fields: [
            Field(name: "min_length", ty: Int32),
            Field(name: "actual_length", ty: Int32),
        ],
    ),
    EndpointErrorSchema(
        name: "TooManyLoginAttempts",
        code: "ErrorCode::TooManyLoginAttempts",
        message: "Too many login attempts",
        fields: [
            Field(name: "retry_after_ms", ty: Int64),
        ],
    ),
],
```

`endpoint-gen` generates a `{EndpointName}Error` enum that can be used as the handler error type:

```rust
use endpoint_libs::libs::handler::{RequestHandler, Response};
use endpoint_libs::libs::toolbox::RequestContext;

pub struct MethodLogin;

#[async_trait::async_trait(?Send)]
impl RequestHandler for MethodLogin {
    type Request = LoginRequest;
    type Error = LoginError;

    async fn handle(&self, _ctx: RequestContext, req: LoginRequest) -> Response<LoginRequest, LoginError> {
        if req.password.len() < 8 {
            return Err(LoginError::PasswordTooShort {
                min_length: 8,
                actual_length: req.password.len() as i32,
            }
            .into());
        }

        Err(LoginError::WrongPassword.into())
    }
}
```
