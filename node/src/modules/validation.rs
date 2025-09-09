use parity_scale_codec::{Encode, Decode};
use sp_core::{ed25519, blake2_256, H256, Pair};
use std::sync::Arc;

use crate::modules::proposal::Proposal;
use crate::modules::tx::TxVerifier;
use crate::service::FullClient;

#[derive(Clone, Debug)]
pub enum DeferReason { MissingParent, MissingData }

#[derive(Clone, Debug)]
pub enum RejectCode {
    BadEnvelope,
    BadSignature,
    WrongLeader,
    UnknownParent,
    EpochMismatch,
    CapExceeded,
    BadMerkle,
    NonEligibleTx,
    BadOrder,
    BadNonceOrder,
}

#[derive(Clone, Debug)]
pub enum ProposalVerdict { Accept, Reject(RejectCode), Defer(DeferReason) }

fn merkle_root_ordered(items: &[[u8;32]]) -> [u8;32] {
    if items.is_empty() { return blake2_256(&[]); }
    let mut level: Vec<[u8;32]> = items.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity((level.len()+1)/2);
        for pair in level.chunks(2) {
            if pair.len() == 1 { next.push(pair[0]); }
            else { next.push(blake2_256(&[&pair[0][..], &pair[1][..]].concat())); }
        }
        level = next;
    }
    level[0]
}

fn h_order_key(seed_e: &[u8;32], tx: &[u8;32]) -> [u8;32] {
    let mut x = [0u8;32];
    for i in 0..32 { x[i] = seed_e[i] ^ tx[i]; }
    blake2_256(&x)
}

pub fn validate_proposal(
    client: Arc<FullClient>,
    verifier: &TxVerifier,
    proposal: &Proposal,
    proposal_sig: &[u8;64],
    ordered_body: &[[u8;32]],
    body_bytes: usize,
    seed_e: [u8;32],
    slot: u64,
    max_block_bytes: usize,
    _max_block_weight: u64,
) -> ProposalVerdict {
    // 1. Envelope sanity
    if body_bytes > max_block_bytes { return ProposalVerdict::Reject(RejectCode::CapExceeded); }
    // Signature over domain
    let mut pk32 = [0u8;32]; pk32.copy_from_slice(&proposal.proposer_pk);
    let pubk = ed25519::Public(pk32);
    let msg = [b"POSE/PROPOSAL/1".as_ref(), &proposal.encode()].concat();
    if ed25519::Pair::verify(&ed25519::Signature(*proposal_sig), &msg, &pubk) == false {
        return ProposalVerdict::Reject(RejectCode::BadSignature);
    }
    // VRF check (placeholder consistent with builder)
    let expected_vrf = blake2_256(&[&proposal.seed_e[..], &proposal.proposer_pk[..], proposal.parent_hash.as_bytes()].concat());
    if proposal.vrf_output != expected_vrf { return ProposalVerdict::Reject(RejectCode::WrongLeader); }
    // Verify seed matches local seed_e
    if proposal.seed_e != seed_e { return ProposalVerdict::Reject(RejectCode::EpochMismatch); }

    // 2. Parent linkage
    use node_template_runtime::opaque::Block;
    use sp_runtime::generic::BlockId;
    let bid = BlockId::<Block>::Hash(proposal.parent_hash);
    if client.header(&bid).ok().flatten().is_none() {
        return ProposalVerdict::Defer(DeferReason::MissingParent);
    }

    // 4. Caps & header/body consistency
    if (proposal.tx_count as usize) != ordered_body.len() { /* not fatal, but sanity */ }
    let merkle = merkle_root_ordered(ordered_body);
    if proposal.tx_merkle_root != merkle { return ProposalVerdict::Reject(RejectCode::BadMerkle); }
    if let Some(root) = proposal.eligible_root { if root != merkle { return ProposalVerdict::Reject(RejectCode::BadMerkle); } }

    // 5. Eligibility discipline
    for h in ordered_body {
        if !verifier.has_quorum(h) { return ProposalVerdict::Reject(RejectCode::NonEligibleTx); }
    }

    // 6. Deterministic ordering check
    let mut keys: Vec<([u8;32],[u8;32])> = ordered_body.iter().map(|h| (h_order_key(&seed_e, h), *h)).collect();
    let mut sorted = keys.clone();
    sorted.sort_by(|a,b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    if sorted != keys { return ProposalVerdict::Reject(RejectCode::BadOrder); }

    // 7. Nonce/UTXO discipline: TODO — requires parsing extrinsics and parent state

    // 8. Budget: already checked bytes; weight TODO

    // 9. Stateless call guardrails: omitted here — handled in runtime import

    ProposalVerdict::Accept
}
