use parity_scale_codec::{Decode, Encode};
use sc_client_api::HeaderBackend;
use sp_api::ProvideRuntimeApi;
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;
use sp_core::{blake2_256, ed25519, Pair};
use node_template_runtime::opaque::Block;
use sp_runtime::{OpaqueExtrinsic, generic::BlockId};
use sp_runtime::transaction_validity::{TransactionValidityError, TransactionSource};
use std::{collections::{HashMap, HashSet}, sync::{Arc, Mutex}};

use crate::modules::entropy_leader::compute_leader;

#[derive(Clone, Debug)]
pub enum DeferReason { FutureEra, NonceGap, TemporarilyUnverifiable }

#[derive(Clone, Debug)]
pub enum RejectCode {
    DecodeFail,
    TooLarge,
    Expired,
    OldNonce,
    InsufficientBalance,
    TooHeavy,
    CallInvalid,
    Duplicate,
}

#[derive(Clone, Debug)]
pub enum Verdict { Accept, Reject(RejectCode), Defer(DeferReason) }

#[derive(Clone, Debug)]
pub struct TxRecord {
    pub tx_bytes: Vec<u8>,
    pub hash: [u8;32],
    pub sender: Option<String>,
    pub nonce: Option<u32>,
    pub fee: Option<u128>,
    pub weight_est: Option<u64>,
    pub expires_at: Option<u64>,
}

#[derive(Clone, Encode, Decode, Debug)]
pub struct TxAttestation {
    pub tx_hash: [u8;32],
    pub ok: bool,
    pub signer_idx: u16,
    pub sig: [u8;64],
}

#[derive(Default)]
pub struct AttestBitmap {
    // tx_hash -> bitset of attestations for current group (epoch scoped)
    pub bits: HashMap<[u8;32], Vec<bool>>,
    pub first_seen_slot: HashMap<[u8;32], u64>,
}

pub struct EligibleIndex {
    // ordered txs by sorting key
    pub order: Vec<([u8;32], [u8;32])>, // (order_key, tx_hash)
}

impl Default for EligibleIndex { fn default() -> Self { Self { order: Vec::new() } } }

pub struct TxVerifier {
    pub client: Arc<crate::service::FullClient>,
    pub max_tx_bytes: usize,
    pub group_roster: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
    pub my_peer_id: Arc<dyn Fn() -> String + Send + Sync>,
    pub external_round_nonce: Option<[u8;32]>,
    pub ed_pair: ed25519::Pair,
    // state
    pub seen: Mutex<HashSet<[u8;32]>>,
    pub store: Mutex<HashMap<[u8;32], Vec<u8>>>,
    pub bitmaps: Mutex<AttestBitmap>,
    pub eligible: Mutex<EligibleIndex>,
}

impl TxVerifier {
    pub fn new(
        client: Arc<crate::service::FullClient>,
        max_tx_bytes: usize,
        group_roster: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
        my_peer_id: Arc<dyn Fn() -> String + Send + Sync>,
        external_round_nonce: Option<[u8;32]>,
        ed_seed: Option<String>,
    ) -> Self {
        let ed_pair = ed_seed
            .and_then(|s| ed25519::Pair::from_string(&s, None).ok())
            .unwrap_or_else(|| ed25519::Pair::from_string("//Alice", None).expect("dev key"));
        Self {
            client, max_tx_bytes, group_roster, my_peer_id, external_round_nonce,
            ed_pair, seen: Mutex::new(HashSet::new()), store: Mutex::new(HashMap::new()), bitmaps: Mutex::default(), eligible: Mutex::default(),
        }
    }

    pub fn validate_tx(&self, tx_bytes: &[u8]) -> Verdict {
        // 1) Size
        if tx_bytes.len() > self.max_tx_bytes { return Verdict::Reject(RejectCode::TooLarge); }
        // 8) Duplication (early)
        let hash = blake2_256(tx_bytes);
        if self.seen.lock().unwrap().contains(&hash) { return Verdict::Reject(RejectCode::Duplicate); }

        // 1) Decode attempt (runtime type) — ensures bytes are a valid SCALE encoding of our UncheckedExtrinsic
        if node_template_runtime::UncheckedExtrinsic::decode(&mut &*tx_bytes).is_err() {
            return Verdict::Reject(RejectCode::DecodeFail);
        }

        // Defer to runtime for signature/mortality/nonce/fees/weight/call rules
        let xt = match OpaqueExtrinsic::from_bytes(tx_bytes) {
            Ok(x) => x,
            Err(_) => return Verdict::Reject(RejectCode::DecodeFail),
        };
        let at_hash = self.client.info().best_hash;
        let at = BlockId::<Block>::Hash(at_hash);
        match self.client.runtime_api().validate_transaction(&at, TransactionSource::External, xt, at_hash) {
            Ok(inner) => match inner {
                Ok(_valid) => {
                    // store bytes for proposal builder
                    let h = blake2_256(tx_bytes);
                    self.store.lock().unwrap().insert(h, tx_bytes.to_vec());
                    Verdict::Accept
                },
                Err(TransactionValidityError::Invalid(_)) => Verdict::Reject(RejectCode::CallInvalid),
                Err(TransactionValidityError::Unknown(_)) => Verdict::Defer(DeferReason::TemporarilyUnverifiable),
            },
            Err(_api_err) => Verdict::Defer(DeferReason::TemporarilyUnverifiable),
        }
    }

