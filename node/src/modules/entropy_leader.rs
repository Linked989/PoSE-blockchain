use sp_core::blake2_256;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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
                let round_seed = blake2_256(&[seed.as_slice(), peer_bytes.as_bytes()].concat());

                // Synthesize IoT-like device readings (5 to 10 devices) deterministically
                let device_count = 5 + (round_seed[0] as usize % 6);
                let mut iot_entropy: Vec<u8> = Vec::with_capacity(device_count * 16);
                for i in 0..device_count {
                    let h = blake2_256(&[&round_seed[..], &[i as u8]].concat());
                    // Derive plausible readings from hash bytes
                    let temperature_c = 10 + (h[0] % 25) as i16; // 10..35 C
                    let humidity_pc = 20 + (h[1] % 60) as u8;    // 20..80 %
                    let vibration = (h[2] % 100) as u8;          // 0..99 arbitrary units
                    let device_id = ((h[3] as u16) << 8) | (h[4] as u16);

                    // Accumulate entropy buffer
                    iot_entropy.extend_from_slice(&h);

                    log::debug!(
                        target: label,
                        "IoT sample dev={:04x} temp={}C hum={}%% vib={}",
                        device_id, temperature_c, humidity_pc, vibration
                    );
                }

                // Final election seed mixes everything
                let election_seed = blake2_256(&[&round_seed[..], &iot_entropy[..]].concat());

                // Score each peer by hashing seed||peer and taking the minimum
                let mut best_peer: Option<(String, [u8; 32])> = None;
                for p in &peers {
                    let h = blake2_256(&[&election_seed[..], p.as_bytes()].concat());
                    match &best_peer {
                        None => best_peer = Some((p.clone(), h)),
                        Some((_, cur)) if h < *cur => best_peer = Some((p.clone(), h)),
                        _ => {}
                    }
                }

                if let Some((leader, _)) = best_peer {
                    // Announce the selected leader and current participants
                    log::info!(target: label, "Group size: {} peers", peers.len());
                    log::info!(target: label, "Members: {}", peers.join(", "));
                    log::info!(target: label, "Selected leader: {}", leader);
                }
            } else {
                log::debug!(
                    target: label,
                    "Waiting for peers: {}/{} present",
                    peers.len(),
                    min_group
                );
            }

            thread::sleep(interval);
        }
    });
}
