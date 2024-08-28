use crate::model::*;
use lib::database::*;
#[allow(unused_imports)]
use lib::types::*;
use postgres_from_row::FromRow;
use rust_decimal::Decimal;
use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
pub struct FunUserAddObjectRespRow {
    pub success: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FunUserAddObjectReq {
    pub kind: i32,
    pub id: i32,
    pub timestamp: i64,
    pub transaction_hash: BlockchainTransactionHash,
    pub contract_address: BlockchainAddress,
    pub detail: serde_json::Value,
}

#[allow(unused_variables)]
impl DatabaseRequest for FunUserAddObjectReq {
    type ResponseRow = FunUserAddObjectRespRow;
    fn statement(&self) -> &str {
        "SELECT * FROM api.fun_user_add_object(a_kind => $1::int, a_id => $2::int, a_timestamp => $3::bigint, a_transaction_hash => $4::varchar, a_contract_address => $5::varchar, a_detail => $6::jsonb);"
    }
    fn params(&self) -> Vec<&(dyn ToSql + Sync)> {
        vec![
            &self.kind as &(dyn ToSql + Sync),
            &self.id as &(dyn ToSql + Sync),
            &self.timestamp as &(dyn ToSql + Sync),
            &self.transaction_hash as &(dyn ToSql + Sync),
            &self.contract_address as &(dyn ToSql + Sync),
            &self.detail as &(dyn ToSql + Sync),
        ]
    }
}
