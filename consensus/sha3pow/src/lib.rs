//! IoT group-based, epoch/slot consensus scaffold with HotStuff-style commits.
//!
//! This module replaces the previous SHA3 PoW implementation with a slot/epoch
//! driven consensus scaffold designed for IoT committees and HotStuff-like 3-phase
//! commits. It provides types and minimal logic to model:
//! - Epoch and slot scheduling with per-epoch randomness
//! - VRF-like leader selection (deterministic hash-based stub)
//! - HotStuff phases (Prepare, PreCommit, Commit) and Quorum Certificates (QCs)
//! - IoT groups/committees with threshold signatures (stubbed verifier)
//! - Data-availability precondition hooks for voting
//!
//! NOTE: This is a self-contained scaffold intended to be integrated with a
//! networked engine and runtime hooks. It does not perform networking or real
//! BLS/VRF cryptography; those are stubbed to keep the code minimal and
//! buildable within the current repository.

use parity_scale_codec::{Decode, Encode};
use sha3::{Digest, Sha3_256};
use sp_core::H256;
use std::collections::{BTreeMap, BTreeSet};
// Note: time types are referenced fully qualified (std::time::...).

// -----------------------------
// Config and basic types
// -----------------------------

/// Milliseconds per slot (target 250ms)
pub const SLOT_MS: u64 = 250;
/// Slots per epoch (e.g., 100)
pub const SLOTS_PER_EPOCH: u64 = 100;
/// Maximum committee groups active in parallel.
pub const MAX_ACTIVE_GROUPS: usize = 5;
/// Target block size in bytes (~1.5 MB)
pub const BLOCK_TARGET_BYTES: u32 = 1_500_000;

pub type Epoch = u64;
pub type Slot = u64;
pub type GroupId = u32;
pub type DeviceId = H256; // abstract device identifier
pub type View = u64;

/// Randomness seed for an epoch (derived from last QC and optional IoT entropy)
#[derive(Clone, Copy, Encode, Decode, Debug, PartialEq, Eq)]
pub struct EpochSeed(pub H256);

/// HotStuff phases
#[derive(Clone, Copy, Encode, Decode, Debug, PartialEq, Eq)]
pub enum Phase { Prepare, PreCommit, Commit }

/// Minimal block header carrying consensus metadata.
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct BlockHeader {
    pub parent: H256,
    pub number: u64,
    pub epoch: Epoch,
    pub slot: Slot,
    pub proposer: DeviceId,
    pub payload_hash: H256, // Merkle or content hash of tx batch(es)
}

/// Compact quorum certificate (HotStuff-style) with aggregated signature stub.
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct QuorumCertificate {
    pub epoch: Epoch,
    pub slot: Slot,
    pub phase: Phase,
    pub block_id: H256,       // hash(header)
    pub voters_bitmap: Vec<u8>,// compact bitmap for up to N members
    pub agg_sig: Vec<u8>,      // threshold BLS signature (stub)
}

/// Availability certificate for referenced data batches (stub)
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct AvailabilityCertificate {
    pub block_id: H256,
    pub batch_ids: Vec<H256>,
}

/// Alias for data batch identifier referenced by a block proposal.
pub type BatchId = H256;

/// Lightweight batch header (data-availability unit).
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct BatchHeader {
    pub id: BatchId,
    pub merkle_root: H256,
    pub size_bytes: u32,
    pub tx_count: u32,
}

/// IoT group committee information
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct Committee {
    pub group_id: GroupId,
    pub members: Vec<DeviceId>,
    pub threshold: u16, // BLS threshold, e.g., 2f+1
}

impl Committee {
    pub fn contains(&self, id: &DeviceId) -> bool { self.members.iter().any(|m| m == id) }
    pub fn size(&self) -> usize { self.members.len() }
}

/// Epoch state snapshot
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct EpochState {
    pub epoch: Epoch,
    pub seed: EpochSeed,
    pub committee: Committee,
}

// -----------------------------
// Deterministic VRF-like leader schedule (stub)
// -----------------------------

/// Deterministically pick a leader from committee for a given slot using the seed.
/// This is a placeholder for a real VRF-based schedule.
pub fn leader_for_slot(seed: EpochSeed, slot: Slot, committee: &Committee) -> DeviceId {
    let mut hasher = Sha3_256::new();
    hasher.update(seed.0.as_bytes());
    hasher.update(&slot.to_le_bytes());
    // Hash each member into an ordering and pick the minimum
    let mut best: Option<(H256, DeviceId)> = None;
    for m in &committee.members {
        let mut h = hasher.clone();
        h.update(m.as_bytes());
        let out = H256::from_slice(&h.finalize()[..32]);
        if best.as_ref().map(|(b, _)| &out < b).unwrap_or(true) {
            best = Some((out, *m));
        }
    }
    best.map(|(_, m)| m).unwrap_or_else(|| H256::repeat_byte(0))
}

