use crate::alonzo::{Header, HeaderBody, VrfCert};
use cardano_multiplatform_lib::utils::BigNum;
use entity::{
    block,
    block::EraValue,
    sea_orm::{
        ConnectionTrait, Database, DatabaseBackend, DatabaseTransaction, DbConn, DbErr, Schema,
        TransactionTrait,
    },
};
use pallas::{
    codec::{
        minicbor::{self, bytes::ByteVec, Decode},
        utils::{KeyValuePairs, MaybeIndefArray},
    },
    crypto::hash::Hash,
    ledger::primitives::alonzo::{self, TransactionBody},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tasks::{
    dsl::database_task::BlockGlobalInfo, execution_plan::ExecutionPlan,
    multiera::multiera_executor::process_multiera_block, utils::TaskPerfAggregator,
};

type OwnedBlockInfo = (String, alonzo::Block, BlockGlobalInfo);

fn some_tx() -> TransactionBody {
    todo!();
}

fn some_alonzo_block() -> OwnedBlockInfo {
    let cbor = "".to_string();

    let tx = some_tx();

    let txs = vec![tx];

    let block_type = alonzo::Block {
        header: Header {
            header_body: HeaderBody {
                block_number: 0,
                slot: 0,
                prev_hash: [0; 32].into(),
                issuer_vkey: Vec::new().into(),
                vrf_vkey: Vec::new().into(),
                nonce_vrf: VrfCert(Vec::new().into(), Vec::new().into()),
                leader_vrf: VrfCert(Vec::new().into(), Vec::new().into()),
                block_body_size: 0,
                block_body_hash: [0; 32].into(),
                operational_cert: Vec::new().into(),
                unknown_0: 0,
                unknown_1: 0,
                unknown_2: Vec::new().into(),
                protocol_version_major: 0,
                protocol_version_minor: 0,
            },
            body_signature: Vec::new().into(),
        },
        transaction_bodies: MaybeIndefArray::Def(txs),
        transaction_witness_sets: MaybeIndefArray::Def(Vec::new()),
        auxiliary_data_set: KeyValuePairs::Def(Vec::new()),
        invalid_transactions: None,
    };
    let block_global_data = BlockGlobalInfo {
        era: EraValue::Byron,
        epoch: Some(999),
        epoch_slot: None,
    };
    (cbor, block_type, block_global_data)
}

pub async fn in_memory_db_conn() -> DbConn {
    Database::connect("sqlite::memory:").await.unwrap()
}

pub fn new_perf_aggregator() -> Arc<Mutex<TaskPerfAggregator>> {
    Default::default()
}

async fn wrap_process_multiera_block(
    txn: &DatabaseTransaction,
    owned_block_info: OwnedBlockInfo,
    exec_plan: Arc<ExecutionPlan>,
    perf_aggregator: Arc<Mutex<TaskPerfAggregator>>,
) -> Result<(), DbErr> {
    let block_info = (
        owned_block_info.0.as_str(),
        &owned_block_info.1,
        &owned_block_info.2,
    );
    process_multiera_block(txn, block_info, &exec_plan, perf_aggregator.clone())
        .await
        .unwrap();
    Ok(())
}

async fn setup_schema(db: &DbConn) {
    let schema = Schema::new(DatabaseBackend::Sqlite);
    let stmt_for_blocks = schema.create_table_from_entity(block::Entity);

    let builder = db.get_database_backend();

    db.execute(builder.build(&stmt_for_blocks)).await.unwrap();
}

#[tokio::test]
async fn process_multiera_block__can_find_datum() {
    let conn = in_memory_db_conn().await;
    setup_schema(&conn).await;
    // setup_schema(&conn).await;
    let mut table = HashMap::new();
    table.insert("readonly".to_string(), false);

    let exec_plan = Arc::new(ExecutionPlan::from(vec![("MultiEraDatumTask", table)]));
    let perf_aggregator = new_perf_aggregator();
    let block_info = some_alonzo_block();

    conn.transaction(|db_tx| {
        Box::pin(wrap_process_multiera_block(
            db_tx,
            block_info,
            exec_plan.clone(),
            perf_aggregator.clone(),
        ))
    })
    .await
    .unwrap();
}