    pub fn attestate_if_leader(&self, tx_bytes: &[u8]) -> Option<TxAttestation> {
        let peers = (self.group_roster)();
        let seed = self.client.info().best_hash.as_fixed_bytes().clone();
        let me = (self.my_peer_id)();
        if let Some((winner, _all, _round_idx, _round_seed)) = compute_leader(peers.clone(), seed, self.external_round_nonce) {
            // Only group validators attest; we model group as all peers, and signer_idx is my index in roster
            let mut roster = peers.clone();
            roster.sort(); roster.dedup();
            let my_idx = roster.iter().position(|p| p == &me)? as u16;
            // Issue attestation only if we're in the active group
            let tx_hash = blake2_256(tx_bytes);
            let msg = [b"POSE/ATTEST/1".as_ref(), &tx_hash].concat();
            let sig = self.ed_pair.sign(&msg);
            let mut sig64 = [0u8; 64]; sig64.copy_from_slice(&sig.0);
            Some(TxAttestation { tx_hash, ok: true, signer_idx: my_idx, sig: sig64 })
        } else { None }
    }

    pub fn note_tx(&self, tx_bytes: &[u8]) { self.seen.lock().unwrap().insert(blake2_256(tx_bytes)); }

    pub fn apply_attestation(&self, att: &TxAttestation) -> bool {
        let roster = (self.group_roster)();
        let n = roster.len();
        if att.signer_idx as usize >= n { return false; }
        let mut bm = self.bitmaps.lock().unwrap();
        let entry = bm.bits.entry(att.tx_hash).or_insert_with(|| vec![false; n]);
        if att.signer_idx as usize >= entry.len() { entry.resize(n, false); }
        let already = entry[att.signer_idx as usize];
        if !already { entry[att.signer_idx as usize] = true; }
        drop(bm);

        // If threshold reached, mark ELIGIBLE
        let threshold = self.eligible_threshold();
        let count = {
            let bm = self.bitmaps.lock().unwrap();
            bm.bits.get(&att.tx_hash).map(|v| v.iter().filter(|b| **b).count()).unwrap_or(0)
        };
        if count >= threshold {
            let seed_e = self.epoch_seed();
            let mut xored = [0u8;32];
            for i in 0..32 { xored[i] = att.tx_hash[i] ^ seed_e[i]; }
            let key = blake2_256(&xored);
            let mut elig = self.eligible.lock().unwrap();
            if !elig.order.iter().any(|(_,h)| h == &att.tx_hash) {
                elig.order.push((key, att.tx_hash));
                elig.order.sort_by(|a,b| a.0.cmp(&b.0));
            }
        }
        !already
    }

    pub fn eligible_threshold(&self) -> usize {
        let n = (self.group_roster)().len();
        ((2*n)/3) + 1
    }

    fn epoch_seed(&self) -> [u8;32] {
        let peers = (self.group_roster)();
        let peer_bytes = peers.join(",");
        let roster_commitment = blake2_256(peer_bytes.as_bytes());
        let seed = self.client.info().best_hash.as_fixed_bytes().clone();
        let round_seed_base = blake2_256(&[seed.as_slice(), peer_bytes.as_bytes()].concat());
        let time_slot_bytes = {
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            (now / 300).to_le_bytes()
        };
        let ext_nonce = self.external_round_nonce.unwrap_or([0u8;32]);
        let mut input = Vec::with_capacity(32+32+8+32);
        input.extend_from_slice(&round_seed_base);
        input.extend_from_slice(&roster_commitment);
        input.extend_from_slice(&time_slot_bytes);
        input.extend_from_slice(&ext_nonce);
        blake2_256(&input)
    }

    pub fn eligible_ordered(&self) -> Vec<[u8;32]> {
        let elig = self.eligible.lock().unwrap();
        elig.order.iter().map(|(_,h)| *h).collect()
    }

    pub fn snapshot_store(&self) -> HashMap<[u8;32], Vec<u8>> {
        self.store.lock().unwrap().clone()
    }
}