// -----------------------------
// HotStuff commit logic (scaffold)
// -----------------------------

/// Node’s view of safety and liveness for HotStuff
#[derive(Default, Debug, Clone)]
pub struct HotStuffState {
    pub locked_block: Option<H256>,     // highest locked block id
    pub best_qc: Option<QuorumCertificate>,
}

impl HotStuffState {
    pub fn update_lock(&mut self, qc: &QuorumCertificate) {
        if qc.phase == Phase::Commit {
            self.locked_block = Some(qc.block_id);
        }
        self.best_qc = Some(qc.clone());
    }
}

/// Minimal vote structure
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct Vote {
    pub voter: DeviceId,
    pub epoch: Epoch,
    pub slot: Slot,
    pub phase: Phase,
    pub block_id: H256,
    pub sig_share: Vec<u8>, // BLS share (stub)
}

/// Aggregates votes and produces QCs when threshold reached.
#[derive(Default, Debug, Clone)]
pub struct Aggregator {
    pub votes: Vec<Vote>,
}

impl Aggregator {
    pub fn clear(&mut self) { self.votes.clear(); }

    pub fn push(&mut self, v: Vote) { self.votes.push(v); }

    pub fn try_qc(&self, committee: &Committee, phase: Phase, epoch: Epoch, slot: Slot, block_id: H256) -> Option<QuorumCertificate> {
        // Count unique voters in committee for the same (epoch, slot, phase, block_id)
        let mut set = BTreeSet::new();
        for v in self.votes.iter() {
            if v.epoch == epoch && v.slot == slot && v.phase == phase && v.block_id == block_id && committee.contains(&v.voter) {
                set.insert(v.voter);
            }
        }
        if set.len() as u16 >= committee.threshold {
            // Stub aggregation: concat shares and set bitmap (all ones up to len)
            let mut agg = Vec::new();
            for v in self.votes.iter() {
                if v.epoch == epoch && v.slot == slot && v.phase == phase && v.block_id == block_id && committee.contains(&v.voter) {
                    agg.extend_from_slice(&v.sig_share);
                }
            }
            let bitmap_bytes = (committee.size() + 7) / 8;
            let voters_bitmap = vec![0xFF; bitmap_bytes];
            return Some(QuorumCertificate { epoch, slot, phase, block_id, voters_bitmap, agg_sig: agg });
        }
        None
    }

    /// Threshold BLS-style QC aggregation using a crypto provider.
    pub fn try_qc_with_crypto<C: CryptoProvider + ?Sized>(
        &self,
        committee: &Committee,
        phase: Phase,
        epoch: Epoch,
        slot: Slot,
        block_id: H256,
        crypto: &C,
    ) -> Option<QuorumCertificate> {
        let mut shares: Vec<(DeviceId, Vec<u8>)> = Vec::new();
        for v in self.votes.iter() {
            if v.epoch == epoch && v.slot == slot && v.phase == phase && v.block_id == block_id && committee.contains(&v.voter) {
                shares.push((v.voter, v.sig_share.clone()));
            }
        }
        if shares.len() as u16 >= committee.threshold {
            let mut msg = block_id.as_bytes().to_vec();
            msg.push(phase as u8);
            if let Some(agg) = crypto.bls_aggregate_and_verify(committee, &msg, &shares) {
                let bitmap_bytes = (committee.size() + 7) / 8;
                let voters_bitmap = vec![0xFF; bitmap_bytes];
                return Some(QuorumCertificate { epoch, slot, phase, block_id, voters_bitmap, agg_sig: agg });
            }
        }
        None
    }
}

// -----------------------------
// Data availability (stub)
// -----------------------------

#[derive(Default, Debug, Clone)]
pub struct AvailabilityIndex {
    present: BTreeSet<H256>,
}

impl AvailabilityIndex {
    pub fn mark_present(&mut self, batch: H256) { let _ = self.present.insert(batch); }
    pub fn has_all(&self, batches: &[H256]) -> bool { batches.iter().all(|b| self.present.contains(b)) }
}

