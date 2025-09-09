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
pub fn new_full(mut config: Configuration) -> Result<TaskManager, ServiceError> {
    // Extract prometheus registry early as config will be moved into spawn_tasks
    let prometheus_registry = config.prometheus_registry().cloned();
    // Register custom P2P notification protocols for PoSE
    {
        use sc_network::config::{NonDefaultSetConfig, SetConfig, NonReservedPeerMode};
        let mut push_proto = |name: &'static str, max_size: u64| {
            let set = NonDefaultSetConfig {
                notifications_protocol: name.into(),
                max_notification_size: max_size,
                fallback_names: Vec::new(),
                set_config: SetConfig {
                    in_peers: 25,
                    out_peers: 25,
                    reserved_nodes: Vec::new(),
                    non_reserved_mode: NonReservedPeerMode::Accept,
                },
            };
            if !config.network.extra_sets.iter().any(|s| s.notifications_protocol == name) {
                config.network.extra_sets.push(set);
            }
        };
        push_proto("/pose/tx_attest/1", 2 * 1024);
        push_proto("/pose/proposal/1", 8 * 1024);
    }
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

    // Shared handle to inject the verifier into RPC after network init
    let pose_verifier_handle: std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<crate::modules::tx::TxVerifier>>>> = std::sync::Arc::new(std::sync::Mutex::new(None));
    let rpc_extensions_builder = {
        let client = client.clone();
        let pool = transaction_pool.clone();
        let pose_handle_for_rpc = pose_verifier_handle.clone();

        Box::new(move |deny_unsafe, _| {
            let deps = crate::rpc::FullDeps {
                client: client.clone(),
                pool: pool.clone(),
                deny_unsafe,
                pose_verifier: pose_handle_for_rpc.clone(),
            };
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

    // Spawn basic notification listeners for PoSE topics (stub handlers for now)
    {
        let network_for_notif = network.clone();
        task_manager.spawn_handle().spawn("pose-notifs", None, async move {
            use futures::StreamExt;
            let mut events = network_for_notif.event_stream("pose-notifs");
            while let Some(ev) = events.next().await {
                match ev {
                    sc_network::Event::NotificationsReceived { remote, messages } => {
                        for (protocol, data) in messages {
                            log::debug!(target: "pose-net", "recv from {} proto {} bytes={}", remote, protocol, data.len());
                            // TODO: decode by protocol and apply attestation/proposal
                        }
                    }
                    _ => {}
                }
            }
        });
    }

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

    // Manual-seal worker (dev): build/import blocks on command
    let manual_seal_tx = {
        use futures::channel::mpsc;
        use sc_consensus_manual_seal as manual;
        let proposer_factory = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref().map(|r| r),
            None,
        );
        let (tx, rx) = mpsc::unbounded();
        let commands_stream = rx;
        let client_for_seal = client.clone();
        let pool_for_seal = transaction_pool.clone();
        let select_chain_for_seal = select_chain.clone();
        let essential_handle = task_manager.spawn_essential_handle();
        // Move block_import into the task (no clone)
        let block_import_for_seal = block_import;
        essential_handle.spawn_blocking("manual-seal", None, manual::run_manual_seal(manual::ManualSealParams {
            block_import: block_import_for_seal,
            env: proposer_factory,
            client: client_for_seal,
            pool: pool_for_seal,
            commands_stream,
            select_chain: select_chain_for_seal,
            consensus_data_provider: None,
            create_inherent_data_providers: move |_, _| async move {
                let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
                Ok((timestamp,))
            },
        }));
        std::sync::Arc::new(std::sync::Mutex::new(tx))
    };

    // Spawn pacemaker (per-slot timing/leader expectations)
    {
        let network_for_pace = network.clone();
        let client_for_pace = client.clone();
        let network_for_my_id = network.clone();
        let external_round_nonce = std::env::var("LEADER_ROUND_NONCE")
            .ok()
            .map(|s| sp_core::blake2_256(s.as_bytes()));

        // Build proposer and pass callback to pacemaker
        let tx_handle = pose_verifier_handle.clone();
        let proposer = {
            let guard = tx_handle.lock().unwrap();
            if let Some(v) = &*guard {
                let ed_seed = std::env::var("POSE_ATTEST_SEED").ok();
                Some(std::sync::Arc::new(crate::modules::proposal::Proposer::new(
                    client.clone(),
                    v.clone(),
                    ed_seed,
                    1024 * 1024, // 1 MB block cap for bytes
                    10_000_000,  // dummy weight cap
                )))
            } else { None }
        };
        // Build sealer using verifier if available
        let sealer = {
            let guard = pose_verifier_handle.lock().unwrap();
            if let Some(v) = &*guard {
                let ed_seed = std::env::var("POSE_ATTEST_SEED").ok();
                Some(std::sync::Arc::new(crate::modules::seal::Sealer::new(
                    client.clone(), v.clone(), ed_seed, 1024*1024, 10_000_000,
                )))
            } else { None }
        };
        // When leader: trigger sealing and a manual-seal block import
        let on_leader: Option<std::sync::Arc<dyn Fn(u64, [u8;32]) + Send + Sync>> = sealer.as_ref().map(|s| {
            let s = s.clone();
            let tx = manual_seal_tx.clone();
            std::sync::Arc::new(move |slot, seed_e| {
                s.seal_slot(seed_e, slot);
                // Ask manual-seal to build/import a block now
                let guard = tx.lock().unwrap();
                let _ = guard.unbounded_send(sc_consensus_manual_seal::EngineCommand::SealNewBlock {
                    create_empty: true,
                    finalize: false,
                    parent_hash: None,
                    sender: None,
                });
            }) as _
        });

        crate::modules::pacemaker::spawn_pacemaker(
            crate::modules::pacemaker::PacemakerConfig::default(),
            move || {
                let mut ids: Vec<String> = Vec::new();
                if let Ok(state) = futures::executor::block_on(network_for_pace.network_state()) {
                    ids.extend(state.connected_peers.keys().cloned());
                }
                ids.push(network_for_pace.local_peer_id().to_base58());
                ids.sort(); ids.dedup();
                ids
            },
            move || {
                let info = client_for_pace.info();
                let best = info.best_number;
                if let Ok(Some(h)) = client_for_pace.block_hash(best) {
                    h.as_fixed_bytes().clone()
                } else { H256::zero().as_fixed_bytes().clone() }
            },
            move || network_for_my_id.local_peer_id().to_base58(),
            external_round_nonce,
            on_leader,
        );
    }

    // Initialize tx verifier manager (API-only for now; hook into pool/gossip next)
    {
        let network_for_tx = network.clone();
        let client_for_tx = client.clone();
        let pose_handle = pose_verifier_handle.clone();
        let group_roster = move || {
            let mut ids: Vec<String> = Vec::new();
            if let Ok(state) = futures::executor::block_on(network_for_tx.network_state()) {
                ids.extend(state.connected_peers.keys().cloned());
            }
            ids.push(network_for_tx.local_peer_id().to_base58());
            ids.sort(); ids.dedup();
            ids
        };
        let my_id = move || network.local_peer_id().to_base58();
        let ed_seed = std::env::var("POSE_ATTEST_SEED").ok();
        let verifier = crate::modules::tx::TxVerifier::new(
            client.clone(),
            1024 * 128,
            Arc::new(group_roster),
            Arc::new(my_id),
            std::env::var("LEADER_ROUND_NONCE").ok().map(|s| sp_core::blake2_256(s.as_bytes())),
            ed_seed,
        );
        let verifier = std::sync::Arc::new(verifier);
        // Make it available to RPC
        *pose_handle.lock().unwrap() = Some(verifier.clone());
        log::info!("tx-verifier initialized (max 128KB, attest topic /pose/tx_attest/1)");

        // Start a simple pool scanner to auto-validate and attestate new txs periodically (stub)
        use futures::StreamExt;
        use sc_service::TransactionPool as _;
        let mut import_stream = transaction_pool.import_notification_stream();
        let verifier_for_scan = verifier.clone();
        task_manager.spawn_handle().spawn("pose-pool-scan", None, async move {
            while let Some(_) = import_stream.next().await {
                // Iterate over ready txs and attempt attestation
                use sc_service::TransactionPool as _;
                let mut iter = transaction_pool.ready();
                while let Some(tx) = iter.next() {
                    // Encode the extrinsic into raw bytes for validation/attestation
                    let bytes: Vec<u8> = parity_scale_codec::Encode::encode(&tx.data);
                    match verifier_for_scan.validate_tx(&bytes) {
                        crate::modules::tx::Verdict::Accept => {
                            if let Some(att) = verifier_for_scan.attestate_if_leader(&bytes) {
                                log::info!(target: "pose-attest", "attested tx {}", hex::encode(att.tx_hash));
                                // TODO: gossip over /pose/tx_attest/1 via notifications
                            }
                        }
                        _ => {}
                    }
                }
            }
        });
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
