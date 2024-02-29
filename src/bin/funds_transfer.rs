use dotenv::dotenv;
use ethers::core::utils::format_units;
use ethers::middleware::SignerMiddleware;
use ethers::prelude::*;
use ethers::providers::{Provider, Http};
use ethers::signers::LocalWallet;
use ethers::types::{Address, U256};
use std::path::Path;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, trace};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let rust_log = std::env::var("RUST_LOG").expect("RUST_LOG missing");
    let arb_rpc_url = std::env::var("RPC_ENDPOINT_URL").expect("RPC_ENDPOINT_URL missing");
    let passphrase_file = std::env::var("PASSPHRASE_FILE").expect("PASSPHRASE_FILE missing");

    let key_json_file = std::env::var("JSON_KEY_FILE").expect("JSON_KEY_FILE missing");

    let orch_eth_addr = std::env::var("ORCH_ETH_ADDR").expect("ORCH_ETH_ADDR missing");

    let fee_recipient_eth_addr =
        std::env::var("ETH_FEE_RECIPIENT_ETH_ADDR").expect("ETH_FEE_RECIPIENT_ETH_ADDR missing");
    let transfer_bond_recipient_eth_addr = std::env::var("TRANSFER_BOND_RECIPIENT_ETH_ADDR")
        .expect("TRANSFER_BOND_RECIPIENT_ETH_ADDR missing");
    let chain_id = std::env::var("CHAIN_ID").expect("CHAIN_ID missing");
    let chain_id = chain_id.parse::<u64>().expect("CHAIN_ID is not a u64 type");

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or(rust_log),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let livepeer_bonding_manager_address =
        "0x35Bcf3c30594191d53231E4FF333E8A770453e40".parse::<Address>()?;
    let livepeer_rounds_manager_address =
        "0xdd6f56DcC28D3F5f27084381fE8Df634985cc39f".parse::<Address>()?;

    info!(
        "transfer_bond_recipient_eth_address [{}] ",
        &transfer_bond_recipient_eth_addr
    );
    let transfer_bond_recipient_eth_address =
        transfer_bond_recipient_eth_addr.parse::<Address>()?;

    info!("fee_recipient_eth_addr [{}] ", &fee_recipient_eth_addr);
    let fee_recipient_eth_address = fee_recipient_eth_addr.parse::<Address>()?;

    info!("orch_eth_addr [{}] ", &orch_eth_addr);
    let orch_wallet = orch_eth_addr.parse::<Address>()?;

    info!("chain id [{}] ", &chain_id);

    let address_zero = "0x0000000000000000000000000000000000000000".parse::<Address>()?;

    let pending_fee_threshold = 0.03;
    let one_eth_in_wei: i64 = 1000000000000000000;

    // load the passhpase and private key json files to construct the Orch Wallet
    info!("loading passphrase file name [{}] ", &passphrase_file);
    let passphrase = std::fs::read_to_string(passphrase_file)
        .expect("could open passphrase file")
        .trim_end()
        .to_string();
    let key_json = Path::new(&key_json_file);
    info!(
        "loading private key json file name [{}] ",
        &key_json.display()
    );
    let wallet = LocalWallet::decrypt_keystore(key_json, passphrase)
        .expect("could not load wallet with key and passphrase provided");
    // 1800 seconds is 30 mins
    let sleep_timer_secs = 1800;

    let mut last_locked_round = None;

    loop {
        trace!("Start loop ");
        let provider: Arc<Provider<Http>> = Arc::new(Provider::<Http>::connect(&arb_rpc_url).await);
        let lpt_rounds_manager =
            RoundsManager::new(livepeer_rounds_manager_address, provider.clone());

        let wallet = wallet.clone().with_chain_id(chain_id);
        let end_round = U256::from(99999);

        // STEP 0: make sure round is locked. check every 30 mins
        let current_round = lpt_rounds_manager.current_round().await.unwrap();
        let is_round_locked = lpt_rounds_manager.current_round_locked().await.unwrap();

        if !is_round_locked {
            info!(
                "Current Round [{}] is not locked. No transfers until the round is locked.",
                &current_round
            );
            trace!("Round Not Locked sleep ... ");
            sleep(Duration::from_secs(sleep_timer_secs)).await;
            trace!("Round Not Locked awake ... ");
            continue;
        }
        //set the last locked round, so we can compare the current round to the last locked round
        //if they are different, then its ok to call reward.
        match &last_locked_round{
            Some(last_round)=>{
                info!("last_round {} current {}",&last_round,&current_round);

                // if last_round != current_round {
                //     todo!();
                // }
            },
            None=>{
                last_locked_round = Some(current_round.clone());
            }
        }

        info!(
            "Current Round [{}] is locked. Last Locked Round [{:?}].",
            &current_round,&last_locked_round
        );

        let client = Arc::new(SignerMiddleware::new(provider, wallet));
        let lpt_bonding_manager =
            BondingManager::new(livepeer_bonding_manager_address, client.clone());

        // TASK 1 - Transfer LPT from "Orch" wallet to "Stake" Wallet
        info!(
            "Begin to transfer bonded LPT...",
        );
        // STEP 1: check the balance of LPT for the "Orch" wallet.
        let orch_pending_stake_wei = lpt_bonding_manager
            .pending_stake(orch_wallet, end_round)
            .await
            .unwrap();
        info!("Total stake [{:?}] WEI", &orch_pending_stake_wei);
        let orch_pending_stake: f64 = format_units(orch_pending_stake_wei, "ether")
            .unwrap()
            .parse::<f64>()
            .unwrap();
        info!("Total stake [{}] ETH", &orch_pending_stake);

        if orch_pending_stake > 1.0 {
            // STEP 2: calc the total - 1 LPT

            let lpt_to_transfer_bond = orch_pending_stake_wei - one_eth_in_wei;
            info!("Stake ready for transfer [{}] WEI", &lpt_to_transfer_bond);

            //  STEP 3:  transfer bond from ORCH wallet to Livepeer STAKE wallet
            let transfer_bond_response = lpt_bonding_manager.transfer_bond(
                transfer_bond_recipient_eth_address,
                lpt_to_transfer_bond,
                address_zero,
                address_zero,
                address_zero,
                address_zero,
            );
            let result = transfer_bond_response.send().await;
            if let Ok(tx_hash) = result {
                info!(
                    "Transfer Bond Transaction sent successfully. Hash: {:?}",
                    tx_hash
                );
            } else {
                info!("Transfer Bond Transaction failed");
            }
        }

        // TASK 2
        // STEP 1: Check for fees to withdraw for the Orch wallet
        let orch_pending_fees = lpt_bonding_manager
            .pending_fees(orch_wallet, end_round)
            .await
            .unwrap();
        let orch_pending_fees_f64: f64 = format_units(orch_pending_fees, "ether")
            .unwrap()
            .parse::<f64>()
            .unwrap();

        info!(
            "Pending fees [{}] Threshold to withdraw [{}]",
            &orch_pending_fees_f64, &pending_fee_threshold
        );

        //  STEP 2: if the fees are greater than a WITHDRAW THRESHOLD:
        if orch_pending_fees_f64 >= pending_fee_threshold {
            info!(
                "Fees ready for transfer [{}] ETH [{}] WEI",
                &orch_pending_fees_f64, &orch_pending_fees
            );

            // STEP 3: Withdraw the fees from LP smart contract and transfer the entire ETH balance to the Pool "payout" wallet
            let withdraw_response =
                lpt_bonding_manager.withdraw_fees(fee_recipient_eth_address, orch_pending_fees);
            let result = withdraw_response.send().await;
            if let Ok(tx_hash) = result {
                info!(
                    "Withdraw Fees Transaction sent successfully. Hash: {:?}",
                    tx_hash
                );
            } else {
                info!("Withdraw Fees Transaction failed");
            }
        }
        trace!("sleep time ... ");
        sleep(Duration::from_secs(sleep_timer_secs)).await;
        trace!("awake ... ");
    }
}
