use sp_core::blake2_256;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn log2(x: f64) -> f64 { x.ln() / std::f64::consts::LN_2 }

// Compute collision entropy H2 = -log2( sum p_i^2 ) over byte histogram
pub(crate) fn collision_entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() { return 0.0; }
    let mut counts = [0u64; 256];
    for &b in bytes { counts[b as usize] += 1; }
    let n = bytes.len() as f64;
    let sum_p2: f64 = counts.iter()
        .filter(|&&c| c > 0)
        .map(|&c| { let p = c as f64 / n; p * p })
        .sum();
    if sum_p2 <= 0.0 { 0.0 } else { -log2(sum_p2) }
}

// Shannon entropy with Miller–Madow correction
pub(crate) fn shannon_mm_entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() { return 0.0; }
    let mut counts = [0u64; 256];
    for &b in bytes { counts[b as usize] += 1; }
    let n = bytes.len() as f64;
    let mut k_nonzero = 0u64;
    let h: f64 = counts.iter()
        .filter(|&&c| c > 0)
        .map(|&c| { k_nonzero += 1; let p = c as f64 / n; -p * log2(p) })
        .sum();
    // Miller–Madow correction: (k-1)/(2N ln 2)
    let correction = if n > 0.0 { ((k_nonzero as f64) - 1.0) / (2.0 * n * std::f64::consts::LN_2) } else { 0.0 };
    h + correction
}

fn hex32(x: &[u8; 32]) -> String { format!("0x{}", hex::encode(x)) }

#[derive(Clone)]
pub struct GroupComputation {
    pub id: String,
    pub h2: f64,
    pub hmm: f64,
    pub digest: [u8;32],
    pub device_ids: Vec<String>,
}

pub fn compute_leader(
    mut peers: Vec<String>,
    seed: [u8;32],
    external_round_nonce: Option<[u8;32]>,
) -> Option<(GroupComputation, Vec<GroupComputation>, u64, [u8;32])> {
    if peers.is_empty() { return None; }
    peers.sort(); peers.dedup();
    let peer_bytes = peers.join(",");
    let roster_commitment = blake2_256(peer_bytes.as_bytes());
    let round_seed_base = blake2_256(&[seed.as_slice(), peer_bytes.as_bytes()].concat());
    let time_slot_bytes = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        (now / 300).to_le_bytes()
    };
    let ext_nonce = external_round_nonce.unwrap_or([0u8;32]);
    let mut round_seed_input = Vec::with_capacity(32+32+8+32);
    round_seed_input.extend_from_slice(&round_seed_base);
    round_seed_input.extend_from_slice(&roster_commitment);
    round_seed_input.extend_from_slice(&time_slot_bytes);
    round_seed_input.extend_from_slice(&ext_nonce);
    let round_seed = blake2_256(&round_seed_input);
    let round_index = u64::from_le_bytes([
        round_seed[0],round_seed[1],round_seed[2],round_seed[3],
        round_seed[4],round_seed[5],round_seed[6],round_seed[7]
    ]);

    let mut scores: Vec<GroupComputation> = Vec::with_capacity(peers.len());
    for gid in &peers {
        let mut mix_bytes: Vec<u8> = Vec::with_capacity(10*32);
        let mut device_ids = Vec::with_capacity(10);
        for i in 0..10u8 {
            let did_hash = blake2_256(&[&round_seed[..], gid.as_bytes(), &[i]].concat());
            let dev_id = format!("dev-{}-{}", &gid[..std::cmp::min(6, gid.len())], &hex::encode(&did_hash[..4]));
            device_ids.push(dev_id);
            let temperature_c = 15 + (did_hash[0] % 20) as u8;
            let humidity_pc   = 30 + (did_hash[1] % 50) as u8;
            let weight_kg     = 5  + (did_hash[2] % 95) as u8;
            let battery_pc    = 20 + (did_hash[3] % 81) as u8;
            let motion        = did_hash[4] % 2;
            let reading_ser = [temperature_c, humidity_pc, weight_kg, battery_pc, motion];
            let reading_hash = blake2_256(&[&did_hash[..], &reading_ser[..]].concat());
            mix_bytes.extend_from_slice(&reading_hash);
        }
        let h2 = collision_entropy(&mix_bytes);
        let hmm = shannon_mm_entropy(&mix_bytes);
        let mix_hash = blake2_256(&mix_bytes);
        let digest = blake2_256(&[gid.as_bytes(), &round_seed, &roster_commitment, &mix_hash].concat());
        scores.push(GroupComputation { id: gid.clone(), h2, hmm, digest, device_ids });
    }
    scores.sort_by(|a,b| {
        use std::cmp::Ordering::*;
        match b.h2.partial_cmp(&a.h2).unwrap_or(Equal) {
            Equal => match b.hmm.partial_cmp(&a.hmm).unwrap_or(Equal) {
                Equal => a.digest.cmp(&b.digest),
                other => other,
            },
            other => other,
        }
    });
    scores.first().cloned().map(|w| (w, scores, round_index, round_seed))
}

