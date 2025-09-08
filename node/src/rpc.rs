//! A collection of node-specific RPC methods.
//! Substrate provides the `sc-rpc` crate, which defines the core RPC layer
//! used by Substrate nodes. This file extends those RPC definitions with
//! capabilities that are specific to this project's runtime configuration.

#![warn(missing_docs)]

use std::sync::{Arc, Mutex};

use jsonrpsee::RpcModule;
use node_template_runtime::{opaque::Block, AccountId, Balance, Index, BlockNumber, Hash};
use crate::modules::tx::{self, TxVerifier};
use parity_scale_codec::{Encode, Decode};
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};

pub use sc_rpc_api::DenyUnsafe;

/// Full client dependencies.
pub struct FullDeps<C, P> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Whether to deny unsafe calls
    pub deny_unsafe: DenyUnsafe,
    pub pose_verifier: Arc<Mutex<Option<Arc<TxVerifier>>>>,
}

/// Instantiate all full RPC extensions.
pub fn create_full<C, P>(
	deps: FullDeps<C, P>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
	C: ProvideRuntimeApi<Block>,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + 'static,
	C: Send + Sync + 'static,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: pallet_contracts_rpc::ContractsRuntimeApi<Block, AccountId, Balance, BlockNumber, Hash>,
	C::Api: BlockBuilder<Block>,
	P: TransactionPool + 'static,
{
	use pallet_transaction_payment_rpc::{TransactionPaymentApiServer, TransactionPaymentRpc};
	use substrate_frame_rpc_system::{SystemApiServer, SystemRpc};
	use pallet_contracts_rpc::{ContractsApiServer, ContractsRpc};

    let mut module = RpcModule::new(());
    let FullDeps { client, pool, deny_unsafe, pose_verifier } = deps;

	module.merge(SystemRpc::new(client.clone(), pool.clone(), deny_unsafe).into_rpc())?;
	module.merge(TransactionPaymentRpc::new(client.clone()).into_rpc())?;
    module.merge(ContractsRpc::new(client).into_rpc())?;

    // PoSE Tx attestation RPCs
    {
        let pose_verifier = pose_verifier.clone();
        module.register_method("pose_submit_tx", move |params, _| {
            let hex_str: String = params.one()?;
            let bytes = match hex_str.strip_prefix("0x") {
                Some(s) => match hex::decode(s) { Ok(b) => b, Err(e) => return Ok(serde_json::json!({"error":"bad-hex","message": e.to_string()})) },
                None => match hex::decode(&hex_str) { Ok(b) => b, Err(e) => return Ok(serde_json::json!({"error":"bad-hex","message": e.to_string()})) },
            };
            let guard = pose_verifier.lock().unwrap();
            if let Some(verifier) = &*guard {
                let verdict = verifier.validate_tx(&bytes);
                let att = if matches!(verdict, tx::Verdict::Accept) { verifier.attestate_if_leader(&bytes) } else { None };
                drop(guard);
                if let Some(att) = att {
                    let att_hex = format!("0x{}", hex::encode(att.encode()));
                    Ok(serde_json::json!({"verdict":"ACCEPT","attestation": att_hex}))
                } else {
                    let v = match verdict {
                        tx::Verdict::Accept => "ACCEPT",
                        tx::Verdict::Reject(_) => "REJECT",
                        tx::Verdict::Defer(_) => "DEFER",
                    };
                    Ok(serde_json::json!({"verdict": v}))
                }
            } else {
                Ok(serde_json::json!({"error":"verifier-not-ready"}))
            }
        })?;
    }

    {
        let pose_verifier = pose_verifier.clone();
        module.register_method("pose_submit_attestation", move |params, _| {
            let hex_str: String = params.one()?;
            let bytes = match hex_str.strip_prefix("0x") {
                Some(s) => match hex::decode(s) { Ok(b) => b, Err(e) => return Ok(serde_json::json!({"error":"bad-hex","message": e.to_string()})) },
                None => match hex::decode(&hex_str) { Ok(b) => b, Err(e) => return Ok(serde_json::json!({"error":"bad-hex","message": e.to_string()})) },
            };
            let att = match tx::TxAttestation::decode(&mut &bytes[..]) {
                Ok(a) => a,
                Err(e) => return Ok(serde_json::json!({"error":"bad-attestation","message": format!("{}", e)})),
            };
            let guard = pose_verifier.lock().unwrap();
            if let Some(verifier) = &*guard {
                let new = verifier.apply_attestation(&att);
                Ok(serde_json::json!({"applied": new}))
            } else {
                Ok(serde_json::json!({"error":"verifier-not-ready"}))
            }
        })?;
    }

    {
        let pose_verifier = pose_verifier.clone();
        module.register_method("pose_threshold", move |_params, _| {
            let guard = pose_verifier.lock().unwrap();
            if let Some(verifier) = &*guard {
                Ok(serde_json::json!({"threshold": verifier.eligible_threshold()}))
            } else {
                Ok(serde_json::json!({"error":"verifier-not-ready"}))
            }
        })?;
    }

    {
        let pose_verifier = pose_verifier.clone();
        module.register_method("pose_eligible", move |_params, _| {
            let guard = pose_verifier.lock().unwrap();
            if let Some(verifier) = &*guard {
                let list: Vec<String> = verifier.eligible_ordered().into_iter().map(|h| format!("0x{}", hex::encode(h))).collect();
                Ok(serde_json::json!({"eligible": list}))
            } else {
                Ok(serde_json::json!({"error":"verifier-not-ready"}))
            }
        })?;
    }
	// Extend this RPC with a custom API by using the following syntax.
	// `YourRpcStruct` should have a reference to a client, which is needed
	// to call into the runtime.
	// `module.merge(YourRpcTrait::into_rpc(YourRpcStruct::new(ReferenceToClient, ...)))?;`

	Ok(module)
}
