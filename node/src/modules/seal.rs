use parity_scale_codec::{Encode, Decode};
use sp_core::{blake2_256, ed25519, Pair, H256};
use std::sync::Arc;

use crate::modules::{proposal::Proposal, validation, tx::TxVerifier};
use crate::service::FullClient;
use sc_client_api::HeaderBackend;

#[derive(Encode, Decode, Clone)]
pub struct ProposalMsg {
    pub proposal: Proposal,
    pub signature: [u8;64],
    pub body_hashes: Vec<[u8;32]>,
}

pub struct Sealer {
    pub client: Arc<FullClient>,
    pub verifier: Arc<TxVerifier>,
    pub ed_pair: ed25519::Pair,
    pub max_block_bytes: usize,
    pub max_block_weight: u64,
}

impl Sealer {
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
        Self { client, verifier, ed_pair, max_block_bytes, max_block_weight }
    }

    pub fn seal_slot(&self, seed_e: [u8;32], slot: u64) {
        let t0 = std::time::Instant::now();
        // Select parent (simple best head)
        let parent_hash = self.client.info().best_hash;

        // Snapshot eligible
        let elig = self.verifier.eligible_ordered();
        let store = self.verifier.snapshot_store();

        // Order already deterministic by eligible_ordered (H(tx ⊕ seed_e)); ensure tie-break stable by hash
        let mut body_hashes: Vec<[u8;32]> = elig
            .into_iter()
            .filter(|h| store.contains_key(h))
            .collect();
        body_hashes.sort();

        // Pack by byte cap
        let mut included: Vec<[u8;32]> = Vec::new();
        let mut used_bytes: usize = 0;
        for h in body_hashes {
            if let Some(bytes) = store.get(&h) {
                if used_bytes + bytes.len() > self.max_block_bytes { break; }
                included.push(h);
                used_bytes += bytes.len();
            }
        }

        // Roots
        let tx_merkle_root = merkle_root_ordered(&included);
        let eligible_root = Some(tx_merkle_root);

        // VRF placeholder
        let mut pk32 = [0u8;32]; pk32.copy_from_slice(&self.ed_pair.public().0);
        let vrf_output = blake2_256(&[&seed_e[..], &pk32[..], parent_hash.as_bytes()].concat());
        let vrf_proof = {
            let sig = self.ed_pair.sign(&[b"POSE/VRF/1".as_ref(), &seed_e, &pk32, parent_hash.as_bytes()].concat());
            let mut a = [0u8;64]; a.copy_from_slice(&sig.0); a
        };

        let proposal = Proposal {
            parent_hash,
            tx_merkle_root,
            seed_e,
            vrf_output,
            vrf_proof,
            proposer_pk: pk32,
            eligible_root,
            tx_count: included.len() as u32,
        };
        // Sign proposal
        let sig = self.ed_pair.sign(&[b"POSE/PROPOSAL/1".as_ref(), &proposal.encode()].concat());
        let mut sig64 = [0u8;64]; sig64.copy_from_slice(&sig.0);

        // Self-validate
        let verdict = validation::validate_proposal(
            self.client.clone(),
            &self.verifier,
            &proposal,
            &sig64,
            &included,
            used_bytes,
            seed_e,
            slot,
            self.max_block_bytes,
            self.max_block_weight,
        );

        match verdict {
            validation::ProposalVerdict::Accept => {
                let dt = t0.elapsed();
                log::info!(target: "pose-seal", "Sealed proposal: parent={}, txs={}, bytes={}, latency={:?}", hex::encode(parent_hash), included.len(), used_bytes, dt);
                // Encode message; TODO: gossip via /pose/proposal/1
                let _msg = ProposalMsg { proposal, signature: sig64, body_hashes: included };
            }
            validation::ProposalVerdict::Reject(code) => {
                log::warn!(target: "pose-seal", "Self-validation rejected proposal: {:?}", code);
            }
            validation::ProposalVerdict::Defer(reason) => {
                log::warn!(target: "pose-seal", "Self-validation deferred proposal: {:?}", reason);
            }
        }
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
                next.push(blake2_256(&[&pair[0][..], &pair[1][..]].concat()));
            }
        }
        level = next;
    }
    level[0]
}

