use crate::definitions::{EnumElement, ErrorCodeSchema, GenService};
use crate::rust;
use convert_case::{Case, Casing};
use endpoint_libs::libs::error_code::ErrorCode;
use endpoint_libs::model::Type;
use eyre::bail;
use std::collections::{HashMap, HashSet};

pub fn build_error_code_catalog(custom_error_codes: Vec<ErrorCodeSchema>) -> eyre::Result<Vec<ErrorCodeSchema>> {
    let mut codes = builtin_error_codes();
    codes.extend(custom_error_codes);

    let mut names: HashMap<String, ErrorCodeSchema> = HashMap::new();
    let mut numbers: HashMap<i64, ErrorCodeSchema> = HashMap::new();

    for code in &codes {
        let variant = validate_error_code_variant(&code.name)?;
        if let Some(existing) = names.insert(variant.clone(), code.clone()) {
            bail!(
                "Duplicate error code name '{}': conflicts with '{}'",
                code.name,
                existing.name
            );
        }

        if let Some(existing) = numbers.insert(code.code, code.clone()) {
            bail!(
                "Duplicate error code value {} for '{}' and '{}'",
                code.code,
                existing.name,
                code.name
            );
        }
    }

    codes.sort_by_key(|code| code.code);
    Ok(codes)
}

pub fn validate_reserved_enum_names(enums: &[EnumElement]) -> eyre::Result<()> {
    for enum_element in enums {
        if let Type::Enum { name, .. } = &enum_element.inner {
            if name.to_case(Case::Pascal) == "ErrorCode" {
                bail!("Enum name 'ErrorCode' is reserved for generated endpoint error codes");
            }
        }
    }

    Ok(())
}

pub fn validate_endpoint_error_codes(services: &[GenService], error_codes: &[ErrorCodeSchema]) -> eyre::Result<()> {
    let allowed_variants = error_codes
        .iter()
        .map(|code| validate_error_code_variant(&code.name))
        .collect::<eyre::Result<HashSet<_>>>()?;

    for service in services {
        for endpoint in &service.endpoints {
            for error in &endpoint.schema.errors {
                let variant = rust::error_code_variant_name(error.code.variant());
                if !allowed_variants.contains(&variant) {
                    bail!(
                        "Unknown error code '{}' in endpoint '{}' error '{}'",
                        error.code,
                        endpoint.schema.name,
                        error.name
                    );
                }
            }
        }
    }

    Ok(())
}

fn validate_error_code_variant(name: &str) -> eyre::Result<String> {
    let variant = rust::error_code_variant_name(name);
    let is_valid = variant.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && variant.chars().all(|c| c.is_ascii_alphanumeric());

    if !is_valid {
        bail!("Invalid error code name '{name}': expected a Rust enum variant name");
    }

    Ok(variant)
}