/// Spawns a background thread that, once at least `min_group` peers are present,
/// deterministically selects a leader among them using entropy derived from:
/// - latest best block hash (seed shared by all honest nodes)
/// - current peer set (sorted)
/// - synthetic IoT-like measurements (deterministically derived from the seed)
///
/// Peers are passed in via the `get_peers` closure (must include local peer id),
/// and a 32-byte seed via `get_seed` (e.g., best block hash).
pub fn spawn_entropy_leader<FPeers, FSeed>(
    get_peers: FPeers,
    get_seed: FSeed,
    min_group: usize,
    interval: Duration,
    label: &'static str,
    external_round_nonce: Option<[u8;32]>,
) where
    FPeers: Fn() -> Vec<String> + Send + Sync + 'static,
    FSeed: Fn() -> [u8; 32] + Send + Sync + 'static,
{
    let get_peers = Arc::new(get_peers);
    let get_seed = Arc::new(get_seed);

    thread::spawn(move || {
        loop {
            // Fetch current peer set and shared seed
            let mut peers = (get_peers)();
            // Ensure deterministic order across nodes
            peers.sort();
            peers.dedup();

            if peers.len() >= min_group {
                // Derive a stable round seed from best hash and peer set
                let seed = (get_seed)();
                let peer_bytes = peers.join(",");
                let round_seed_base = blake2_256(&[seed.as_slice(), peer_bytes.as_bytes()].concat());

                // Roster commitment for groups (sorted peer IDs)
                let roster_commitment = blake2_256(peer_bytes.as_bytes());

                // Mix in external nonce if provided, else a coarse time-slot to vary across restarts
                let time_slot_bytes = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    // 5-minute slots
                    let slot: u64 = now / 300;
                    slot.to_le_bytes()
                };
                let ext_nonce = external_round_nonce.unwrap_or([0u8; 32]);
                let mut round_seed_input = Vec::with_capacity(32 + 32 + 8 + 32);
                round_seed_input.extend_from_slice(&round_seed_base);
                round_seed_input.extend_from_slice(&roster_commitment);
                round_seed_input.extend_from_slice(&time_slot_bytes);
                round_seed_input.extend_from_slice(&ext_nonce);
                let round_seed = blake2_256(&round_seed_input);

                // Round index derived from seed high bytes (deterministic across nodes)
                let round_index = u64::from_le_bytes([round_seed[0],round_seed[1],round_seed[2],round_seed[3],round_seed[4],round_seed[5],round_seed[6],round_seed[7]]);

                // For each group (peer id), synthesize 10 IoT devices and readings deterministically from (group_id, round_seed)
                struct GroupScore { id: String, h2: f64, hmm: f64, digest: [u8;32], device_ids: Vec<String> }
                let mut scores: Vec<GroupScore> = Vec::with_capacity(peers.len());
                for gid in &peers {
                    let mut mix_bytes: Vec<u8> = Vec::with_capacity(10 * 32);
                    let mut device_ids = Vec::with_capacity(10);
                    for i in 0..10u8 {
                        // Device ID derived deterministically
                        let did_hash = blake2_256(&[&round_seed[..], gid.as_bytes(), &[i]].concat());
                        let dev_id = format!("dev-{}-{}", &gid[..std::cmp::min(6, gid.len())], &hex::encode(&did_hash[..4]));
                        device_ids.push(dev_id);

                        // Generate plausible readings based on hash
                        let temperature_c = 15 + (did_hash[0] % 20) as u8;     // 15..34 C
                        let humidity_pc   = 30 + (did_hash[1] % 50) as u8;     // 30..79 %
                        let weight_kg     = 5  + (did_hash[2] % 95) as u8;     // 5..99 kg
                        let battery_pc    = 20 + (did_hash[3] % 81) as u8;     // 20..100 %
                        let motion        = did_hash[4] % 2;                   // 0/1
                        // Pack readings into a small record and hash to bytes for entropy calc
                        let reading_ser = [temperature_c, humidity_pc, weight_kg, battery_pc, motion];
                        let reading_hash = blake2_256(&[&did_hash[..], &reading_ser[..]].concat());
                        mix_bytes.extend_from_slice(&reading_hash);
                    }

                    let h2 = collision_entropy(&mix_bytes);
                    let hmm = shannon_mm_entropy(&mix_bytes);
                    let mix_hash = blake2_256(&mix_bytes);
                    // D_g = H(group_id || r || C_r || roster_commitment || H(mix_g))
                    let mut r_bytes = [0u8; 8];
                    r_bytes.copy_from_slice(&round_index.to_le_bytes());
                    let digest = blake2_256(&[
                        gid.as_bytes(),
                        &r_bytes,
                        &round_seed,
                        &roster_commitment,
                        &mix_hash,
                    ].concat());

                    scores.push(GroupScore { id: gid.clone(), h2, hmm, digest, device_ids });
                }

                // Rank by H2 descending, then H_MM descending, then lowest digest wins
                scores.sort_by(|a,b| {
                    use std::cmp::Ordering::*;
                    match b.h2.partial_cmp(&a.h2).unwrap_or(Equal) {
                        Equal => match b.hmm.partial_cmp(&a.hmm).unwrap_or(Equal) {
                            Equal => a.digest.cmp(&b.digest),
                            other => other,
                        },
                        other => other,
                    }
                });

            //     if let Some(winner) = scores.first() {
            //         log::info!(target: label, "Round r={} roster_commitment={}", round_index, hex32(&roster_commitment));
            //         for g in &scores {
            //             log::info!(target: label,
            //                 "Group {}: H2={:.4}, H_MM={:.4}, digest={}, devices={} => {:?}",
            //                 &g.id,
            //                 g.h2, g.hmm, hex32(&g.digest), g.device_ids.len(), g.device_ids);
            //         }
            //         log::info!(target: label, "Leader Group: {}", winner.id);
            //     }
            // } else {
            //     log::debug!(
            //         target: label,
            //         "Waiting for peers: {}/{} present",
            //         peers.len(),
            //         min_group
            //     );
            // }

            thread::sleep(interval);
        }
    });
}
