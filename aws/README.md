# Polymarket HFT Bot - AWS Deployment Guide

This directory contains the Infrastructure-as-Code (IaC) to deploy the bot to a high-performance AWS EC2 instance.

## 📂 Contents

- `cloudformation.yaml`: AWS CloudFormation template definition.
- `deploy.sh`: Interactive script to deploy/update the stack.

## 🚀 Deployment Instructions

### 1. Prerequisites
- **AWS CLI** installed and configured (`aws configure`).
- An **EC2 Key Pair** created in your target region (e.g., `us-east-1`).

### 2. Deploy
Run the deployment script from the project root:
```bash
./aws/deploy.sh
```
Follow the prompts to select your Key Pair. The script will output your instance's **Public IP**.

### 3. Setup the Bot
Connect to your new instance:
```bash
ssh -i /path/to/your-key.pem ec2-user@<PUBLIC_IP>
```

Once inside, the environment is already pre-configured with Rust, Git, and build tools.

**Clone & Configure:**
```bash
# Clone your repo (Assuming you pushed your latest code)
git clone https://github.com/0xBoji/polymarket-sniper-rs.git bot
cd bot

# Create .env file with your secrets
nano .env
# Paste:
# POLYGON_PRIVATE_KEY=...
# POLYMARKET_API_KEY=...
# ...
```

### 4. Run Live
Use `tmux` to keep the bot running after you disconnect.

```bash
# Start a new session
tmux new -s trader

# Run the bot in release mode (optimized)
cargo run --release

# Detach from session (Ctrl+B, then D)
```

To re-attach later:
```bash
tmux attach -t trader
```

## 🛠 Management through AWS Console

- **Stop/Start**: You can stop the instance when not trading to save costs.
- **Monitoring**: Check the "Monitoring" tab in EC2 Console for CPU/Network usage.
- **Scaling**: Update `cloudformation.yaml` (InstanceType) and run `./deploy.sh` again to upgrade the instance.
