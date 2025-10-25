# 📦 Publishing Rafka-rs to crates.io

This project is set up for both local development and publishing to crates.io. Here's how to manage both scenarios:

## 🔧 Local Development (Current Setup)

The project is currently configured for local development with path dependencies:

```toml
[dependencies]
rafka-core = { path = "crates/core" }
rafka-broker = { path = "crates/broker" }
# ... etc
```

This allows you to:
- ✅ Build and test locally
- ✅ Make changes across crates
- ✅ Use `cargo run` and `cargo test`

## 🚀 Publishing to crates.io

To publish to crates.io, you need to:

1. **Prepare for publishing** (replace path deps with version deps):
   ```bash
   ./scripts/prepare_for_publish.sh prepare
   ```

2. **Publish all crates** (in dependency order):
   ```bash
   ./scripts/prepare_for_publish.sh publish
   ```

3. **Restore local development** (restore path deps):
   ```bash
   ./scripts/prepare_for_publish.sh restore
   ```

## 📋 Publishing Process Explained

### Why This Approach?

- **Local Development**: Path dependencies allow instant updates across crates
- **Publishing**: Version dependencies ensure crates.io can resolve dependencies
- **Automation**: Script handles the conversion automatically

### Publishing Order

The script publishes crates in dependency order:

1. **rafka-core** (no dependencies)
2. **rafka-storage** (depends on rafka-core)
3. **rafka-producer** (depends on rafka-core)
4. **rafka-consumer** (depends on rafka-core)
5. **rafka-broker** (depends on rafka-core, rafka-storage)
6. **Main workspace** (depends on all)

### Version Management

All crates use version **1.0.0**:
- `rafka-core = "1.0.0"`
- `rafka-broker = "1.0.0"`
- `rafka-producer = "1.0.0"`
- `rafka-consumer = "1.0.0"`
- `rafka-storage = "1.0.0"`

## 🛠️ Manual Publishing (Alternative)

If you prefer manual control:

### Step 1: Update Dependencies
Replace all path dependencies with version dependencies:

```toml
# In each Cargo.toml
rafka-core = "1.0.0"  # Instead of { path = "../core" }
```

### Step 2: Publish in Order
```bash
cd crates/core && cargo publish
cd ../storage && cargo publish
cd ../producer && cargo publish
cd ../consumer && cargo publish
cd ../broker && cargo publish
cd ../.. && cargo publish
```

### Step 3: Restore for Development
```toml
# Restore path dependencies
rafka-core = { path = "../core" }
```

## ⚠️ Important Notes

- **First Time**: You need to publish each crate individually first
- **Authentication**: Make sure you're logged in with `cargo login`
- **Testing**: Always test locally before publishing
- **Version Bumps**: Update versions for new releases

## 🔍 Troubleshooting

### "No matching package found"
- **Cause**: Trying to use version deps before publishing
- **Solution**: Publish dependencies first, or use path deps for local dev

### "All dependencies must have version requirements"
- **Cause**: Path dependencies in published crate
- **Solution**: Use the prepare script to convert to version deps

### "Failed to publish"
- **Cause**: Dependency not yet published
- **Solution**: Publish dependencies in correct order

## 📚 Additional Resources

- [Cargo Publishing Guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [Crates.io Publishing](https://crates.io/cargo-commands.html#cargo-publish)
- [Workspace Dependencies](https://doc.rust-lang.org/cargo/reference/workspaces.html#workspace-dependencies)
