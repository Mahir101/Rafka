#!/bin/bash

echo "🚀 Preparing Rafka-rs for publishing to crates.io"
echo "=============================================="

# Function to replace path dependencies with version dependencies
replace_dependencies() {
    local file="$1"
    echo "📝 Updating $file..."
    
    # Replace path dependencies with version dependencies
    sed -i.bak 's/rafka-core = { path = "\.\.\/core" }/rafka-core = "0.1.0"/g' "$file"
    sed -i.bak 's/rafka-core = { path = "crates\/core" }/rafka-core = "0.1.0"/g' "$file"
    sed -i.bak 's/rafka-storage = { path = "\.\.\/storage" }/rafka-storage = "0.1.0"/g' "$file"
    sed -i.bak 's/rafka-broker = { path = "crates\/broker" }/rafka-broker = "0.1.0"/g' "$file"
    sed -i.bak 's/rafka-producer = { path = "crates\/producer" }/rafka-producer = "0.1.0"/g' "$file"
    sed -i.bak 's/rafka-consumer = { path = "crates\/consumer" }/rafka-consumer = "0.1.0"/g' "$file"
    sed -i.bak 's/rafka-storage = { path = "crates\/storage" }/rafka-storage = "0.1.0"/g' "$file"
    
    # Temporarily comment out workspace section in main Cargo.toml
    if [[ "$file" == "Cargo.toml" ]]; then
        echo "📝 Temporarily commenting out workspace section..."
        sed -i.bak 's/^\[workspace\]/# [workspace]/g' "$file"
        sed -i.bak 's/^members = \[/# members = [/g' "$file"
        sed -i.bak 's/^    "crates\/broker",/#     "crates\/broker",/g' "$file"
        sed -i.bak 's/^    "crates\/core",/#     "crates\/core",/g' "$file"
        sed -i.bak 's/^    "crates\/producer",/#     "crates\/producer",/g' "$file"
        sed -i.bak 's/^    "crates\/consumer",/#     "crates\/consumer",/g' "$file"
        sed -i.bak 's/^    "crates\/storage"/#     "crates\/storage"/g' "$file"
        sed -i.bak 's/^\]/# ]/g' "$file"
    fi
}

# Function to restore path dependencies
restore_dependencies() {
    local file="$1"
    echo "🔄 Restoring $file..."
    
    # Restore path dependencies
    sed -i.bak 's/rafka-core = "0.1.0"/rafka-core = { path = "..\/core" }/g' "$file"
    sed -i.bak 's/rafka-core = "0.1.0"/rafka-core = { path = "crates\/core" }/g' "$file"
    sed -i.bak 's/rafka-storage = "0.1.0"/rafka-storage = { path = "..\/storage" }/g' "$file"
    sed -i.bak 's/rafka-broker = "0.1.0"/rafka-broker = { path = "crates\/broker" }/g' "$file"
    sed -i.bak 's/rafka-producer = "0.1.0"/rafka-producer = { path = "crates\/producer" }/g' "$file"
    sed -i.bak 's/rafka-consumer = "0.1.0"/rafka-consumer = { path = "crates\/consumer" }/g' "$file"
    sed -i.bak 's/rafka-storage = "0.1.0"/rafka-storage = { path = "crates\/storage" }/g' "$file"
    
    # Restore workspace section in main Cargo.toml
    if [[ "$file" == "Cargo.toml" ]]; then
        echo "🔄 Restoring workspace section..."
        sed -i.bak 's/^# \[workspace\]/[workspace]/g' "$file"
        sed -i.bak 's/^# members = \[/members = [/g' "$file"
        sed -i.bak 's/^#     "crates\/broker",/    "crates\/broker",/g' "$file"
        sed -i.bak 's/^#     "crates\/core",/    "crates\/core",/g' "$file"
        sed -i.bak 's/^#     "crates\/producer",/    "crates\/producer",/g' "$file"
        sed -i.bak 's/^#     "crates\/consumer",/    "crates\/consumer",/g' "$file"
        sed -i.bak 's/^#     "crates\/storage"/    "crates\/storage"/g' "$file"
        sed -i.bak 's/^# ]/]/g' "$file"
    fi
}

