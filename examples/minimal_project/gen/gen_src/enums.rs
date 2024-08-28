use endpoint_gen::model::{EnumVariant, Type};

/// Returns a vector of the available `Service`s (e.g. `auth`, `user`, `admin`, `chatbot`).
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
