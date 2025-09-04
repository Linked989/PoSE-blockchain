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
use sc_client_api::{HeaderBackend, BlockBackend, Finalizer};
use sp_consensus_pow::Seal as RawSeal;
use sc_consensus_pow::{Error as PowError, PowAlgorithm};
use sp_runtime::SaturatedConversion;
use sp_core::blake2_256;
// IoT epoch/slot consensus scaffold
use sha3pow::{
    Engine as IoTEngine,
    Committee, EpochSeed, EpochState, AvailabilityCertificate, BlockHeader,
    Phase, header_hash, derive_epoch_seed, new_epoch,
    SLOT_MS, SLOTS_PER_EPOCH,
    QuorumCertificate, ProposalMsg, BatchHeader, VoteMsg, QCMsg, MempoolPolicy, PreEndorseAggregator, encode_qc_ac_digest, PreEndorsement, has_two_thirds_preendorsement,
};
use parity_scale_codec::{Encode, Decode};
use std::sync::atomic::{AtomicU32, Ordering};
use futures::StreamExt;

const HOTSTUFF_PROTOCOL: &str = "/iot-hotstuff/1";

enum NetInbound { Proposal(ProposalMsg), Vote(VoteMsg), QC(QCMsg), PreEndorse(PreEndorsement) }


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

/// Minimal consensus algorithm that accepts any provided seal and fixed difficulty.
#[derive(Clone)]
pub struct AcceptAllPow;

impl<B: BlockT<Hash = H256>> PowAlgorithm<B> for AcceptAllPow {
    type Difficulty = U256;

    fn difficulty(&self, _parent: B::Hash) -> Result<Self::Difficulty, PowError<B>> {
        Ok(U256::from(1))
    }