# Function to publish crates in dependency order
publish_crates() {
    echo "📦 Publishing crates in dependency order..."
    
    # Publish core first (no dependencies)
    echo "1️⃣ Publishing rafka-core..."
    cd crates/core
    cargo publish --allow-dirty
    if [ $? -ne 0 ]; then
        echo "❌ Failed to publish rafka-core"
        exit 1
    fi
    cd ../..
    
    # Publish storage (depends on core)
    echo "2️⃣ Publishing rafka-storage..."
    cd crates/storage
    cargo publish --allow-dirty
    if [ $? -ne 0 ]; then
        echo "❌ Failed to publish rafka-storage"
        exit 1
    fi
    cd ../..
    
    # Publish producer (depends on core)
    echo "3️⃣ Publishing rafka-producer..."
    cd crates/producer
    cargo publish --allow-dirty
    if [ $? -ne 0 ]; then
        echo "❌ Failed to publish rafka-producer"
        exit 1
    fi
    cd ../..
    
    # Publish consumer (depends on core)
    echo "4️⃣ Publishing rafka-consumer..."
    cd crates/consumer
    cargo publish --allow-dirty
    if [ $? -ne 0 ]; then
        echo "❌ Failed to publish rafka-consumer"
        exit 1
    fi
    cd ../..
    
    # Publish broker (depends on core and storage)
    echo "5️⃣ Publishing rafka-broker..."
    cd crates/broker
    cargo publish --allow-dirty
    if [ $? -ne 0 ]; then
        echo "❌ Failed to publish rafka-broker"
        exit 1
    fi
    cd ../..
    
    # Publish main workspace
    echo "6️⃣ Publishing main workspace..."
    cargo publish --allow-dirty
    if [ $? -ne 0 ]; then
        echo "❌ Failed to publish main workspace"
        exit 1
    fi
}

# Main execution
case "$1" in
    "prepare")
        echo "🔧 Preparing for publishing..."
        replace_dependencies "Cargo.toml"
        replace_dependencies "crates/broker/Cargo.toml"
        replace_dependencies "crates/producer/Cargo.toml"
        replace_dependencies "crates/consumer/Cargo.toml"
        replace_dependencies "crates/storage/Cargo.toml"
        echo "✅ Ready for publishing!"
        echo "💡 Run: ./scripts/prepare_for_publish.sh publish"
        ;;
    "publish")
        echo "📦 Publishing all crates..."
        publish_crates
        echo "✅ All crates published successfully!"
        ;;
    "restore")
        echo "🔄 Restoring local development setup..."
        restore_dependencies "Cargo.toml"
        restore_dependencies "crates/broker/Cargo.toml"
        restore_dependencies "crates/producer/Cargo.toml"
        restore_dependencies "crates/consumer/Cargo.toml"
        restore_dependencies "crates/storage/Cargo.toml"
        echo "✅ Restored to local development setup!"
        ;;
    *)
        echo "Usage: $0 {prepare|publish|restore}"
        echo ""
        echo "Commands:"
        echo "  prepare  - Replace path dependencies with version dependencies"
        echo "  publish  - Publish all crates to crates.io"
        echo "  restore  - Restore path dependencies for local development"
        echo ""
        echo "Example workflow:"
        echo "  1. ./scripts/prepare_for_publish.sh prepare"
        echo "  2. ./scripts/prepare_for_publish.sh publish"
        echo "  3. ./scripts/prepare_for_publish.sh restore"
        exit 1
        ;;
esac

# Clean up backup files
find . -name "*.bak" -delete

echo "🎉 Done!"
