use parity_scale_codec::{Encode, Decode};
use sp_core::{blake2_256, ed25519, Pair, H256};
use std::{sync::{Arc, Mutex}, collections::HashMap};

use crate::service::FullClient;
use sc_client_api::HeaderBackend;
use crate::modules::tx::TxVerifier;

#[derive(Clone, Encode, Decode, Debug)]
pub struct Proposal {
    pub parent_hash: H256,
    pub tx_merkle_root: [u8;32],
    pub seed_e: [u8;32],
    pub vrf_output: [u8;32],
    pub vrf_proof: [u8;64],
    pub proposer_pk: [u8;32],
    pub eligible_root: Option<[u8;32]>,
    pub tx_count: u32,
}

pub struct Proposer {
    pub client: Arc<FullClient>,
    pub verifier: Arc<TxVerifier>,
    pub ed_pair: ed25519::Pair,
    pub max_block_bytes: usize,
    pub max_block_weight: u64,
    pub last_proposal: Mutex<Option<Proposal>>, // for debug/RPC
}

impl Proposer {
    pub fn new(
        client: Arc<FullClient>,
        verifier: Arc<TxVerifier>,
        ed_seed: Option<String>,
        max_block_bytes: usize,
        max_block_weight: u64,
    ) -> Self {
        let ed_pair = ed_seed
            .and_then(|s| ed25519::Pair::from_string(&s, None).ok())
            .unwrap_or_else(|| ed25519::Pair::from_string("//Alice", None).expect("dev key"));
        Self { client, verifier, ed_pair, max_block_bytes, max_block_weight, last_proposal: Mutex::new(None) }
    }

    pub fn propose_slot(&self, seed_e: [u8;32], _slot: u64) -> Option<Proposal> {
        // Preconditions: We assume pacemaker ensures we are the leader.
        // Snapshot ELIGIBLE at slot start
        let eligible_hashes: Vec<[u8;32]> = self.verifier.eligible_ordered();
        if eligible_hashes.is_empty() {
            // Empty block allowed
        }

        // Gather bytes for each hash if present locally
        let store: HashMap<[u8;32], Vec<u8>> = self.verifier.snapshot_store();

        // Deterministic ordering: eligible_ordered already sorted by H(tx_hash ⊕ seed_e)
        // Packing loop with max size (weight approximated by size for now)
        let mut included: Vec<[u8;32]> = Vec::new();
        let mut used_bytes: usize = 0;
        for h in eligible_hashes {
            if let Some(bytes) = store.get(&h) {
                let sz = bytes.len();
                if used_bytes + sz > self.max_block_bytes { break; }
                included.push(h);
                used_bytes += sz;
            }
        }

        // Compute Merkle roots
        let tx_merkle_root = merkle_root_ordered(&included);
        let eligible_root = Some(merkle_root_ordered(&included));

        let parent_hash = self.client.info().best_hash;
        // Pseudo-VRF: derive vrf_output as H(seed_e || proposer_pk || parent_hash),
        // vrf_proof = ed25519 signature over that domain (placeholder for proper VRF)
        let mut pk32 = [0u8;32]; pk32.copy_from_slice(&self.ed_pair.public().0);
        let vrf_output = blake2_256(&[&seed_e[..], &pk32[..], parent_hash.as_bytes()].concat());
        let mut msg = Vec::with_capacity(32 + 32 + 32);
        msg.extend_from_slice(b"POSE/VRF/1");
        msg.extend_from_slice(&seed_e);
        msg.extend_from_slice(&pk32);
        msg.extend_from_slice(parent_hash.as_bytes());
        let sig = self.ed_pair.sign(&msg);
        let mut vrf_proof = [0u8;64]; vrf_proof.copy_from_slice(&sig.0);

        let mut prop = Proposal {
            parent_hash,
            tx_merkle_root,
            seed_e,
            vrf_output,
            vrf_proof,
            proposer_pk: pk32,
            eligible_root,
            tx_count: included.len() as u32,
        };

        // Sign proposal header under "POSE/PROPOSAL/1"
        let prop_bytes = prop.encode();
        let sig2 = self.ed_pair.sign(&[b"POSE/PROPOSAL/1".as_ref(), &prop_bytes].concat());
        // For now, overload vrf_proof with signature? No, keep vrf_proof separate; consumers can verify both.

        // Store last proposal for debugging
        *self.last_proposal.lock().unwrap() = Some(prop.clone());
        log::info!(target: "proposer", "Built proposal: parent={}, txs={}, bytes={}", hex::encode(parent_hash), prop.tx_count, used_bytes);

        // TODO: Gossip on /pose/proposal/1 using notifications set
        Some(prop)
    }
}

fn merkle_root_ordered(items: &[[u8;32]]) -> [u8;32] {
    if items.is_empty() { return blake2_256(&[]); }
    let mut level: Vec<[u8;32]> = items.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity((level.len()+1)/2);
        for pair in level.chunks(2) {
            if pair.len() == 1 { next.push(pair[0]); }
            else {
                let h = blake2_256(&[&pair[0][..], &pair[1][..]].concat());
                next.push(h);
            }
        }
        level = next;
    }
    level[0]
}
