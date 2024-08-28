use endpoint_gen::model::{EndpointSchema, Field, Service, Type};

/// Returns a vector of the available `Service`s (e.g. `auth`, `user`, `admin`, `chatbot`).
pub fn get_services() -> Vec<Service> {
    vec![Service::new("service_1", 1, get_service_endpoints())]
}

pub fn get_service_endpoints() -> Vec<EndpointSchema> {
    vec![example_endpoint()]
}

pub fn example_endpoint() -> EndpointSchema {
    EndpointSchema::new(
        "Authorize", // name
        10030,       // code
        vec![
            // input params
            Field::new("username", Type::String),
            Field::new("token", Type::UUID),
            Field::new("service", Type::enum_ref("service")),
            Field::new("device_id", Type::String),
            Field::new("device_os", Type::String),
        ],
        vec![Field::new("success", Type::Boolean)], // returns
    )
}
