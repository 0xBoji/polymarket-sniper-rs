#!/bin/bash
set -e

STACK_NAME="polymarket-hft-bot"
TEMPLATE_FILE="aws/cloudformation.yaml"

# Check AWS CLI
if ! command -v aws &> /dev/null; then
    echo "❌ AWS CLI could not be found. Please install and configure it."
    exit 1
fi

echo "🚀 Deploying Polymarket HFT Bot to AWS..."

# Get Key Pair
echo "🔑 Available Key Pairs:"
aws ec2 describe-key-pairs --query 'KeyPairs[*].KeyName' --output text

read -p "Enter the name of the EC2 Key Pair to use: " key_name

if [ -z "$key_name" ]; then
    echo "❌ Key Pair name is required."
    exit 1
fi

echo "⏳ Deploying CloudFormation stack '$STACK_NAME'..."
aws cloudformation deploy \
    --template-file $TEMPLATE_FILE \
    --stack-name $STACK_NAME \
    --parameter-overrides KeyName=$key_name \
    --capabilities CAPABILITY_IAM

echo "✅ Deployment complete!"

# Get Outputs
echo "Fetching instance details..."
aws cloudformation describe-stacks \
    --stack-name $STACK_NAME \
    --query 'Stacks[0].Outputs' \
    --output table

IP=$(aws cloudformation describe-stacks --stack-name $STACK_NAME --query "Stacks[0].Outputs[?OutputKey=='PublicIP'].OutputValue" --output text)

echo "
🎉 To connect to your bot:
    ssh -i /path/to/${key_name}.pem ec2-user@$IP

Next Steps:
1. Connect to the instance.
2. Clone/Upload your bot code to ~/bot.
3. Configure .env.
4. Run 'cargo run --release'.
"
