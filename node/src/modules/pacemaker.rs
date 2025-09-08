use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::modules::entropy_leader::compute_leader;

#[derive(Clone, Debug)]
pub struct PacemakerConfig {
    pub slot_ms: u64,            // T_slot
    pub epoch_slots: u64,        // E
    pub propose_base_ms: u64,    // T_propose_base
    pub vote_base_ms: u64,       // T_vote_base
    pub settle_base_ms: u64,     // T_settle_base
    pub ewma_alpha: f64,         // alpha
    pub ewma_min_ms: u64,        // clamp min
    pub ewma_max_ms: u64,        // clamp max
}

impl Default for PacemakerConfig {
    fn default() -> Self {
        Self {
            slot_ms: 250,
            epoch_slots: 40,
            propose_base_ms: 250,
            vote_base_ms: 250,
            settle_base_ms: 100,
            ewma_alpha: 0.2,
            ewma_min_ms: 150,
            ewma_max_ms: 1000,
        }
    }
}

fn clamp_u64(x: u64, lo: u64, hi: u64) -> u64 { x.max(lo).min(hi) }

/// Spawns the pacemaker loop.
/// - `get_peers`: returns sorted group IDs (peer ids) including self
/// - `get_seed`: shared 32-byte seed (e.g., best block hash)
/// - `my_id`: returns local peer id
/// - `external_round_nonce`: optional extra entropy shared across nodes
pub fn spawn_pacemaker<FGPeers, FGSeed, FGMyId>(
    cfg: PacemakerConfig,
    get_peers: FGPeers,
    get_seed: FGSeed,
    my_id: FGMyId,
    external_round_nonce: Option<[u8;32]>,
)
where
    FGPeers: Fn() -> Vec<String> + Send + Sync + 'static,
    FGSeed: Fn() -> [u8;32] + Send + Sync + 'static,
    FGMyId: Fn() -> String + Send + Sync + 'static,
{
    let get_peers = Arc::new(get_peers);
    let get_seed = Arc::new(get_seed);
    let my_id = Arc::new(my_id);
    let cfg0 = cfg.clone();

    thread::spawn(move || {
        let cfg = cfg0;
        let slot = Duration::from_millis(cfg.slot_ms);
        // Align slot0 to now (monotonic). No wall-clock is used beyond this alignment.
        let slot0 = Instant::now();
        let mut last_slot: i64 = -1;
        let mut ewma_ms: f64 = cfg.vote_base_ms as f64; // seed EWMA with base

        loop {
            let now = Instant::now();
            let elapsed = now.duration_since(slot0);
            let s = (elapsed.as_millis() as u64) / cfg.slot_ms; // current slot index
            if s as i64 == last_slot { thread::sleep(Duration::from_millis(10)); continue; }
            last_slot = s as i64;

            let epoch_index = s / cfg.epoch_slots;
            let round_in_epoch = s % cfg.epoch_slots;

            // Determine leader using entropy-based selection
            let peers = (get_peers)();
            let seed = (get_seed)();
            let my = (my_id)();
            let leader = compute_leader(peers.clone(), seed, external_round_nonce)
                .map(|(win, _all, round_seed_num, _round_seed)| (win.id, round_seed_num));

            let leader_id = leader.as_ref().map(|(id, _)| id.clone());
            let round_index = leader.map(|(_, r)| r).unwrap_or(0);

            let l_ewma = clamp_u64(((2.0 * ewma_ms) as u64), cfg.ewma_min_ms, cfg.ewma_max_ms);
            let t_propose = Duration::from_millis(clamp_u64(cfg.propose_base_ms.max(l_ewma), cfg.ewma_min_ms, cfg.ewma_max_ms));
            let t_vote    = Duration::from_millis(clamp_u64(cfg.vote_base_ms.max(l_ewma), cfg.ewma_min_ms, cfg.ewma_max_ms));
            let t_settle  = Duration::from_millis(clamp_u64(cfg.settle_base_ms.max(l_ewma/2), cfg.ewma_min_ms, cfg.ewma_max_ms));

            // log::info!(target: "pacemaker", 
            //     "slot={} epoch={} round={} leader={:?} (me={}) budgets: propose={:?} vote={:?} settle={:?}",
            //     s, epoch_index, round_in_epoch, leader_id, my, t_propose, t_vote, t_settle);

            // Leader nudge: if I'm the leader and not proposed by half of propose time, print reminder
            if let Some(ref lid) = leader_id { if *lid == my { 
                let half = t_propose / 2;
                let start = Instant::now();
                thread::spawn(move || {
                    thread::sleep(half);
                    log::debug!(target: "pacemaker", "leader-nudge: build now (slot ongoing, {:?} elapsed)", start.elapsed());
                });
            }}

            // Per-slot timers -- non-blocking advance
            thread::spawn(move || {
                let slot_start = Instant::now();
                thread::sleep(t_propose);
                log::debug!(target: "pacemaker", "propose timeout at +{:?}", slot_start.elapsed());
                thread::sleep(t_vote);
                log::debug!(target: "pacemaker", "vote timeout at +{:?}", slot_start.elapsed());
                thread::sleep(t_settle);
                log::debug!(target: "pacemaker", "settle cutoff at +{:?}", slot_start.elapsed());
            });

            // Simple EWMA decay towards base (without external samples yet)
            ewma_ms = cfg.ewma_alpha * (cfg.vote_base_ms as f64) + (1.0 - cfg.ewma_alpha) * ewma_ms;
        }
    });
}