/// Lightweight DAG to track block proposals and their referenced data batches.
#[derive(Default, Debug, Clone)]
pub struct Dag {
    pub nodes: BTreeMap<H256, DagNode>,
    pub available_batches: BTreeSet<BatchId>,
}

#[derive(Clone, Debug)]
pub struct DagNode {
    pub header: BlockHeader,
    pub batches: Vec<BatchId>,
    pub parents: Vec<H256>,
}

impl Dag {
    pub fn insert_proposal(&mut self, header: BlockHeader, batches: Vec<BatchId>, parents: Vec<H256>) {
        let id = header_hash(&header);
        let node = DagNode { header, batches, parents };
        self.nodes.insert(id, node);
    }

    pub fn mark_batch_available(&mut self, batch: BatchId) { self.available_batches.insert(batch); }

    pub fn block_has_all_batches(&self, block_id: &H256) -> bool {
        if let Some(n) = self.nodes.get(block_id) {
            n.batches.iter().all(|b| self.available_batches.contains(b))
        } else { false }
    }

    pub fn parent_of(&self, block_id: &H256) -> Option<H256> {
        self.nodes.get(block_id).and_then(|n| n.parents.first().cloned())
    }
}

// -----------------------------
// Consensus engine scaffold
// -----------------------------

pub struct Engine {
    pub epoch: EpochState,
    pub state: HotStuffState,
    pub aggregator: Aggregator,
    pub availability: AvailabilityIndex,
    pub crypto: Box<dyn CryptoProvider + Send + Sync>,
    // HotStuff view/timeout (pacemaker)
    pub view: View,
    pub view_start: std::time::Instant,
    pub timeout: std::time::Duration,
    pub locked_view: Option<View>,
    // DAG and QC tracking for commit rule
    pub dag: Dag,
    pub qcs: BTreeMap<H256, QuorumCertificate>,
    pub committed: BTreeSet<H256>,
    // Mempool policy and pre-endorsements
    pub mempool: Box<dyn MempoolPolicy + Send + Sync>,
    pub preendorse: PreEndorseAggregator,
}

impl Engine {
    pub fn new(epoch: EpochState) -> Self {
        Self { 
            epoch,
            state: Default::default(),
            aggregator: Default::default(),
            availability: Default::default(),
            crypto: Box::new(NoCrypto),
            view: 0,
            view_start: std::time::Instant::now(),
            timeout: std::time::Duration::from_millis(800),
            locked_view: None,
            dag: Default::default(),
            qcs: Default::default(),
            committed: Default::default(),
            mempool: Box::new(NoMempoolPolicy),
            preendorse: Default::default(),
        }
    }

    pub fn new_with_crypto(epoch: EpochState, crypto: Box<dyn CryptoProvider + Send + Sync>) -> Self {
        Self { 
            epoch,
            state: Default::default(),
            aggregator: Default::default(),
            availability: Default::default(),
            crypto,
            view: 0,
            view_start: std::time::Instant::now(),
            timeout: std::time::Duration::from_millis(800),
            locked_view: None,
            dag: Default::default(),
            qcs: Default::default(),
            committed: Default::default(),
            mempool: Box::new(NoMempoolPolicy),
            preendorse: Default::default(),
        }
    }

    /// Build a header for current slot by the designated leader
    pub fn propose_header(&self, parent: H256, number: u64, slot: Slot, payload_hash: H256) -> Option<BlockHeader> {
        let leader = self.leader_for_slot(slot);
        Some(BlockHeader { parent, number, epoch: self.epoch.epoch, slot, proposer: leader, payload_hash })
    }

    /// Decide if we should vote for the proposed header (availability + HotStuff safety)
    pub fn should_vote(&self, _me: &DeviceId, _header: &BlockHeader, ac: &AvailabilityCertificate) -> bool {
        // Availability gate: vote only if all referenced batches are present
        if !self.availability.has_all(&ac.batch_ids) { return false; }
        // Safety gate: follow a basic locked-block rule (stub)
        true
    }

    /// Submit a vote (sig share is a stub here)
    pub fn make_vote(&self, me: DeviceId, phase: Phase, header: &BlockHeader) -> Vote {
        let mut msg = header_hash(header).as_bytes().to_vec();
        msg.push(phase as u8);
        let sig = self.crypto.bls_sign_share(&me, &msg);
        Vote { voter: me, epoch: header.epoch, slot: header.slot, phase, block_id: header_hash(header), sig_share: sig }
    }

