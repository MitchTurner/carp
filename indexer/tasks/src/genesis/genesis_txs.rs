extern crate shred;

use crate::config::EmptyConfig::EmptyConfig;
use cardano_multiplatform_lib::{
    address::ByronAddress,
    genesis::byron::{config::GenesisData, parse::redeem_pubkey_to_txid},
    utils::Value,
};
use entity::{
    prelude::*,
    sea_orm::{DatabaseTransaction, DbErr, EntityTrait, Set},
};
use shred::{DispatcherBuilder, ResourceId, System, SystemData, World, Write};
use std::sync::{Arc, Mutex};

use crate::dsl::task_macro::*;
use crate::utils::{blake2b256, TaskPerfAggregator};
use entity::sea_orm::{IntoActiveModel, Iterable};
use futures::future::{join_all, try_join};

use super::genesis_block::GenesisBlockTask;

carp_task! {
  name GenesisTransactionTask;
  configuration EmptyConfig;
  doc "Parses Genesis transactions (avvm & non-avvm balances from genesis)";
  era genesis;
  dependencies [GenesisBlockTask];
  read [genesis_block];
  write [genesis_txs, genesis_addresses, genesis_outputs];
  should_add_task |block, _properties| {
    !block.1.avvm_distr.is_empty() || !block.1.non_avvm_balances.is_empty()
  };
  execute |previous_data, task| handle_txs(
      task.db_tx,
      task.block,
      &previous_data.genesis_block.as_ref().unwrap()
  );
  merge_result |previous_data, result| {
    *previous_data.genesis_txs = result.0;
    *previous_data.genesis_addresses = result.1;
    *previous_data.genesis_outputs = result.2;
  };
}

async fn handle_txs(
    db_tx: &DatabaseTransaction,
    block_info: BlockInfo<'_, GenesisData>,
    database_block: &BlockModel,
) -> Result<
    (
        Vec<TransactionModel>,
        Vec<AddressModel>,
        Vec<TransactionOutputModel>,
    ),
    DbErr,
> {
    // note: avvm added before non-avvm
    // https://github.com/input-output-hk/cardano-ledger/blob/ac51494e151af0ad99b937a787458ce71db0aaea/eras/byron/ledger/impl/src/Cardano/Chain/UTxO/GenesisUTxO.hs#L21

    let mut transactions: Vec<TransactionActiveModel> = vec![];
    // note: genesis file is a JSON structure, so there shouldn't be duplicate addresses
    // even across avvm and non-avvm it should be unique, otherwise two txs with the same tx hash would exist
    let mut address_lambdas: Vec<Box<dyn Fn(i64) -> AddressActiveModel>> = vec![];
    let mut outputs: Vec<cardano_multiplatform_lib::TransactionOutput> = vec![];

    for (pub_key, amount) in block_info.1.avvm_distr.iter() {
        let (tx_hash, extended_addr) =
            redeem_pubkey_to_txid(pub_key, Some(block_info.1.protocol_magic));
        let byron_addr =
            ByronAddress::from_bytes(extended_addr.to_address().as_ref().to_vec()).unwrap();

        // note: strictly speaking, genesis txs are unordered so there is no defined index
        let tx_index = transactions.len() as i32;
        transactions.push(TransactionActiveModel {
            block_id: Set(database_block.id),
            hash: Set(tx_hash.to_bytes().to_vec()),
            is_valid: Set(true),
            payload: Set(byron_addr.to_bytes()),
            tx_index: Set(tx_index),
            ..Default::default()
        });

        let addr_copy = byron_addr.clone();
        address_lambdas.push(Box::new(move |tx_id| AddressActiveModel {
            payload: Set(addr_copy.to_bytes()),
            first_tx: Set(tx_id),
            ..Default::default()
        }));

        // TODO: this is actually wrong. CML uses the Shelley format, but this should be the Byron format
        outputs.push(cardano_multiplatform_lib::TransactionOutput::new(
            &byron_addr.to_address(),
            &Value::new(amount),
        ));
    }

    // note: empty on mainnet
    for (addr, amount) in block_info.1.non_avvm_balances.iter() {
        let byron_addr = ByronAddress::from_bytes(addr.as_ref().to_vec()).unwrap();

        let tx_hash = blake2b256(addr.as_ref());

        // println!("{}", amount.to_str());
        // println!("{}", byron_addr.to_base58());
        // println!("{}", hex::encode(tx_hash));

        // note: strictly speaking, genesis txs are unordered so there is no defined index
        let tx_index = transactions.len() as i32;
        transactions.push(TransactionActiveModel {
            block_id: Set(database_block.id),
            hash: Set(tx_hash.to_vec()),
            is_valid: Set(true),
            payload: Set(byron_addr.to_bytes()),
            tx_index: Set(tx_index),
            ..Default::default()
        });

        let addr_copy = byron_addr.clone();
        address_lambdas.push(Box::new(move |tx_id| AddressActiveModel {
            payload: Set(addr_copy.to_bytes()),
            first_tx: Set(tx_id),
            ..Default::default()
        }));

        // TODO: this is actually wrong. CML uses the Shelley format, but this should be the Byron format
        outputs.push(cardano_multiplatform_lib::TransactionOutput::new(
            &byron_addr.to_address(),
            &Value::new(amount),
        ));
    }

    let inserted_txs = insert_active_models(db_tx, &transactions).await?;
    let addresses: Vec<_> = address_lambdas
        .iter()
        .enumerate()
        .map(|(idx, addr)| addr(inserted_txs[idx].id))
        .collect();
    let inserted_addresses = insert_active_models(db_tx, &addresses).await?;

    let outputs_to_add: Vec<_> = inserted_txs
        .iter()
        .zip(&inserted_addresses)
        .enumerate()
        .map(|(i, (tx, addr))| TransactionOutputActiveModel {
            address_id: Set(addr.id),
            tx_id: Set(tx.id),
            payload: Set(outputs[i].to_bytes()),
            // recall: genesis txs are hashes of addresses
            // so all txs have a single output
            output_index: Set(0),
            ..Default::default()
        })
        .collect();
    let inserted_outputs = insert_active_models(db_tx, &outputs_to_add).await?;

    Ok((inserted_txs, inserted_addresses, inserted_outputs))
}

// https://github.com/SeaQL/sea-orm/issues/691
async fn insert_active_models<ActiveModel, Model>(
    db_tx: &DatabaseTransaction,
    transactions: &[ActiveModel], // Should this be for Iter?
) -> Result<Vec<<ActiveModel::Entity as EntityTrait>::Model>, DbErr>
where
    <<ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model: IntoActiveModel<ActiveModel>,
    ActiveModel: ActiveModelBehavior + Send + Sync,
    ActiveModel: ActiveModelTrait,
{
    let future_inserts = transactions
        .iter()
        .map(|active_model| active_model.clone().insert(db_tx)) // Can we do without clone?
        .collect::<Vec<_>>();
    join_all(future_inserts).await.into_iter().collect()
}