    fn verify(
        &self,
        _parent: &sp_runtime::generic::BlockId<B>,
        _pre_hash: &H256,
        _pre_digest: Option<&[u8]>,
        _seal: &RawSeal,
        _difficulty: Self::Difficulty,
    ) -> Result<bool, PowError<B>> {
        // Accept any seal without verification.
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
            sc_consensus_pow::PowBlockImport<
                Block,
                Arc<FullClient>,
                FullClient,
                FullSelectChain,
                AcceptAllPow,
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

    let pow_block_import = sc_consensus_pow::PowBlockImport::new(
        client.clone(),
        client.clone(),
        AcceptAllPow,
        u32::MAX,                       // effectively disable inherent checks
        select_chain.clone(),
        move |_, ()| async move {
            let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
            Ok(timestamp)
        },
        can_author_with
      );
      
      let import_queue = sc_consensus_pow::import_queue(
        Box::new(pow_block_import.clone()),
        None,
        AcceptAllPow,  // minimal accept-all algorithm
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
        other: (telemetry, pow_block_import),
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
        other: (mut telemetry, pow_block_import),
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
    
    // Register IoT HotStuff notifications protocol on the network
    let hotstuff_set = sc_network::config::NonDefaultSetConfig {
        notifications_protocol: HOTSTUFF_PROTOCOL.into(),
        max_notification_size: 1024 * 1024,
        set_config: sc_network::config::SetConfig {
            in_peers: 25,
            out_peers: 25,
            reserved_nodes: Vec::new(),
            non_reserved_mode: sc_network::config::NonReservedPeerMode::Accept,
        },
        fallback_names: Vec::new(),
    };
    let mut config = config;
    config.network.extra_sets.push(hotstuff_set);

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
    // Notifications peers set and receiver task for HotStuff
    let hotstuff_peers: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<sc_network::PeerId>>> = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
    let network_for_notif = network.clone();
    let peers_for_notif = hotstuff_peers.clone();
    let protocol_for_notif = HOTSTUFF_PROTOCOL;
    let (tx_net_main, rx_net_main) = std::sync::mpsc::channel::<NetInbound>();
    task_manager.spawn_handle().spawn("hotstuff-notifs", None, async move {
        let mut events = network_for_notif.event_stream("hotstuff".into());
                while let Some(ev) = events.next().await {
            match ev {
                sc_network::Event::NotificationStreamOpened { remote, protocol, .. } => {
                    if protocol == protocol_for_notif { peers_for_notif.lock().unwrap().insert(remote); }
                }
                sc_network::Event::NotificationStreamClosed { remote, protocol, .. } => {
                    if protocol == protocol_for_notif { peers_for_notif.lock().unwrap().remove(&remote); }
                }
                sc_network::Event::NotificationsReceived { messages, .. } => {
                    for (p, data) in messages {
                        if p != protocol_for_notif || data.is_empty() { continue; }
                        match data[0] {
                            1 => if let Ok(m) = ProposalMsg::decode(&mut &data[1..]) { let _ = tx_net_main.send(NetInbound::Proposal(m)); },
                            2 => if let Ok(m) = VoteMsg::decode(&mut &data[1..]) { let _ = tx_net_main.send(NetInbound::Vote(m)); },
                            3 => if let Ok(m) = QCMsg::decode(&mut &data[1..]) { let _ = tx_net_main.send(NetInbound::QC(m)); },
                            4 => if let Ok(m) = PreEndorsement::decode(&mut &data[1..]) { let _ = tx_net_main.send(NetInbound::PreEndorse(m)); },
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    });


	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config,
			task_manager.spawn_handle(),
			client.clone(),
			network.clone(),
		);
	}

    let role = config.role.clone();
    let _force_authoring = config.force_authoring;
    let _backoff_authoring_blocks: Option<()> = None;
    let prometheus_registry = config.prometheus_registry().cloned();

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

    if role.is_authority() {
        let proposer_factory = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|x| x.handle()),
        );

        let can_author_with =
            sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

        let (_worker, worker_task) = sc_consensus_pow::start_mining_worker(
            Box::new(pow_block_import),
            client.clone(),
            select_chain,
            AcceptAllPow,
            proposer_factory,
            network.clone(),
            network.clone(),
            None,
            move |_, ()| async move {
                let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
                Ok(timestamp)
            },
            // time to wait for a new block before starting to mine a new one
            Duration::from_secs(10),
            // how long to take to actually build the block (i.e. executing extrinsics)
            Duration::from_secs(10),
            can_author_with,
        );


        task_manager
            .spawn_essential_handle()
            .spawn_blocking("accept-all-consensus", Some("block-authoring"), worker_task);

        // Slot/Epoch driver: produce a block only when this node is leader for the slot.
        let client_for_slot = client.clone();
        let pool_for_slot = transaction_pool.clone();
        let worker_for_slot = _worker.clone();
        let hotstuff_peers_for_slot = hotstuff_peers.clone();
        let avg_tx_size_for_policy = avg_tx_size_bytes.clone();
        thread::spawn(move || {
            // Capture network for notifications broadcast
            let network = network.clone();
            let hotstuff_peers = hotstuff_peers_for_slot;
            // Inbound notifications channel
            let rx_net = rx_net_main;
            let client_local = client_for_slot.clone();
            // Device identity (stub): use genesis hash as a stand-in for a node/device id.
            let me: H256 = client_local
                .block_hash(0)
                .ok()
                .flatten()
                .unwrap_or_else(|| H256::repeat_byte(1));

            // Single-node committee for now (threshold = 1)
            let committee = Committee { group_id: 0, members: vec![me], threshold: 1 };

            // Seed from genesis; rotate every epoch using derived seed from last commit QC.
            let seed = EpochSeed(
                client_local
                    .block_hash(0)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| H256::repeat_byte(2)),
            );
            let mut epoch_index: u64 = 0;
            let mut slot_index: u64 = 0;
            let mut engine = IoTEngine::new(EpochState { epoch: epoch_index, seed, committee });
            // Install a simple mempool policy (currently returns empty batches; easy to extend)
            struct PoolMempoolPolicy<TPool> { pool: std::sync::Arc<TPool>, avg_tx_size: std::sync::Arc<AtomicU32> }
            impl<TPool> MempoolPolicy for PoolMempoolPolicy<TPool>
            where TPool: sc_transaction_pool_api::TransactionPool + Send + Sync + 'static
            {
                fn select_batches(
                    &self,
                    _preendorse: &PreEndorseAggregator,
                    _committee: &sha3pow::Committee,
                    target_bytes: u32,
                ) -> Vec<BatchHeader> {
                    // Use pool.status() as a portable way to estimate ready extrinsics.
                    let status = self.pool.status();
                    let ready = status.ready as u32;
                    if ready == 0 { return Vec::new(); }
                    // Approximate packing: cap by target_bytes assuming 250 bytes avg per tx.
                    let mut avg_tx = self.avg_tx_size.load(Ordering::Relaxed);
                    if avg_tx == 0 { avg_tx = 250; }
                    let max_txs = (target_bytes / avg_tx).max(1);
                    let take = ready.min(max_txs);
                    let bytes = take.saturating_mul(avg_tx);
                    // Build a single batch header for now. Merkle root is placeholder (hash of bytes||txs).
                    let mut mix = Vec::new();
                    mix.extend_from_slice(&bytes.to_le_bytes());
                    mix.extend_from_slice(&take.to_le_bytes());
                    let root = H256::from(blake2_256(&mix));
                    vec![BatchHeader { id: root, merkle_root: root, size_bytes: bytes, tx_count: take }]
                }
            }
            engine.mempool = Box::new(PoolMempoolPolicy { pool: pool_for_slot.clone(), avg_tx_size: avg_tx_size_for_policy });

            
            // Helper to broadcast notifications to all connected peers for our protocol
            let mut broadcast = |tag: u8, payload: Vec<u8>| {
                let mut buf = Vec::with_capacity(1 + payload.len());
                buf.push(tag);
                buf.extend_from_slice(&payload);
                let peers = hotstuff_peers.lock().unwrap().clone();
                for peer in peers { let _ = network.write_notification(peer, HOTSTUFF_PROTOCOL.into(), buf.clone()); }
            };

            // Metrics: local counters
            let mut last_metrics = std::time::Instant::now();
            let mut proposals_made: u64 = 0;
            let mut qcs_formed: u64 = 0;
            let mut prepare_qcs: u64 = 0;
            let mut precommit_qcs: u64 = 0;
            let mut commit_qcs: u64 = 0;
            let mut commits_made: u64 = 0;
            let mut last_committed: Option<H256> = None;
            // Track previous headers for pipelined phases across blocks
            let mut prev_header: Option<BlockHeader> = None;
            let mut prev_prev_header: Option<BlockHeader> = None;
            let mut last_commit_qc: Option<QuorumCertificate> = None;
            let mut view_timeouts: u64 = 0;
            let mut packed_last_bytes: u32 = 0;
            let mut packed_last_txs: u32 = 0;


            loop {
                // Drain inbound gossip and handle
                while let Ok(msg) = rx_net.try_recv() {
                    match msg {
                        NetInbound::Proposal(p) => {
                            // Minimal DA enforcement on inbound proposals
                            if !has_two_thirds_preendorsement(&p, &engine.preendorse, &engine.epoch.committee) {
                                continue;
                            }
                            // Handle proposal from network
                            if let Some(v) = engine.handle_proposal(me, &p) {
                                // Broadcast our Prepare vote
                                broadcast(2, v.encode());
                                let _ = engine.handle_vote(v);
                            }
                        }
                        NetInbound::Vote(v) => {
                            if let Some(qc) = engine.handle_vote(v.clone()) { broadcast(3, qc.encode()); let _ = engine.handle_qc(qc); }
                        }
                        NetInbound::QC(q) => {
                            if let Some(committed) = engine.handle_qc(q) {
                                if let Some(last) = committed.last() {
                                    if client_local.header(&sp_runtime::generic::BlockId::<Block>::Hash(*last)).ok().flatten().is_some() {
                                        let info_fin = client_local.info();
                                        if info_fin.finalized_hash != *last {
                                            let bid = sp_runtime::generic::BlockId::<Block>::Hash(*last);
                                            let _ = client_local.finalize_block(bid, None, true);
                                        }
                                    }
                                }
                            } }
                        NetInbound::PreEndorse(pre) => { engine.preendorse.add(pre); }
                    }
                }
                // Pacemaker: rotate view on timeout
                if engine.view_timed_out() {
                    engine.next_view();
                    view_timeouts = view_timeouts.saturating_add(1);
                }

                // Leadership (use view-based leader for realism)
                let leader = engine.leader_for_view(engine.view);
                if leader == me {
                    // Build header metadata for this proposal
                    let info = client_local.chain_info();
                    let parent_hash = info.best_hash;
                    let number_u64: u64 = info.best_number.saturated_into();
                    let payload_hash = H256::from(blake2_256(parent_hash.as_bytes()));

                    if let Some(header) = engine.propose_header(parent_hash, number_u64 + 1, slot_index, payload_hash) {
                        // Build proposal with batches selected by policy (currently empty)
                        let batches: Vec<BatchHeader> = engine.select_batches();
                        if let Some(bh) = batches.first() { packed_last_bytes = bh.size_bytes; packed_last_txs = bh.tx_count; }
                        let proposal: ProposalMsg = engine.build_proposal(parent_hash, number_u64 + 1, slot_index, &batches);
                        proposals_made = proposals_made.saturating_add(1);
                        // Broadcast proposal
                        broadcast(1, proposal.encode());

                         // On building a proposal, broadcast pre-endorsements for each batch (dev DA path)
                        for b in &batches {
                            let pre = PreEndorsement { batch_id: b.id, voter: me, sig_share: Vec::new() };
                            engine.preendorse.add(pre.clone());
                            broadcast(4, pre.encode());
                        }

                        // Minimal DA enforcement: require >= 2/3 pre-endorsement coverage across batches
                        if !has_two_thirds_preendorsement(&proposal, &engine.preendorse, &engine.epoch.committee) {
                            continue;
                        }
                        // Handle proposal: produce Prepare vote if safe and available for CURRENT block
                        if let Some(prepare_vote) = engine.handle_proposal(me, &proposal) {
                            // Broadcast our prepare vote
                            broadcast(2, prepare_vote.encode());
                            if let Some(prepare_qc) = engine.handle_vote(prepare_vote) {
                                qcs_formed = qcs_formed.saturating_add(1);
                                prepare_qcs = prepare_qcs.saturating_add(1);
                                broadcast(3, prepare_qc.encode());
                                let _ = engine.handle_qc(prepare_qc);
                            }
                        }

                        // For PREVIOUS block: attempt PreCommit phase in this slot
                        if let Some(ph) = &prev_header {
                            let precommit_v = engine.make_vote(me, Phase::PreCommit, ph);
                            broadcast(2, VoteMsg(precommit_v.clone()).encode());
                            if let Some(precommit_qc) = engine.handle_vote(VoteMsg(precommit_v)) {
                                qcs_formed = qcs_formed.saturating_add(1);
                                precommit_qcs = precommit_qcs.saturating_add(1);
                                broadcast(3, precommit_qc.encode());
                                let _ = engine.handle_qc(precommit_qc);
                            }
                        }

                        // For GRANDPARENT block: attempt Commit phase in this slot
                        if let Some(gph) = &prev_prev_header {
                            let commit_v = engine.make_vote(me, Phase::Commit, gph);
                            broadcast(2, VoteMsg(commit_v.clone()).encode());
                            if let Some(commit_qc) = engine.handle_vote(VoteMsg(commit_v)) {
                                qcs_formed = qcs_formed.saturating_add(1);
                                commit_qcs = commit_qcs.saturating_add(1);
                                broadcast(3, commit_qc.encode());
                                if let Some(committed) = engine.handle_qc(commit_qc.clone()) {
                                    commits_made = commits_made.saturating_add(committed.len() as u64);
                                    if let Some(last) = committed.last() { 
                                        last_committed = Some(*last);
                                        // Finalize the last committed block (HotStuff 3-chain grandparent), if known and not already finalized.
                                        if client_local.header(&sp_runtime::generic::BlockId::<Block>::Hash(*last)).ok().flatten().is_some() {
                                            let info_fin = client_local.info();
                                            if info_fin.finalized_hash != *last {
                                                let bid = sp_runtime::generic::BlockId::<Block>::Hash(*last);
                                                let _ = client_local.finalize_block(bid, None, true);
                                            }
                                        }
                                    }
                                }
                                // Prepare digest payload for logging/embedding
                                let ac = AvailabilityCertificate { block_id: header_hash(&proposal.header), batch_ids: batches.iter().map(|b| b.id).collect() };
                                let QCMsg(q) = commit_qc.clone();
                                let digest_payload = encode_qc_ac_digest(&q, &ac);
                                log::info!("Consensus digest (QC+AC) bytes: {}", digest_payload.len());
                                let QCMsg(qc) = commit_qc.clone();
                                if qc.phase == Phase::Commit { last_commit_qc = Some(qc); }
                            }
                        }

                        // Shift headers for next slot pipeline
                        prev_prev_header = prev_header.take();
                        prev_header = Some(proposal.header.clone());

                        // Trigger a block build/import via the worker path.
                        let worker = worker_for_slot.clone();
                        if worker.metadata().is_some() {
                            let _ = futures::executor::block_on(worker.submit(Vec::<u8>::new()));
                        }
                    }
                }

                // Extra consensus metrics every 2 seconds
                if last_metrics.elapsed().as_secs_f64() >= 2.0 {
                    let leader = engine.leader_for_view(engine.view);
                    log::info!(
                        "✅✅✅ Consensus metrics — epoch: {}, slot: {}, view: {}, leader: {:?}, proposals: {}, QCs: {} [prepare:{}, precommit:{}, commit:{}], commits: {}, last_commit: {:?}, view_timeouts: {}, packed: {}B/{}tx",
                        engine.epoch.epoch,
                        slot_index,
                        engine.view,
                        leader,
                        proposals_made,
                        qcs_formed,
                        prepare_qcs, precommit_qcs, commit_qcs,
                        commits_made,
                        last_committed,
                        view_timeouts,
                        packed_last_bytes, packed_last_txs
                    );
                    last_metrics = std::time::Instant::now();
                }

                // Epoch rotation
                if (slot_index + 1) % SLOTS_PER_EPOCH == 0 {
                    epoch_index = epoch_index.saturating_add(1);
                    let next_seed = if let Some(qc) = &last_commit_qc {
                        derive_epoch_seed(qc, None)
                    } else {
                        engine.epoch.seed
                    };
                    engine.epoch = new_epoch(epoch_index, next_seed, engine.epoch.committee.clone());
                    slot_index = 0;
                } else {
                    slot_index = slot_index.saturating_add(1);
                }

                thread::sleep(Duration::from_millis(SLOT_MS));
            }
        });
    }

    network_starter.start_network();

    // Spawn a lightweight metrics logger: block height, extrinsics in new blocks, user-tx TPS and totals.
    {
        let client_for_metrics = client.clone();
        // Also track how many peers have an open HotStuff notifications stream
        let hotstuff_peers_for_metrics = hotstuff_peers.clone();
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
                let hs_peers = hotstuff_peers_for_metrics.lock().map(|s| s.len()).unwrap_or(0);
                // Attempt to parse any IQC (QC+AC) digest from the best block header for on-chain visibility (dev aid)
                {
                    // Validate any IQC digest found in the best header (dev on-chain validation)
                    let bid = sp_runtime::generic::BlockId::<Block>::Hash(info.best_hash);
                    if let Ok(Some(header)) = client_for_metrics.header(&bid) {
                        let header_hash = header.hash();
                        for logi in header.digest.logs() {
                            if let sp_runtime::generic::DigestItem::Other(raw) = logi {
                                if raw.len() >= 4 && &raw[0..4] == sha3pow::DIGEST_TAG {
                                    let mut data = &raw[4..];
                                    let qc = sha3pow::QuorumCertificate::decode(&mut data);
                                    let ac = qc.as_ref().ok().and_then(|_| sha3pow::AvailabilityCertificate::decode(&mut data).ok());
                                    match (qc, ac) {
                                        (Ok(qc), Some(ac)) => {
                                            let ok_ids = qc.block_id == header_hash && ac.block_id == header_hash;
                                            log::info!(
                                                "IQC digest detected: phase={:?}, batches={}, ids_match_header={} (best #{})",
                                                qc.phase,
                                                ac.batch_ids.len(),
                                                ok_ids,
                                                best
                                            );
                                        }
                                        _ => {
                                            log::warn!("IQC digest found but failed to decode QC/AC (best #{})", best);
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }

                log::info!(
                    "Node metrics — height: {}, extrinsics: {}, user_extrinsics: {}, TPS: {:.2}, user_TPS: {:.2}, hotstuff_peers: {}",
                    best,
                    extrinsics_in_interval,
                    user_extrinsics_in_interval,
                    tps,
                    user_tps,
                    hs_peers
                );



                last_best = best;
                last_instant = std::time::Instant::now();
                thread::sleep(Duration::from_secs(5));
            }
        });
        
    }


    // Dev-only minimal finalizer: opt-in via DEV_FINALIZER=1 (leader only).
    let dev_finalizer_enabled = std::env::var("DEV_FINALIZER").ok().as_deref() == Some("1");
    if dev_finalizer_enabled {
        log::info!("DEV_FINALIZER=1: this node will finalize blocks (others should not)");
        let client_for_finality = client.clone();
        thread::spawn(move || {
            loop {
                let info = client_for_finality.info();
                if info.best_hash != info.finalized_hash {
                    let best_id = sp_runtime::generic::BlockId::<Block>::Hash(info.best_hash);
                    let _ = client_for_finality.finalize_block(best_id, None, true);
                }
                thread::sleep(Duration::from_secs(5));
            }
        });
    }

    Ok(task_manager)
}
