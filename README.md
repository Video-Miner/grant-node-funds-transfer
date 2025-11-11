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

This service automates two critical tasks for Livepeer orchestrators:

1. **Bond Transfer**: Automatically transfers bonded LPT from an orchestrator wallet to a designated staking wallet, maintaining a minimum balance of 1 LPT in the orchestrator wallet
2. **Fee Withdrawal**: Monitors and withdraws accumulated ETH fees when they exceed a configurable threshold (default: 0.03 ETH)

The application operates on a 30-minute polling cycle and only executes transactions when the current Livepeer round is locked, ensuring safe and predictable operations.

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

- **Automated LPT Transfer**: Transfers excess bonded LPT while maintaining operational reserve
- **Fee Management**: Automatically withdraws accumulated fees above threshold
- **Round Awareness**: Only executes when Livepeer rounds are locked for safety
- **Configurable**: All parameters controlled via environment variables
- **Logging**: Comprehensive structured logging with tracing
- **Docker Support**: Production-ready containerization
- **Error Handling**: Robust error handling with transaction status monitoring

## Prerequisites

### Development

- Rust 1.75.0 or later
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
# Logging level (trace, debug, info, warn, error)
RUST_LOG=info

# Arbitrum RPC endpoint URL
RPC_ENDPOINT_URL=https://arb1.arbitrum.io/rpc

# Path to the passphrase file for keystore decryption
PASSPHRASE_FILE=/path/to/passphrase.txt

# Path to the encrypted JSON keystore file
JSON_KEY_FILE=/path/to/keystore.json

# Orchestrator wallet address (the wallet being monitored)
ORCH_ETH_ADDR=0xYourOrchestratorAddress

# Recipient address for withdrawn ETH fees
ETH_FEE_RECIPIENT_ETH_ADDR=0xYourFeeRecipientAddress

# Recipient address for transferred bonded LPT
TRANSFER_BOND_RECIPIENT_ETH_ADDR=0xYourStakeRecipientAddress

# Chain ID (Arbitrum One = 42161, Arbitrum Sepolia = 421614)
CHAIN_ID=42161
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

The following are hardcoded in the application but can be modified in `src/bin/funds_transfer.rs`:

- **pending_fee_threshold**: `0.03` ETH - Minimum fees before withdrawal
- **sleep_timer_secs**: `1800` seconds (30 minutes) - Polling interval
- **Reserve LPT**: `1.0` LPT - Minimum balance kept in orchestrator wallet

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

The application will:

1. Load configuration from environment variables
2. Connect to the Arbitrum RPC endpoint
3. Enter a continuous loop that:
   - Checks if the current round is locked
   - If locked:
     - Checks orchestrator's bonded LPT balance
     - Transfers excess LPT (if > 1 LPT total)
     - Checks accumulated fees
     - Withdraws fees (if ≥ 0.03 ETH)
   - If not locked:
     - Waits for round to lock
   - Sleeps for 30 minutes
   - Repeats

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
INFO  - Configuration loaded successfully
INFO  - Current Round [X] is locked
INFO  - Total stake [Y] ETH
INFO  - Transfer Bond Transaction sent successfully. Hash: 0x...
INFO  - Pending fees [Z] Threshold to withdraw [0.03]
INFO  - Withdraw Fees Transaction sent successfully. Hash: 0x...
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
- Regular log output every 30 minutes
- Successful transaction hashes
- No error messages in logs
- Recipient wallet balances increasing

### Operational Scenarios

#### Scenario 1: Normal Operation
```
[INFO] Current Round [1234] is locked
[INFO] Total stake [10.5] ETH
[INFO] Stake ready for transfer [9500000000000000000] WEI
[INFO] Transfer Bond Transaction sent successfully. Hash: 0xabc...
[INFO] Pending fees [0.05] Threshold to withdraw [0.03]
[INFO] Fees ready for transfer [0.05] ETH
[INFO] Withdraw Fees Transaction sent successfully. Hash: 0xdef...
```

#### Scenario 2: Insufficient Balance
```
[INFO] Current Round [1234] is locked
[INFO] Total stake [0.8] ETH
[INFO] Pending fees [0.02] Threshold to withdraw [0.03]
```
No transactions executed - waiting for thresholds to be met.

#### Scenario 3: Round Not Locked
```
[INFO] Current Round [1234] is not locked. No transfers until the round is locked.
```
Application waits 30 minutes and checks again.

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

#### 1. "RPC_ENDPOINT_URL missing"
**Cause**: Environment variable not set
**Solution**: 
```bash
export RPC_ENDPOINT_URL=https://arb1.arbitrum.io/rpc
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
3. Test RPC connectivity: `curl -X POST $RPC_ENDPOINT_URL -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'`
4. Verify wallet address has necessary permissions in Livepeer protocol
5. Review Livepeer Protocol documentation for round and staking mechanics

## Advanced Configuration

### Modifying Operational Parameters

Edit `src/bin/funds_transfer.rs`:

```rust
// Change fee withdrawal threshold (default 0.03 ETH)
let pending_fee_threshold = 0.05;

// Change polling interval (default 1800 seconds / 30 minutes)
let sleep_timer_secs = 3600; // 1 hour

// Change reserve LPT (default 1.0 LPT)
let lpt_to_transfer_bond = orch_pending_stake_wei - (2 * one_eth_in_wei);
```

After modifications, rebuild the application.

### Using Alternative Networks

For Arbitrum Sepolia testnet:
```bash
CHAIN_ID=421614
RPC_ENDPOINT_URL=https://sepolia-rollup.arbitrum.io/rpc
```

Update contract addresses in code if different on testnet.

## Performance Considerations

- **RPC Rate Limits**: The 30-minute polling interval is conservative. Adjust based on your RPC provider's limits
- **Gas Costs**: Monitor gas prices on Arbitrum - typically very low compared to mainnet
- **Memory Usage**: Application uses minimal memory (~10-20MB) during operation
- **CPU Usage**: Negligible except during transaction signing

## License

[Specify your license here]

## Contributing

[Add contribution guidelines if applicable]

## Support

For issues related to:
- **This application**: [Create an issue in the repository]
- **Livepeer Protocol**: Visit [Livepeer Discord](https://discord.gg/livepeer)
- **Arbitrum Network**: Visit [Arbitrum Discord](https://discord.gg/arbitrum)

## Version History

- **0.0.1**: Initial release with basic bond transfer and fee withdrawal functionality

---

**Disclaimer**: This software is provided as-is. Always test thoroughly in a testnet environment before deploying to production with real funds. The authors assume no liability for any losses incurred through the use of this software.
