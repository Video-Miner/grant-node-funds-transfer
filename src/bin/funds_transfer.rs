use std::{env, fmt, path::Path, sync::Arc, time::Duration};

use ethers::{
    contract::abigen,
    middleware::SignerMiddleware,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{Address, TxHash, U256},
};
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};

abigen!(
    BondingManager,
    "src/abi/BondingManager.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

abigen!(
    RoundsManager,
    "src/abi/RoundsManager.json",
    event_derives(serde::Deserialize, serde::Serialize)
);
#[derive(Clone, Debug)]
struct Config {
    http_rpc_url: String,
    chain_id: u64,

    rounds_manager_addr: Address,
    bonding_manager_addr: Address,

    json_key_file: String,
    passphrase_file: String,
    orchestrator_addr: Option<Address>,

    // Loop timing
    loop_sleep_secs: u64,
    // Tx receipt wait timeout
    receipt_timeout_secs: u64,

    // Bond transfer
    lpt_receiver_addr: Address,
    lpt_min_retain_wei: U256,

    // Fee withdrawal
    eth_fee_receiver_addr: Address,
    eth_fee_withdraw_threshold_wei: U256,
}

#[derive(Debug)]
enum AppError {
    MissingEnv(&'static str),
    BadEnv(&'static str, String),
    Provider(String),
    Wallet(String),
    Contract(String),
    Tx(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::MissingEnv(k) => write!(f, "missing env var: {k}"),
            AppError::BadEnv(k, v) => write!(f, "invalid env var {k}: {v}"),
            AppError::Provider(e) => write!(f, "provider error: {e}"),
            AppError::Wallet(e) => write!(f, "wallet error: {e}"),
            AppError::Contract(e) => write!(f, "contract error: {e}"),
            AppError::Tx(e) => write!(f, "tx error: {e}"),
        }
    }
}

impl std::error::Error for AppError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RoundState {
    round: U256,
    initialized: bool,
    locked: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let cfg = load_config()?;
    info!(
        "starting funds_transfer with cfg: chain_id={}, rounds_manager={:?}, bonding_manager={:?}, loop_sleep_secs={}",
        cfg.chain_id, cfg.rounds_manager_addr, cfg.bonding_manager_addr, cfg.loop_sleep_secs
    );

    let provider = Provider::<Http>::try_from(cfg.http_rpc_url.as_str())
        .map_err(|e| AppError::Provider(format!("{e}")))?;
    // Small internal polling interval for provider housekeeping
    let provider = provider.interval(Duration::from_millis(250));

    let passphrase = std::fs::read_to_string(&cfg.passphrase_file)
        .map_err(|e| AppError::Wallet(format!("failed to read PASSPHRASE_FILE: {e}")))?;
    let passphrase = passphrase.trim_end();

    let key_json_path = Path::new(&cfg.json_key_file);
    let wallet = LocalWallet::decrypt_keystore(key_json_path, passphrase)
        .map_err(|e| AppError::Wallet(format!("failed to decrypt JSON_KEY_FILE: {e}")))?
        .with_chain_id(cfg.chain_id);

    let derived_addr = wallet.address();
    let orchestrator_addr = cfg.orchestrator_addr.unwrap_or(derived_addr);

    if orchestrator_addr != derived_addr {
        warn!(
            "ORCHESTRATOR_ADDR differs from PRIVATE_KEY derived address; using ORCHESTRATOR_ADDR={:?}, signer={:?}",
            orchestrator_addr, derived_addr
        );
    } else {
        info!("orchestrator/signer address: {:?}", orchestrator_addr);
    }

    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    let rounds = RoundsManager::new(cfg.rounds_manager_addr, client.clone());
    let bonding = BondingManager::new(cfg.bonding_manager_addr, client.clone());

    // Cache last seen round state for transition logging
    let mut last_state: Option<RoundState> = None;

    loop {
        let state = match fetch_round_state(&rounds).await {
            Ok(s) => s,
            Err(e) => {
                warn!("failed to fetch round state: {e}; will retry next loop");
                sleep(Duration::from_secs(cfg.loop_sleep_secs)).await;
                continue;
            }
        };

        if last_state
            .map(|ls| {
                ls.round != state.round
                    || ls.initialized != state.initialized
                    || ls.locked != state.locked
            })
            .unwrap_or(true)
        {
            info!(
                "round state: round={} initialized={} locked={}",
                state.round, state.initialized, state.locked
            );
        } else {
            debug!(
                "round unchanged: round={} initialized={} locked={}",
                state.round, state.initialized, state.locked
            );
        }

        // 1) When initialized: reward() once per round
        if state.initialized
            && let Err(e) = maybe_reward_once_per_round(
                &bonding,
                orchestrator_addr,
                state.round,
                cfg.receipt_timeout_secs,
            )
            .await
        {
            // No internal retry: log and let next loop try again
            warn!("reward check/tx failed: {e}; will retry next loop if still needed");
        }

        // 2) When locked: transferBond + withdrawFees
        if state.locked
            && let Err(e) =
                handle_locked_round_actions(&bonding, orchestrator_addr, state.round, &cfg).await
        {
            warn!("locked-round actions failed: {e}; will retry next loop if still needed");
        }

        last_state = Some(state);
        sleep(Duration::from_secs(cfg.loop_sleep_secs)).await;
    }
}

async fn fetch_round_state<M: Middleware>(
    rounds: &RoundsManager<M>,
) -> Result<RoundState, AppError> {
    let round = rounds
        .current_round()
        .call()
        .await
        .map_err(|e| AppError::Contract(format!("RoundsManager.currentRound() failed: {e}")))?;

    let initialized = rounds
        .current_round_initialized()
        .call()
        .await
        .map_err(|e| {
            AppError::Contract(format!(
                "RoundsManager.currentRoundInitialized() failed: {e}"
            ))
        })?;

    let locked = rounds.current_round_locked().call().await.map_err(|e| {
        AppError::Contract(format!("RoundsManager.currentRoundLocked() failed: {e}"))
    })?;

    Ok(RoundState {
        round,
        initialized,
        locked,
    })
}

/// Calls bonding.reward() ONLY if lastRewardRound < current_round.
/// No internal retries: if tx fails, caller logs and next loop will retry.
async fn maybe_reward_once_per_round<M: Middleware>(
    bonding: &BondingManager<M>,
    orchestrator: Address,
    current_round: U256,
    receipt_timeout_secs: u64,
) -> Result<(), AppError> {
    // getTranscoder(addr) returns a tuple whose first element is lastRewardRound (per ABI)
    let t = bonding
        .get_transcoder(orchestrator)
        .call()
        .await
        .map_err(|e| AppError::Contract(format!("BondingManager.getTranscoder() failed: {e}")))?;

    let last_reward_round: U256 = t.0;

    if last_reward_round >= current_round {
        debug!(
            "reward not needed: lastRewardRound={} currentRound={}",
            last_reward_round, current_round
        );
        return Ok(());
    }

    info!(
        "reward needed: lastRewardRound={} < currentRound={} (sending reward tx)",
        last_reward_round, current_round
    );

    let call = bonding.reward();
    let pending = call
        .send()
        .await
        .map_err(|e| AppError::Tx(format!("reward() send failed: {e}")))?;

    let tx_hash: TxHash = *pending;
    info!("reward tx sent: tx_hash={:?}", tx_hash);

    // Wait for receipt with a timeout so we don't hang forever.
    match timeout(Duration::from_secs(receipt_timeout_secs), pending).await {
        Ok(Ok(Some(receipt))) => {
            info!(
                "reward tx confirmed: tx_hash={:?} status={:?} block={:?} gas_used={:?}",
                receipt.transaction_hash, receipt.status, receipt.block_number, receipt.gas_used
            );
            Ok(())
        }
        Ok(Ok(None)) => Err(AppError::Tx(format!(
            "reward tx pending returned None receipt: tx_hash={:?}",
            tx_hash
        ))),
        Ok(Err(e)) => Err(AppError::Tx(format!(
            "reward tx receipt error: tx_hash={:?} err={e}",
            tx_hash
        ))),
        Err(_) => Err(AppError::Tx(format!(
            "reward tx receipt timeout after {}s: tx_hash={:?}",
            receipt_timeout_secs, tx_hash
        ))),
    }
}

async fn handle_locked_round_actions<M: Middleware>(
    bonding: &BondingManager<M>,
    orchestrator: Address,
    current_round: U256,
    cfg: &Config,
) -> Result<(), AppError> {
    // pendingStake / pendingFees are “as of endRound”; using currentRound keeps it consistent with go-livepeer style.
    let pending_stake = bonding
        .pending_stake(orchestrator, current_round)
        .call()
        .await
        .map_err(|e| AppError::Contract(format!("pendingStake() failed: {e}")))?;

    let pending_fees = bonding
        .pending_fees(orchestrator, current_round)
        .call()
        .await
        .map_err(|e| AppError::Contract(format!("pendingFees() failed: {e}")))?;

    info!(
        "locked round actions: pendingStakeWei={} pendingFeesWei={}",
        pending_stake, pending_fees
    );

    // ---- transferBond ----
    let transferable = match pending_stake.checked_sub(cfg.lpt_min_retain_wei) {
        Some(v) if !v.is_zero() => v,
        _ => {
            debug!(
                "transferBond skipped: pendingStakeWei={} <= LPT_MIN_RETAIN_WEI={}",
                pending_stake, cfg.lpt_min_retain_wei
            );
            U256::zero()
        }
    };

    if !transferable.is_zero() {
        info!(
            "transferBond sending: from_orchestrator={:?} to_receiver={:?} amountWei={}",
            orchestrator, cfg.lpt_receiver_addr, transferable
        );

        let call = bonding.transfer_bond(
            cfg.lpt_receiver_addr,
            transferable,
            Address::zero(),
            Address::zero(),
            Address::zero(),
            Address::zero(),
        );

        match call.send().await {
            Ok(pending) => {
                let tx_hash = *pending;
                info!("transferBond tx sent: tx_hash={:?}", tx_hash);

                match timeout(Duration::from_secs(cfg.receipt_timeout_secs), pending).await {
                    Ok(Ok(Some(receipt))) => {
                        info!(
                            "transferBond confirmed: tx_hash={:?} status={:?} block={:?} gas_used={:?}",
                            receipt.transaction_hash,
                            receipt.status,
                            receipt.block_number,
                            receipt.gas_used
                        );
                    }
                    Ok(Ok(None)) => warn!(
                        "transferBond pending returned None receipt: tx_hash={:?}",
                        tx_hash
                    ),
                    Ok(Err(e)) => {
                        warn!("transferBond receipt error: tx_hash={:?} err={e}", tx_hash)
                    }
                    Err(_) => warn!(
                        "transferBond receipt timeout after {}s: tx_hash={:?}",
                        cfg.receipt_timeout_secs, tx_hash
                    ),
                }
            }
            Err(e) => warn!("transferBond send failed: {e}"),
        }
    }

    // ---- withdrawFees ----
    if pending_fees >= cfg.eth_fee_withdraw_threshold_wei && !pending_fees.is_zero() {
        info!(
            "withdrawFees sending: from_orchestrator={:?} to_receiver={:?} amountWei={}",
            orchestrator, cfg.eth_fee_receiver_addr, pending_fees
        );

        let call = bonding.withdraw_fees(cfg.eth_fee_receiver_addr, pending_fees);
        match call.send().await {
            Ok(pending) => {
                let tx_hash: TxHash = *pending;
                info!("withdrawFees tx sent: tx_hash={:?}", tx_hash);

                match timeout(Duration::from_secs(cfg.receipt_timeout_secs), pending).await {
                    Ok(Ok(Some(receipt))) => {
                        info!(
                            "withdrawFees confirmed: tx_hash={:?} status={:?} block={:?} gas_used={:?}",
                            receipt.transaction_hash,
                            receipt.status,
                            receipt.block_number,
                            receipt.gas_used
                        );
                    }
                    Ok(Ok(None)) => {
                        warn!(
                            "withdrawFees pending returned None receipt: tx_hash={:?}",
                            tx_hash
                        );
                    }
                    Ok(Err(e)) => {
                        warn!("withdrawFees receipt error: tx_hash={:?} err={e}", tx_hash);
                    }
                    Err(_) => {
                        warn!(
                            "withdrawFees receipt timeout after {}s: tx_hash={:?}",
                            cfg.receipt_timeout_secs, tx_hash
                        );
                    }
                }
            }
            Err(e) => {
                warn!("withdrawFees send failed: {e}");
            }
        }
    } else {
        debug!(
            "withdrawFees not needed: pendingFeesWei={} < thresholdWei={}",
            pending_fees, cfg.eth_fee_withdraw_threshold_wei
        );
    }

    Ok(())
}

fn init_logging() {
    // Respect RUST_LOG if set
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn load_config() -> Result<Config, AppError> {
    let http_rpc_url = must_env("HTTP_RPC_URL")?;
    let chain_id = must_parse_env_u64("CHAIN_ID")?;

    let rounds_manager_addr = must_parse_env_addr("ROUNDS_MANAGER_ADDR")?;
    let bonding_manager_addr = must_parse_env_addr("BONDING_MANAGER_ADDR")?;

    let json_key_file = must_env("JSON_KEY_FILE")?;
    let passphrase_file = must_env("PASSPHRASE_FILE")?;
    let orchestrator_addr = parse_env_addr_opt("ORCHESTRATOR_ADDR")?;

    let loop_sleep_secs = parse_env_u64_opt("LOOP_SLEEP_SECS")?.unwrap_or(6);
    let receipt_timeout_secs = parse_env_u64_opt("RECEIPT_TIMEOUT_SECS")?.unwrap_or(90);

    let lpt_receiver_addr = must_parse_env_addr("LPT_RECEIVER_ADDR")?;
    let lpt_min_retain_wei = must_parse_env_u256("LPT_MIN_RETAIN_WEI")?;

    let eth_fee_receiver_addr = must_parse_env_addr("ETH_FEE_RECEIVER_ADDR")?;
    let eth_fee_withdraw_threshold_wei = must_parse_env_u256("ETH_FEE_WITHDRAW_THRESHOLD_WEI")?;

    Ok(Config {
        http_rpc_url,
        chain_id,
        rounds_manager_addr,
        bonding_manager_addr,
        json_key_file,
        passphrase_file,
        orchestrator_addr,
        loop_sleep_secs,
        receipt_timeout_secs,
        lpt_receiver_addr,
        lpt_min_retain_wei,
        eth_fee_receiver_addr,
        eth_fee_withdraw_threshold_wei,
    })
}

fn must_env(key: &'static str) -> Result<String, AppError> {
    env::var(key).map_err(|_| AppError::MissingEnv(key))
}

fn parse_env_u64_opt(key: &'static str) -> Result<Option<u64>, AppError> {
    match env::var(key) {
        Ok(s) => {
            let v = s
                .parse::<u64>()
                .map_err(|e| AppError::BadEnv(key, format!("{e}")))?;
            Ok(Some(v))
        }
        Err(_) => Ok(None),
    }
}

fn must_parse_env_u64(key: &'static str) -> Result<u64, AppError> {
    let s = must_env(key)?;
    s.parse::<u64>()
        .map_err(|e| AppError::BadEnv(key, format!("{e}")))
}

fn parse_env_addr_opt(key: &'static str) -> Result<Option<Address>, AppError> {
    match env::var(key) {
        Ok(s) => {
            let a = s
                .parse::<Address>()
                .map_err(|e| AppError::BadEnv(key, format!("{e}")))?;
            Ok(Some(a))
        }
        Err(_) => Ok(None),
    }
}

fn must_parse_env_addr(key: &'static str) -> Result<Address, AppError> {
    let s = must_env(key)?;
    s.parse::<Address>()
        .map_err(|e| AppError::BadEnv(key, format!("{e}")))
}

fn must_parse_env_u256(key: &'static str) -> Result<U256, AppError> {
    let s = must_env(key)?;
    // Accept decimal strings
    U256::from_dec_str(&s).map_err(|e| AppError::BadEnv(key, format!("{e}")))
}