    /// Try to aggregate into a QC and update locks
    pub fn on_vote(&mut self, v: Vote) -> Option<QuorumCertificate> {
        self.aggregator.push(v);
        if let Some(best) = &self.state.best_qc { return Some(best.clone()); }
        None
    }

    pub fn on_qc(&mut self, qc: QuorumCertificate) { self.state.update_lock(&qc); }

    /// Crypto-backed leader selection using VRF score per member.
    pub fn leader_for_slot(&self, slot: Slot) -> DeviceId {
        let mut best: Option<(H256, DeviceId)> = None;
        for m in &self.epoch.committee.members {
            let score = self.crypto.vrf_score(m, &self.epoch.seed, slot);
            if best.as_ref().map(|(b, _)| score < *b).unwrap_or(true) {
                best = Some((score, *m));
            }
        }
        best.map(|(_, m)| m).unwrap_or_else(|| H256::repeat_byte(0))
    }


    /// Map view to slot (simple 1:1). Useful for leader rotation on timeouts.
    pub fn leader_for_view(&self, view: View) -> DeviceId {
        self.leader_for_slot(view as Slot)
    }

    /// Pacemaker functions
    pub fn view_timed_out(&self) -> bool { self.view_start.elapsed() >= self.timeout }
    pub fn next_view(&mut self) { self.view = self.view.saturating_add(1); self.view_start = std::time::Instant::now(); }

    /// Basic safety check using justify QC against locked view.
    pub fn safety_check(&self, justify: &Option<QuorumCertificate>) -> bool {
        match (self.locked_view, justify) {
            (Some(lv), Some(j)) => j.slot >= lv as u64,
            (Some(_), None) => false,
            _ => true,
        }
    }

    /// Select batches by mempool policy under target size and pre-endorsement ratio.
    pub fn select_batches(&self) -> Vec<BatchHeader> {
        self.mempool.select_batches(&self.preendorse, &self.epoch.committee, BLOCK_TARGET_BYTES)
    }


    /// Register a proposal into the DAG and return its block id.
    pub fn register_proposal(&mut self, p: &ProposalMsg) -> H256 {
        let parent = p.header.parent;
        let id = header_hash(&p.header);
        self.dag.insert_proposal(p.header.clone(), p.batches.iter().map(|b| b.id).collect(), vec![parent]);
        id
    }

    /// Validate a proposal and, if valid and safe, produce a Prepare vote.
    pub fn handle_proposal(&mut self, me: DeviceId, p: &ProposalMsg) -> Option<VoteMsg> {
        if !self.safety_check(&p.parent_qc) { return None; }
        if !self.verify_proposal_availability(p) { return None; }
        let _ = self.register_proposal(p);
        let vote = self.make_vote(me, Phase::Prepare, &p.header);
        Some(VoteMsg(vote))
    }

    /// Handle an incoming vote; try to form a QC for the given phase.
    pub fn handle_vote(&mut self, v: VoteMsg) -> Option<QCMsg> {
        let VoteMsg(v) = v;
        self.aggregator.push(v.clone());
        if let Some(qc) = self.aggregator
            .try_qc_with_crypto(&self.epoch.committee, v.phase, v.epoch, v.slot, v.block_id, self.crypto.as_ref())
            .or_else(|| self.aggregator.try_qc(&self.epoch.committee, v.phase, v.epoch, v.slot, v.block_id))
        {
            self.qcs.insert(qc.block_id, qc.clone());
            self.on_qc(qc.clone());
            return Some(QCMsg(qc));
        }
        None
    }

    /// Handle an incoming QC. Update safety lock and try to commit using 3-phase rule.
    pub fn handle_qc(&mut self, qcmsg: QCMsg) -> Option<Vec<H256>> {
        let QCMsg(qc) = qcmsg;
        self.qcs.insert(qc.block_id, qc.clone());
        self.on_qc(qc.clone());
        let mut committed = Vec::new();
        if qc.phase == Phase::Commit {
            let b = qc.block_id;
            // Relaxed dev-commit: consider the QC's own block as committed.
            if !self.committed.contains(&b) {
                self.committed.insert(b);
                committed.push(b);
            }
            if let Some(p) = self.dag.parent_of(&b) {
                if let Some(g) = self.dag.parent_of(&p) {
                    let parent_qc_ok = self.qcs.get(&p).map(|q| q.phase == Phase::PreCommit).unwrap_or(false);
                    let grand_qc_ok = self.qcs.get(&g).map(|q| q.phase == Phase::Prepare).unwrap_or(false);
                    if parent_qc_ok && grand_qc_ok && !self.committed.contains(&g) {
                        self.committed.insert(g);
                        committed.push(g);
                    }
                }
            }
        }
        if committed.is_empty() { None } else { Some(committed) }
    }

