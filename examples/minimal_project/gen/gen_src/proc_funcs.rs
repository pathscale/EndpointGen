use endpoint_gen::model::{Field, ProceduralFunction, Type};

/// Returns a vector of the available `ProceduralFunction`s (e.g. `auth`, `user`, `admin`, `chatbot`).
pub fn get_proc_functions() -> Vec<ProceduralFunction> {
    vec![get_example_func()].concat()
}

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