fn builtin_error_codes() -> Vec<ErrorCodeSchema> {
    vec![
        ErrorCodeSchema::new("BadRequest", ErrorCode::BAD_REQUEST.code() as i64, "Bad request"),
        ErrorCodeSchema::new(
            "Unauthorized",
            ErrorCode::UNAUTHORIZED.code() as i64,
            "Authentication is required",
        ),
        ErrorCodeSchema::new(
            "PaymentRequired",
            ErrorCode::PAYMENT_REQUIRED.code() as i64,
            "Payment is required",
        ),
        ErrorCodeSchema::new("Forbidden", ErrorCode::FORBIDDEN.code() as i64, "Access is forbidden"),
        ErrorCodeSchema::new("NotFound", ErrorCode::NOT_FOUND.code() as i64, "Resource was not found"),
        ErrorCodeSchema::new(
            "MethodNotAllowed",
            ErrorCode::METHOD_NOT_ALLOWED.code() as i64,
            "Method is not allowed",
        ),
        ErrorCodeSchema::new(
            "NotAcceptable",
            ErrorCode::NOT_ACCEPTABLE.code() as i64,
            "Response format is not acceptable",
        ),
        ErrorCodeSchema::new(
            "ProxyAuthenticationRequired",
            ErrorCode::PROXY_AUTHENTICATION_REQUIRED.code() as i64,
            "Proxy authentication is required",
        ),
        ErrorCodeSchema::new(
            "RequestTimeout",
            ErrorCode::REQUEST_TIMEOUT.code() as i64,
            "Request timed out",
        ),
        ErrorCodeSchema::new(
            "Conflict",
            ErrorCode::CONFLICT.code() as i64,
            "Request conflicts with current state",
        ),
        ErrorCodeSchema::new("Gone", ErrorCode::GONE.code() as i64, "Resource is gone"),
        ErrorCodeSchema::new(
            "LengthRequired",
            ErrorCode::LENGTH_REQUIRED.code() as i64,
            "Content length is required",
        ),
        ErrorCodeSchema::new(
            "PreconditionFailed",
            ErrorCode::PRECONDITION_FAILED.code() as i64,
            "Precondition failed",
        ),
        ErrorCodeSchema::new(
            "PayloadTooLarge",
            ErrorCode::PAYLOAD_TOO_LARGE.code() as i64,
            "Payload is too large",
        ),
        ErrorCodeSchema::new("UriTooLong", ErrorCode::URI_TOO_LONG.code() as i64, "URI is too long"),
        ErrorCodeSchema::new(
            "UnsupportedMediaType",
            ErrorCode::UNSUPPORTED_MEDIA_TYPE.code() as i64,
            "Media type is unsupported",
        ),
        ErrorCodeSchema::new(
            "RangeNotSatisfiable",
            ErrorCode::RANGE_NOT_SATISFIABLE.code() as i64,
            "Requested range cannot be satisfied",
        ),
        ErrorCodeSchema::new(
            "ExpectationFailed",
            ErrorCode::EXPECTATION_FAILED.code() as i64,
            "Expectation failed",
        ),
        ErrorCodeSchema::new("ImATeapot", ErrorCode::IM_A_TEAPOT.code() as i64, "I'm a teapot"),
        ErrorCodeSchema::new(
            "MisdirectedRequest",
            ErrorCode::MISDIRECTED_REQUEST.code() as i64,
            "Request was misdirected",
        ),
        ErrorCodeSchema::new(
            "UnprocessableEntity",
            ErrorCode::UNPROCESSABLE_ENTITY.code() as i64,
            "Entity could not be processed",
        ),
        ErrorCodeSchema::new("Locked", ErrorCode::LOCKED.code() as i64, "Resource is locked"),
        ErrorCodeSchema::new(
            "FailedDependency",
            ErrorCode::FAILED_DEPENDENCY.code() as i64,
            "Dependency failed",
        ),
        ErrorCodeSchema::new(
            "UpgradeRequired",
            ErrorCode::UPGRADE_REQUIRED.code() as i64,
            "Request must be upgraded",
        ),
        ErrorCodeSchema::new(
            "PreconditionRequired",
            ErrorCode::PRECONDITION_REQUIRED.code() as i64,
            "Precondition is required",
        ),
        ErrorCodeSchema::new(
            "TooManyRequests",
            ErrorCode::TOO_MANY_REQUESTS.code() as i64,
            "Too many requests",
        ),
        ErrorCodeSchema::new(
            "RequestHeaderFieldsTooLarge",
            ErrorCode::REQUEST_HEADER_FIELDS_TOO_LARGE.code() as i64,
            "Request header fields are too large",
        ),
        ErrorCodeSchema::new(
            "UnavailableForLegalReasons",
            ErrorCode::UNAVAILABLE_FOR_LEGAL_REASONS.code() as i64,
            "Unavailable for legal reasons",
        ),
        ErrorCodeSchema::new(
            "InternalError",
            ErrorCode::INTERNAL_ERROR.code() as i64,
            "Internal server error",
        ),
        ErrorCodeSchema::new(
            "NotImplemented",
            ErrorCode::NOT_IMPLEMENTED.code() as i64,
            "Endpoint is not implemented",
        ),
        ErrorCodeSchema::new("BadGateway", ErrorCode::BAD_GATEWAY.code() as i64, "Bad gateway"),
        ErrorCodeSchema::new(
            "ServiceUnavailable",
            ErrorCode::SERVICE_UNAVAILABLE.code() as i64,
            "Service is unavailable",
        ),
        ErrorCodeSchema::new(
            "GatewayTimeout",
            ErrorCode::GATEWAY_TIMEOUT.code() as i64,
            "Gateway timed out",
        ),
        ErrorCodeSchema::new(
            "HttpVersionNotSupported",
            ErrorCode::HTTP_VERSION_NOT_SUPPORTED.code() as i64,
            "HTTP version is not supported",
        ),
        ErrorCodeSchema::new(
            "VariantAlsoNegotiates",
            ErrorCode::VARIANT_ALSO_NEGOTIATES.code() as i64,
            "Content negotiation variant problem",
        ),
        ErrorCodeSchema::new(
            "InsufficientStorage",
            ErrorCode::INSUFFICIENT_STORAGE.code() as i64,
            "Insufficient storage",
        ),
        ErrorCodeSchema::new(
            "LoopDetected",
            ErrorCode::LOOP_DETECTED.code() as i64,
            "Loop was detected",
        ),
        ErrorCodeSchema::new(
            "NotExtended",
            ErrorCode::NOT_EXTENDED.code() as i64,
            "Request must be extended",
        ),
        ErrorCodeSchema::new(
            "NetworkAuthenticationRequired",
            ErrorCode::NETWORK_AUTHENTICATION_REQUIRED.code() as i64,
            "Network authentication is required",
        ),
    ]
}