    // --------- Networking/DA scaffold helpers ---------

    /// Build a proposal message with referenced batches and optional parent QC.
    pub fn build_proposal(&self, parent: H256, number: u64, slot: Slot, batches: &[BatchHeader]) -> ProposalMsg {
        // Payload hash as digest of ordered batch ids
        let mut hasher = Sha3_256::new();
        for b in batches { hasher.update(b.id.as_bytes()); }
        let payload_hash = H256::from_slice(&hasher.finalize()[..32]);
        let header = BlockHeader { parent, number, epoch: self.epoch.epoch, slot, proposer: leader_for_slot(self.epoch.seed, slot, &self.epoch.committee), payload_hash };
        let block_id = header_hash(&header);
        let ac = AvailabilityCertificate { block_id, batch_ids: batches.iter().map(|b| b.id).collect() };
        ProposalMsg { header, batches: batches.to_vec(), ac, parent_qc: self.state.best_qc.clone() }
    }

    /// Verify proposal availability using AC and local availability index.
    pub fn verify_proposal_availability(&self, p: &ProposalMsg) -> bool {
        // AC must match header id and include all referenced batches
        if header_hash(&p.header) != p.ac.block_id { return false; }
        if p.batches.len() != p.ac.batch_ids.len() { return false; }
        // Ensure we have all batches locally
        self.availability.has_all(&p.ac.batch_ids)
    }
}

// -----------------------------
// Utilities
// -----------------------------

pub fn header_hash(h: &BlockHeader) -> H256 {
    let mut enc = h.encode();
    let digest = Sha3_256::digest(&mut enc);
    H256::from_slice(&digest[..32])
}

/// Derive a new epoch seed from a previous QC and optional external IoT entropy
pub fn derive_epoch_seed(prev_qc: &QuorumCertificate, iot_entropy: Option<&[u8]>) -> EpochSeed {
    let mut hasher = Sha3_256::new();
    hasher.update(prev_qc.encode());
    if let Some(e) = iot_entropy { hasher.update(e); }
    EpochSeed(H256::from_slice(&hasher.finalize()[..32]))
}

/// Build a new epoch state with a fixed committee snapshot.
pub fn new_epoch(epoch: Epoch, seed: EpochSeed, committee: Committee) -> EpochState {
    EpochState { epoch, seed, committee }
}

// -----------------------------
// Gossip messages and pre-endorsement (DA) support
// -----------------------------

/// Proposal broadcast over gossip: header + DA batches + AC + optional parent QC
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct ProposalMsg {
    pub header: BlockHeader,
    pub batches: Vec<BatchHeader>,
    pub ac: AvailabilityCertificate,
    pub parent_qc: Option<QuorumCertificate>,
}

/// Gossip wrapper for votes and QCs
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct VoteMsg(pub Vote);

#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct QCMsg(pub QuorumCertificate);

/// Pre-endorsement attestation for a batch (DA readiness)
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct PreEndorsement {
    pub batch_id: BatchId,
    pub voter: DeviceId,
    pub sig_share: Vec<u8>, // stubbed signature share
}

/// Aggregates pre-endorsements per batch to compute coverage.
#[derive(Default, Debug, Clone)]
pub struct PreEndorseAggregator {
    by_batch: BTreeMap<BatchId, BTreeSet<DeviceId>>,
}

impl PreEndorseAggregator {
    pub fn add(&mut self, pre: PreEndorsement) {
        self.by_batch.entry(pre.batch_id).or_default().insert(pre.voter);
    }

    pub fn coverage(&self, batch_id: &BatchId, committee: &Committee) -> f32 {
        let have = self.by_batch.get(batch_id).map(|s| s.len() as f32).unwrap_or(0.0);
        let total = committee.size().max(1) as f32;
        have / total
    }

    /// True if batch has >= threshold voters (2/3 approximation using committee threshold)
    pub fn has_quorum(&self, batch_id: &BatchId, committee: &Committee) -> bool {
        let have = self.by_batch.get(batch_id).map(|s| s.len() as u16).unwrap_or(0);
        have >= committee.threshold
    }
}

