# Livepeer Funds Transfer

A Rust-based automated service for managing Livepeer Protocol token (LPT) staking operations on Arbitrum. This application periodically transfers bonded LPT tokens and withdraws accumulated fees from an orchestrator wallet to designated recipient addresses.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Features](#features)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Configuration](#configuration)
- [Building](#building)
- [Running](#running)
- [Docker Deployment](#docker-deployment)
- [Operations](#operations)
- [Security Considerations](#security-considerations)
- [Troubleshooting](#troubleshooting)

## Overview

This service automates Livepeer orchestrator operations by monitoring the Livepeer round lifecycle and executing actions based on on-chain round state.

The application performs the following tasks:

1. **Reward Claiming**: Calls `reward()` exactly once per round when a new round is initialized
2. **Bond Transfer**: Transfers bonded LPT from the orchestrator wallet once the round is locked, while retaining a minimum bonded balance
3. **Fee Withdrawal**: Withdraws accumulated ETH fees when they exceed a configurable threshold

The application runs continuously using a polling loop and safely retries failed actions on subsequent iterations.

## Architecture

### Components

```
livepeer-funds-transfer/
├── Cargo.toml              # Rust dependencies and project metadata
├── Dockerfile              # Multi-stage Docker build configuration
├── src/
│   ├── bin/
│   │   └── funds_transfer.rs    # Main application logic
│   └── abi/
│       ├── BondingManager.json  # Livepeer BondingManager contract ABI
│       └── RoundsManager.json   # Livepeer RoundsManager contract ABI
└── README.md
```

### Key Dependencies

- **tokio**: Async runtime for concurrent operations
- **ethers**: Ethereum/Arbitrum blockchain interaction library
- **dotenv**: Environment variable management
- **serde/serde_json**: Serialization/deserialization
- **tracing**: Structured logging and diagnostics
- **chrono**: Date and time utilities

### Smart Contract Integration

The application interacts with two Livepeer Protocol smart contracts on Arbitrum:

- **BondingManager** (`0x35Bcf3c30594191d53231E4FF333E8A770453e40`): Manages staking, bonding, and fee operations
- **RoundsManager** (`0xdd6f56DcC28D3F5f27084381fE8Df634985cc39f`): Manages protocol rounds and their states

## Features

- **Round-Aware Rewarding**: Calls `reward()` once per initialized round using on-chain state to prevent duplicates
- **Automated LPT Transfer**: Transfers all excess bonded LPT while retaining a minimum bonded amount on the orchestrator
- **Fee Management**: Withdraws accumulated ETH fees above a configurable threshold
- **Round Safety**: Bond transfers and fee withdrawals only occur when the round is locked
- **Polling-Based Execution**: Runs continuously and reacts to round state changes
- **Keystore-Based Signing**: Uses encrypted JSON keystore and passphrase files (no private keys in env vars)
- **Configurable**: All operational parameters controlled via environment variables
- **Logging**: Structured logging suitable for auditing and monitoring
- **Docker Support**: Production-ready containerization
- **Error Handling**: No panics; failures are logged and retried on the next polling cycle

## Prerequisites

### Development

- Rust 1.90.0 or later
- Cargo (comes with Rust)
- Access to an Arbitrum RPC endpoint
- Encrypted Ethereum keystore file (JSON format)
- Passphrase file for keystore decryption

### Production

- Docker (for containerized deployment)
- Kubernetes/orchestration platform (optional)

## Installation

### Clone the Repository

```bash
git clone <repository-url>
cd livepeer-funds-transfer
```

### Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

## Configuration

### Environment Variables

Create a `.env` file in the project root with the following variables:

```bash
# Logging configuration
RUST_LOG=funds_transfer=info

# Arbitrum RPC endpoint
HTTP_RPC_URL=https://arb1.arbitrum.io/rpc

# Path to encrypted keystore file
JSON_KEY_FILE=/path/to/keystore.json

# Path to keystore passphrase file
PASSPHRASE_FILE=/path/to/passphrase.txt

# Optional: orchestrator address (derived from keystore if omitted)
# ORCHESTRATOR_ADDR=0xYourOrchestratorAddress

# Recipient address for transferred bonded LPT
LPT_RECEIVER_ADDR=0xYourStakeRecipientAddress

# Minimum bonded LPT to retain on the orchestrator (wei)
# Example: 1 LPT = 1000000000000000000
LPT_MIN_RETAIN_WEI=1000000000000000000

# Recipient address for withdrawn ETH fees
ETH_FEE_RECEIVER_ADDR=0xYourFeeRecipientAddress

# Minimum ETH fees required before withdrawal (wei)
# Example:          0.03 ETH = 30000000000000000
ETH_FEE_WITHDRAW_THRESHOLD_WEI=30000000000000000

# Chain ID (Arbitrum One = 42161)
CHAIN_ID=42161

# Polling interval in seconds
LOOP_SLEEP_SECS=60

```

### Configuration Details

#### RPC_ENDPOINT_URL
- Public Arbitrum RPC: `https://arb1.arbitrum.io/rpc`
- Consider using a dedicated RPC provider (Infura, Alchemy, QuickNode) for production
- Rate limits may apply to public endpoints

#### Wallet Setup

1. **Generate Keystore**:
   ```bash
   # Using geth or similar tool
   geth account new --keystore ./keystore
   ```

2. **Create Passphrase File**:
   ```bash
   echo "your-secure-passphrase" > passphrase.txt
   chmod 600 passphrase.txt
   ```

3. **Security Note**: Never commit keystore files or passphrases to version control

#### Address Configuration

- **ORCH_ETH_ADDR**: The orchestrator wallet that holds bonded LPT and accumulates fees
- **TRANSFER_BOND_RECIPIENT_ETH_ADDR**: Destination for transferred LPT bonds (typically your main staking wallet)
- **ETH_FEE_RECIPIENT_ETH_ADDR**: Destination for withdrawn ETH fees (can be the same as bond recipient)

#### Operational Parameters

All operational parameters are configurable via environment variables:

- **LPT_MIN_RETAIN_WEI**  
  Minimum bonded LPT that must remain on the orchestrator after transfers.

- **ETH_FEE_WITHDRAW_THRESHOLD_WEI**  
  ETH fees must meet or exceed this value before withdrawal.

- **LOOP_SLEEP_SECS**  
  Polling interval. Failed actions are retried on the next loop iteration.

These values can be adjusted without rebuilding the application.

## Building

### Development Build

```bash
cargo build
```

### Release Build (Optimized)

```bash
cargo build --release
```

The compiled binary will be located at:
- Debug: `./target/debug/funds_transfer`
- Release: `./target/release/funds_transfer`

### Build with Specific Features

```bash
# With all features
cargo build --all-features

# Check for compilation errors without building
cargo check
```

## Running

### Local Execution

1. **Set up environment**:
   ```bash
   cp .env.example .env
   # Edit .env with your configuration
   ```

2. **Run the application**:
   ```bash
   # Using cargo (development)
   cargo run

   # Or run the compiled binary
   ./target/release/funds_transfer
   ```

3. **With custom logging**:
   ```bash
   RUST_LOG=debug cargo run
   ```

### Application Behavior

The application operates in a continuous loop:

1. Loads configuration and decrypts the keystore
2. Polls the Livepeer `RoundsManager` contract to determine:
   - Current round number
   - Whether the round is initialized
   - Whether the round is locked
3. If the round is initialized:
   - Calls `reward()` once per round if it has not already been called
4. If the round is locked:
   - Transfers bonded LPT in excess of `LPT_MIN_RETAIN_WEI`
   - Withdraws ETH fees if they exceed the configured threshold
5. Sleeps for `LOOP_SLEEP_SECS`
6. Repeats

If a transaction fails, the error is logged and the operation is retried on the next polling cycle.

### Stopping the Application

- Press `Ctrl+C` to gracefully stop
- The application will complete its current operation before exiting

## Docker Deployment

### Building the Docker Image

```bash
docker build -t livepeer-funds-transfer:latest .
```

### Running with Docker

```bash
docker run -d \
  --name livepeer-funds-transfer \
  --env-file .env \
  -v /path/to/keystore:/root/keystore \
  -v /path/to/passphrase.txt:/root/passphrase.txt \
  livepeer-funds-transfer:latest
```

### Docker Compose

Create a `docker-compose.yml`:

```yaml
version: '3.8'

services:
  funds-transfer:
    build: .
    container_name: livepeer-funds-transfer
    restart: unless-stopped
    env_file:
      - .env
    volumes:
      - ./keystore:/root/keystore:ro
      - ./passphrase.txt:/root/passphrase.txt:ro
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
```

Run with:
```bash
docker-compose up -d
```

### Multi-Stage Build Benefits

The Dockerfile uses a multi-stage build:
1. **Builder stage**: Compiles the Rust application with all dependencies
2. **Runtime stage**: Creates minimal Debian-based image with only the binary

This reduces the final image size significantly (from ~2GB to ~100MB).

## Operations

### Monitoring

#### Log Messages

The application provides detailed logging at various levels:

```
2025-12-20T14:40:54.859032Z  INFO funds_transfer: starting funds_transfer: chain_id=42161 rounds_manager=0xdd6f56dcc28d3f5f27084381fe8df634985cc39f bonding_manager=0x35bcf3c30594191d53231e4ff333e8a770453e40 sleep_secs=60 flags(reward=true, transfer_bond=true, withdraw_fees=true)
2025-12-20T14:40:55.527227Z  INFO funds_transfer: orchestrator/signer address: 0xYourOrchAddress
2025-12-20T14:40:55.756562Z  INFO funds_transfer: round state changed: round=4035 initialized=true locked=false
```

#### Viewing Logs

```bash
# Docker logs
docker logs -f livepeer-funds-transfer

# Docker Compose logs
docker-compose logs -f

# System journal (if running as systemd service)
journalctl -u livepeer-funds-transfer -f
```

### Transaction Verification

After each transaction, verify on Arbiscan:
- Bond transfers: Check BondingManager contract events
- Fee withdrawals: Check ETH transfers to recipient address

### Health Checks

Monitor for:
- Regular log output every 60 seconds
- Successful transaction hashes
- No error messages in logs
- Recipient wallet balances increasing

### Operational Scenarios

#### Scenario 1: Normal Operation
```
2025-12-20T18:29:38.310281Z  INFO funds_transfer: round state changed: round=4035 initialized=true locked=true
2025-12-20T18:29:38.404593Z  INFO funds_transfer: transferBond sending: round=4035 from_orchestrator=0xYourOrchAddress to_receiver=0xYourStakeOrTreasuryAddress amountWei=1083763440135192443657
2025-12-20T18:29:39.201740Z  INFO funds_transfer: transferBond tx sent: round=4035 tx_hash=0xc5....
2025-12-20T18:29:39.541239Z  INFO funds_transfer: transferBond confirmed: round=4035 tx_hash=0xc5... status=Some(1) block=Some(412709680) gas_used=Some(532629)
2025-12-20T18:29:39.587134Z  INFO funds_transfer: withdrawFees sending: round=4035 from_orchestrator=0xYourOrchAddress to_receiver=0xYourFeeRecipientAddress amountWei=7200000000000000
2025-12-20T18:29:40.193946Z  INFO funds_transfer: withdrawFees tx sent: round=4035 tx_hash=0x93eec...
2025-12-20T18:29:40.529429Z  INFO funds_transfer: withdrawFees confirmed: round=4035 tx_hash=0x93eec... status=Some(1) block=Some(412709684) gas_used=Some(153100)
```

## Security Considerations

### Key Management

1. **Keystore Protection**:
   - Store keystore files in secure locations with restricted permissions (`chmod 600`)
   - Use strong passphrases (20+ characters, mixed case, numbers, symbols)
   - Never commit keystores to version control
   - Consider hardware wallets for production environments

2. **Passphrase Security**:
   - Store passphrase files separately from keystores
   - Use environment variables or secrets management systems in production
   - Rotate passphrases regularly

### Network Security

1. **RPC Endpoints**:
   - Use authenticated RPC endpoints when possible
   - Consider running your own Arbitrum node for maximum security
   - Implement rate limiting and request filtering

2. **Firewall Rules**:
   - Restrict outbound connections to necessary RPC endpoints
   - Block all inbound connections except monitoring

### Operational Security

1. **Minimum Privilege**:
   - Run the application with minimal necessary permissions
   - Use dedicated service accounts
   - Implement principle of least privilege

2. **Monitoring**:
   - Set up alerts for failed transactions
   - Monitor wallet balances regularly
   - Track gas costs and fee withdrawals

3. **Auditing**:
   - Review logs regularly
   - Verify all transactions on-chain
   - Maintain audit trail of configuration changes

### Smart Contract Risks

1. **Contract Addresses**: The hardcoded Livepeer contract addresses are for Arbitrum mainnet. Verify these before deployment.
2. **Protocol Upgrades**: Monitor Livepeer governance for protocol upgrades that might affect these addresses
3. **Gas Prices**: The application uses default gas estimation - monitor for failed transactions during high network congestion

## Troubleshooting

### Common Issues

#### 1. "HTTP_RPC_URL missing"
**Cause**: Environment variable not set
**Solution**: 
```bash
export HTTP_RPC_URL=https://arb1.arbitrum.io/rpc
# Or add to .env file
```

#### 2. "could not load wallet with key and passphrase provided"
**Cause**: Incorrect passphrase or corrupted keystore
**Solution**: 
- Verify passphrase file contains correct passphrase (no trailing newlines)
- Ensure keystore file is valid JSON
- Check file permissions

#### 3. "Transfer Bond Transaction failed"
**Causes**:
- Insufficient gas
- Round not properly locked
- Invalid recipient address
- Insufficient LPT balance

**Solution**: 
- Check wallet has enough ETH for gas
- Verify round status on Livepeer Explorer
- Confirm recipient address is valid
- Review logs for specific error messages

#### 4. Connection timeouts
**Cause**: RPC endpoint unreachable or rate limited
**Solution**:
- Try alternative RPC endpoint
- Use authenticated/paid RPC service
- Increase timeout values (requires code modification)

#### 5. "Current Round is not locked"
**Cause**: Normal operation - rounds are not always locked
**Solution**: Wait for next round lock (rounds lock periodically in Livepeer protocol)

### Debug Mode

Enable verbose logging:
```bash
RUST_LOG=debug cargo run
```

Or for even more detail:
```bash
RUST_LOG=trace cargo run
```

### Testing Without Transactions

To test configuration without executing real transactions, you can:
1. Use a testnet (change CHAIN_ID to 421614 for Arbitrum Sepolia)
2. Modify the code to add a "dry-run" mode
3. Use a test wallet with minimal funds

### Getting Help

1. Check application logs first
2. Verify all environment variables are set correctly
3. Test RPC connectivity: `curl -X POST $HTTP_RPC_URL -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'`
4. Verify wallet address has necessary permissions in Livepeer protocol
5. Review Livepeer Protocol documentation for round and staking mechanics

### Using Alternative Networks

For Arbitrum Sepolia testnet:
```bash
CHAIN_ID=421614
HTTP_RPC_URL=https://sepolia-rollup.arbitrum.io/rpc
```

Update contract addresses in code if different on testnet.

## Performance Considerations

- **RPC Rate Limits**: The 30-minute polling interval is conservative. Adjust based on your RPC provider's limits
- **Gas Costs**: Monitor gas prices on Arbitrum - typically very low compared to mainnet
- **Memory Usage**: Application uses minimal memory (~10-20MB) during operation
- **CPU Usage**: Negligible except during transaction signing

## License

MIT

## Contributing

[Add contribution guidelines if applicable]

## Support

For issues related to:
- **This application**: [Create an issue in the repository]
- **Livepeer Protocol**: Visit [Livepeer Discord](https://discord.gg/livepeer)
- **Arbitrum Network**: Visit [Arbitrum Discord](https://discord.gg/arbitrum)

---

**Disclaimer**: This software is provided as-is. Always test thoroughly in a testnet environment before deploying to production with real funds. The authors assume no liability for any losses incurred through the use of this software.
