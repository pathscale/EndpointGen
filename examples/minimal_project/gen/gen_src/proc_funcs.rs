use endpoint_gen::model::{Field, ProceduralFunction, Type};

/// Returns a vector of the available `ProceduralFunction`s (e.g. `auth`, `user`, `admin`, `chatbot`).
pub fn get_proc_functions() -> Vec<ProceduralFunction> {
    vec![get_example_func()].concat()
}

fn get_example_func() -> Vec<ProceduralFunction> {
    vec![ProceduralFunction::new(
        "fun_user_add_object", // Proc func name
        vec![
            // Proc func input params
            Field::new("kind", Type::Int),
            Field::new("id", Type::Int),
            Field::new("timestamp", Type::BigInt),
            Field::new("transaction_hash", Type::BlockchainTransactionHash),
            Field::new("contract_address", Type::BlockchainAddress),
            Field::new("detail", Type::Object), // JSON object
        ],
        vec![Field::new("success", Type::Boolean)], // Proc func returns
        // Raw sql
        r#"
        BEGIN
            -- delete same kind of object for the same address
            DELETE FROM tbl.object WHERE contract_address = a_contract_address;
            INSERT INTO tbl.object (kind, id, timestamp, transaction_hash, contract_address, detail)
            VALUES (a_kind, a_id, a_timestamp, a_transaction_hash, a_contract_address, a_detail);
        END
        "#,
    )]
}
