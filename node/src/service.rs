//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use node_template_runtime::{self, opaque::Block, RuntimeApi};
use sc_client_api::ExecutorProvider;
pub use sc_executor::NativeElseWasmExecutor;
use sc_keystore::LocalKeystore;
use sc_service::{error::Error as ServiceError, Configuration, TaskManager};
use sc_telemetry::{Telemetry, TelemetryWorker};
use std::{sync::Arc, time::Duration};
use sp_inherents::CreateInherentDataProviders;
use std::thread;
use sp_core::{H256, U256};
use sp_runtime::traits::{Block as BlockT, One};
use sc_client_api::{HeaderBackend, BlockBackend};
use sp_consensus_seal::Seal as RawSeal;
use sc_consensus_seal::{Error as ConsensusError, PowAlgorithm as SealAlgorithm, PowBlockImport as SealBlockImport};
// Removed: SaturatedConversion not used in pure no-seal mode
use std::sync::atomic::{AtomicU32, Ordering};
use parity_scale_codec::Encode;


// Our native executor instance.
pub struct ExecutorDispatch;

impl sc_executor::NativeExecutionDispatch for ExecutorDispatch {
	/// Only enable the benchmarking host functions when we actually want to benchmark.
	#[cfg(feature = "runtime-benchmarks")]
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
	/// Otherwise we only use the default Substrate host functions.
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ExtendHostFunctions = ();

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		node_template_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		node_template_runtime::native_version()
	}
}

pub(crate) type FullClient =
	sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<ExecutorDispatch>>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

/// Minimal accept-all seal algorithm with fixed difficulty.
#[derive(Clone)]
pub struct AcceptAllSeal;

impl<B: BlockT<Hash = H256>> SealAlgorithm<B> for AcceptAllSeal {
    type Difficulty = U256;

    fn difficulty(&self, _parent: B::Hash) -> Result<Self::Difficulty, ConsensusError<B>> {
        Ok(U256::from(1))
    }

    fn verify(
        &self,
        _parent: &sp_runtime::generic::BlockId<B>,
        _pre_hash: &H256,
        _pre_digest: Option<&[u8]>,
        _seal: &RawSeal,
        _difficulty: Self::Difficulty,
    ) -> Result<bool, ConsensusError<B>> {
        // Accept any seal without verification
        Ok(true)
    }
}

pub fn new_partial(
    config: &Configuration,
) -> Result<
    sc_service::PartialComponents<
        FullClient,
        FullBackend,
        FullSelectChain,
        sc_consensus::DefaultImportQueue<Block, FullClient>,
        sc_transaction_pool::FullPool<Block, FullClient>,
        (
            Option<Telemetry>,
            SealBlockImport<
                Block,
                Arc<FullClient>,
                FullClient,
                FullSelectChain,
                AcceptAllSeal,
                impl sp_consensus::CanAuthorWith<Block>,
                impl CreateInherentDataProviders<Block, ()>,
            >,
        ),
    >,
    ServiceError,
> {
	if config.keystore_remote.is_some() {
		return Err(ServiceError::Other("Remote Keystores are not supported.".into()))
	}

	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let executor = NativeElseWasmExecutor::<ExecutorDispatch>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
		config.runtime_cache_size,
	);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, _>(
			config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	let client = Arc::new(client);

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_essential_handle(),
        client.clone(),
    );

    let can_author_with =
    sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

    let block_import = SealBlockImport::new(
        client.clone(),
        client.clone(),
        AcceptAllSeal,
        u32::MAX,                       // effectively disable inherent checks
        select_chain.clone(),
        move |_, ()| async move {
            let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
            Ok(timestamp)
        },
        can_author_with
      );
      
      let import_queue = sc_consensus_seal::import_queue(
        Box::new(block_import.clone()),
        None,
        AcceptAllSeal,  // minimal accept-all algorithm
        &task_manager.spawn_essential_handle(),
        config.prometheus_registry(),
      )?;

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: (telemetry, block_import),
    })
}

fn remote_keystore(_url: &String) -> Result<Arc<LocalKeystore>, &'static str> {
	// FIXME: here would the concrete keystore be built,
	//        must return a concrete type (NOT `LocalKeystore`) that
	//        implements `CryptoStore` and `SyncCryptoStore`
	Err("Remote Keystore not supported.")
}