/// Compute pre-endorsement ratio for a proposal across all batches.
pub fn preendorsement_ratio(p: &ProposalMsg, agg: &PreEndorseAggregator, committee: &Committee) -> f32 {
    if p.batches.is_empty() { return 1.0; }
    let mut endorsed = 0usize;
    for b in &p.batches {
        if agg.has_quorum(&b.id, committee) { endorsed += 1; }
    }
    endorsed as f32 / (p.batches.len() as f32)
}

/// Leaders can use this to decide priority: require >= 2/3 of batches pre-endorsed.
pub fn has_two_thirds_preendorsement(p: &ProposalMsg, agg: &PreEndorseAggregator, committee: &Committee) -> bool {
    preendorsement_ratio(p, agg, committee) >= (2.0 / 3.0)
}

/// Abstract gossip interface to send consensus messages. Default noop implementor provided.
pub trait Gossip {
    fn broadcast_proposal(&self, _p: &ProposalMsg) {}
    fn broadcast_vote(&self, _v: &VoteMsg) {}
    fn broadcast_qc(&self, _q: &QCMsg) {}
}

#[derive(Default, Clone)]
pub struct NoopGossip;

impl Gossip for NoopGossip {}


// -----------------------------
// Mempool policy: batch selection under size/coverage constraints
// -----------------------------

pub trait MempoolPolicy {
    fn select_batches(
        &self,
        preendorse: &PreEndorseAggregator,
        committee: &Committee,
        target_bytes: u32,
    ) -> Vec<BatchHeader>;
}

#[derive(Default)]
pub struct NoMempoolPolicy;

impl MempoolPolicy for NoMempoolPolicy {
    fn select_batches(
        &self,
        _preendorse: &PreEndorseAggregator,
        _committee: &Committee,
        _target_bytes: u32,
    ) -> Vec<BatchHeader> {
        Vec::new()
    }
}

// -----------------------------
// Real-crypto interfaces (VRF + threshold BLS) with default stub
// -----------------------------

/// Provider abstraction for VRF and threshold BLS operations.
pub trait CryptoProvider {
    /// Deterministic VRF score for member given epoch seed and slot (lower is better).
    fn vrf_score(&self, member: &DeviceId, seed: &EpochSeed, slot: Slot) -> H256;

    /// Produce a BLS signature share over the message by a given member.
    fn bls_sign_share(&self, member: &DeviceId, msg: &[u8]) -> Vec<u8>;

    /// Aggregate and verify BLS shares; returns compact aggregate signature on success.
    fn bls_aggregate_and_verify(
        &self,
        committee: &Committee,
        msg: &[u8],
        shares: &[(DeviceId, Vec<u8>)]
    ) -> Option<Vec<u8>>;
}

/// Default no-crypto implementation using hashes; suitable for local dev/testing.
#[derive(Clone, Debug, Default)]
pub struct NoCrypto;

impl CryptoProvider for NoCrypto {
    fn vrf_score(&self, member: &DeviceId, seed: &EpochSeed, slot: Slot) -> H256 {
        let mut h = Sha3_256::new();
        h.update(seed.0.as_bytes());
        h.update(&slot.to_le_bytes());
        h.update(member.as_bytes());
        H256::from_slice(&h.finalize()[..32])
    }

    fn bls_sign_share(&self, member: &DeviceId, msg: &[u8]) -> Vec<u8> {
        let mut h = Sha3_256::new();
        h.update(msg);
        h.update(member.as_bytes());
        h.finalize().to_vec()
    }

    fn bls_aggregate_and_verify(
        &self,
        committee: &Committee,
        msg: &[u8],
        shares: &[(DeviceId, Vec<u8>)]
    ) -> Option<Vec<u8>> {
        let unique: BTreeSet<_> = shares.iter().map(|(id, _)| *id).collect();
        if (unique.len() as u16) < committee.threshold { return None; }
        let mut agg = Vec::new();
        agg.extend_from_slice(msg);
        for (_, s) in shares { agg.extend_from_slice(s); }
        Some(agg)
    }
}

// -----------------------------
// Digest helpers (QC + AC payload)
// -----------------------------

/// Opaque consensus digest identifier (not registered in runtime; for logging/embedding via digest item).
pub const DIGEST_TAG: &[u8; 4] = b"IQC\0"; // IoT-Quorum-Certificate tag

/// Encode a compact digest payload for QC and AC.
pub fn encode_qc_ac_digest(qc: &QuorumCertificate, ac: &AvailabilityCertificate) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(DIGEST_TAG);
    out.extend_from_slice(&qc.encode());
    out.extend_from_slice(&ac.encode());
    out
}
