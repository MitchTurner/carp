use crate::config::ReadonlyConfig::ReadonlyConfig;
use crate::era_common::block_from_hash;
use crate::utils::blake2b256;
use crate::{dsl::default_impl::has_transaction_multiera, dsl::task_macro::*};
use entity::sea_orm::{DatabaseTransaction, Set};
use pallas::ledger::primitives::alonzo::{self};
use pallas::ledger::primitives::Fragment;

carp_task! {
  name MultiEraDatumTask;
  configuration ReadonlyConfig;
  doc "Adds Datums paired with their Hash";
  era multiera;
  dependencies [];
  read [];
  write [multiera_datums];
  should_add_task |_block, _properties| {
    true
  };
  execute |previous_data, task| handle_datums(
      task.db_tx,
      task.block,
      task.config.readonly
  );
  merge_result |previous_data, result| {
    *previous_data.multiera_datums = result;
  };
}

async fn handle_datums(
    db_tx: &DatabaseTransaction,
    block: BlockInfo<'_, alonzo::Block<'_>>,
    readonly: bool,
) -> Result<Vec<()>, DbErr> {
    // let hash = blake2b256(&block.1.header.encode_fragment().unwrap());
    // if readonly {
    //     return block_from_hash(db_tx, &hash).await;
    // }
    // let block = BlockActiveModel {
    //     era: Set(block.2.era.into()),
    //     hash: Set(hash.to_vec()),
    //     height: Set(block.1.header.header_body.block_number as i32),
    //     epoch: Set(block.2.epoch.unwrap() as i32),
    //     slot: Set(block.1.header.header_body.slot as i32),
    //     ..Default::default()
    // };
    // Ok(block.insert(db_tx).await?)
    Ok(Vec::new())
}
