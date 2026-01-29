#!/bin/bash
# start.sh - Automated startup with CloudWatch log cleanup

LOG_GROUP="Polymarket-HFT-Bot"
LOG_FILE="bot.log"

echo "🧹 Cleaning up old logs..."

# 1. Truncate local log file
if [ -f "$LOG_FILE" ]; then
    echo "" > "$LOG_FILE"
    echo "✅ Local $LOG_FILE truncated."
fi

# 2. Delete CloudWatch Log Group to clear console
# This will be recreated automatically by the CloudWatch Agent or the next run
if command -v aws &> /dev/null; then
    REGION=$(curl -s http://127.0.0.1/latest/meta-data/placement/region || echo "us-east-1")
    echo "🗑️ Deleting CloudWatch Log Group: $LOG_GROUP in $REGION..."
    aws logs delete-log-group --log-group-name "$LOG_GROUP" --region "$REGION" || echo "⚠️ Could not delete log group (maybe already gone or no permission)"
    
    # Wait a moment for deletion to propagate
    sleep 2
    
    # Re-create the log group so the agent doesn't complain
    aws logs create-log-group --log-group-name "$LOG_GROUP" --region "$REGION" || echo "⚠️ Could not recreate log group"
    echo "✅ CloudWatch logs cleared."
fi

echo "🚀 Starting Polymarket HFT Bot..."
# Start the bot in the background or foreground as needed
# Using RUST_LOG from .env
export $(grep -v '^#' .env | xargs)
./target/release/polymarket-hft-agent 2>&1 | tee -a "$LOG_FILE"