/// Builds a new service for a full client.
pub fn new_full(config: Configuration) -> Result<TaskManager, ServiceError> {
    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        mut keystore_container,
        select_chain,
        transaction_pool,
        other: (mut telemetry, block_import),
    } = new_partial(&config)?;

	if let Some(url) = &config.keystore_remote {
		match remote_keystore(url) {
			Ok(k) => keystore_container.set_remote_keystore(k),
			Err(e) =>
				return Err(ServiceError::Other(format!(
					"Error hooking up remote keystore for {}: {}",
					url, e
				))),
		};
	}
    let avg_tx_size_bytes = std::sync::Arc::new(AtomicU32::new(250));

    let (network, system_rpc_tx, network_starter) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            block_announce_validator_builder: None,
            warp_sync: None,
        })?;


	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config,
			task_manager.spawn_handle(),
			client.clone(),
			network.clone(),
		);
	}

    // In pure no-seal mode we don't author, so role/authoring settings are not used.

	let rpc_extensions_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();

		Box::new(move |deny_unsafe, _| {
			let deps =
				crate::rpc::FullDeps { client: client.clone(), pool: pool.clone(), deny_unsafe };
			crate::rpc::create_full(deps).map_err(Into::into)
		})
	};

	let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network: network.clone(),
		client: client.clone(),
		keystore: keystore_container.sync_keystore(),
		task_manager: &mut task_manager,
		transaction_pool: transaction_pool.clone(),
		rpc_builder: rpc_extensions_builder,
		backend,
		system_rpc_tx,
		config,
		telemetry: telemetry.as_mut(),
	})?;

    // In pure no-seal mode, do not author blocks at all (no workers, no leader election).

    network_starter.start_network();

    // Spawn entropy-based leader selection once enough peers are present (>= 4)
    {
        let network_for_leader = network.clone();
        let client_for_leader = client.clone();
        // Optional external round nonce from env (shared across nodes per spawn)
        let external_round_nonce = std::env::var("LEADER_ROUND_NONCE")
            .ok()
            .map(|s| sp_core::blake2_256(s.as_bytes()));

        crate::modules::entropy_leader::spawn_entropy_leader(
            move || {
                // Collect connected peer IDs from network state (include local peer)
                let mut ids: Vec<String> = Vec::new();
                if let Ok(state) = futures::executor::block_on(network_for_leader.network_state()) {
                    ids.extend(state.connected_peers.keys().cloned());
                }
                ids.push(network_for_leader.local_peer_id().to_base58());
                ids
            },
            move || {
                // Use best block hash as shared seed across nodes
                let info = client_for_leader.info();
                let best = info.best_number;
                if let Ok(Some(h)) = client_for_leader.block_hash(best) {
                    h.as_fixed_bytes().clone()
                } else {
                    H256::zero().as_fixed_bytes().clone()
                }
            },
            4,                          // minimum group size
            Duration::from_secs(10),    // election interval
            "entropy-leader",
            external_round_nonce,
        );
    }

    // Spawn a lightweight metrics logger: block height, extrinsics in new blocks, user-tx TPS and totals.
    {
        let client_for_metrics = client.clone();
        let avg_tx_size_for_metrics = avg_tx_size_bytes.clone();
        thread::spawn(move || {
            let mut last_best = client_for_metrics.info().best_number;
            let mut last_instant = std::time::Instant::now();
            loop {
                let info = client_for_metrics.info();
                let best = info.best_number;
            // Count extrinsics in newly imported blocks since last sample
            let mut extrinsics_in_interval: u64 = 0;
            let mut user_extrinsics_in_interval: u64 = 0;
                let mut cur = last_best.saturating_add(One::one());
                while cur <= best {
                    if let Ok(Some(hash)) = client_for_metrics.block_hash(cur) {
                        let bid = sp_runtime::generic::BlockId::<Block>::Hash(hash);
                        if let Ok(Some(signed)) = client_for_metrics.block(&bid) {
                            let exts = signed.block.extrinsics();
                            let count = exts.len() as u64;
                            extrinsics_in_interval = extrinsics_in_interval.saturating_add(count);
                            let total_bytes: usize = exts.iter().map(|e| e.encode().len()).sum();
                            let avg = if exts.len()>0 { (total_bytes / exts.len()) as u32 } else { 0 };
                            if avg > 0 { avg_tx_size_for_metrics.store(avg, Ordering::Relaxed); }
                            let user = count.saturating_sub(1);
                            user_extrinsics_in_interval = user_extrinsics_in_interval.saturating_add(user);
                        }
                    }
                    cur = cur.saturating_add(One::one());
                }

                let secs = last_instant.elapsed().as_secs_f64();
                let tps = if secs > 0.0 { extrinsics_in_interval as f64 / secs } else { 0.0 };
                let user_tps = if secs > 0.0 { user_extrinsics_in_interval as f64 / secs } else { 0.0 };
                log::info!(
                    "Node metrics — height: {}, extrinsics: {}, user_extrinsics: {}, TPS: {:.2}, user_TPS: {:.2}",
                    best,
                    extrinsics_in_interval,
                    user_extrinsics_in_interval,
                    tps,
                    user_tps
                );



                last_best = best;
                last_instant = std::time::Instant::now();
                thread::sleep(Duration::from_secs(5));
            }
        });
        
    }


    // No dev finalizer in pure no-seal mode.

    Ok(task_manager)
}
