# Contributing to Rafka

Thank you for your interest in contributing to Rafka! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How Can I Contribute?](#how-can-i-contribute)
- [Development Setup](#development-setup)
- [Coding Standards](#coding-standards)
- [Commit Message Guidelines](#commit-message-guidelines)
- [Pull Request Process](#pull-request-process)
- [Reporting Bugs](#reporting-bugs)
- [Suggesting Features](#suggesting-features)
- [Testing Guidelines](#testing-guidelines)

## Code of Conduct

This project adheres to a Code of Conduct that all contributors are expected to follow. Please read [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) before contributing.

## How Can I Contribute?

### Reporting Bugs

Before creating a bug report, please check [existing issues](https://github.com/Mahir101/Rafka/issues) to see if the problem has already been reported.

#### How to Report a Bug

1. **Use the Bug Report Template**: When opening a new issue, select the "Bug Report" template.

2. **Provide Essential Information**:
   - **Description**: Clear description of the bug
   - **Steps to Reproduce**: Detailed steps to reproduce the issue
   - **Expected Behavior**: What you expected to happen
   - **Actual Behavior**: What actually happened
   - **Environment**: 
     - OS and version
     - Rust version
     - Rafka version
   - **Configuration**: Relevant configuration files
   - **Logs**: Error logs or stack traces

3. **Example Bug Report**:
   ```
   **Description**: Broker crashes when handling large messages (>1MB)
   
   **Steps to Reproduce**:
   1. Start broker: cargo run --bin start_broker
   2. Send message >1MB via producer
   3. Broker crashes with panic
   
   **Expected**: Should handle large messages gracefully
   **Actual**: Panic occurs in storage layer
   
   **Environment**: macOS 14.2, Rust 1.75, Rafka v0.1.0
   ```

### Suggesting Features

We welcome feature suggestions! To propose a new feature:

1. **Check Existing Issues**: Search for similar feature requests
2. **Use the Feature Request Template**: Select "Feature Request" when opening a new issue
3. **Provide Context**:
   - **Description**: What problem does this solve?
   - **Proposed Solution**: How should it work?
   - **Alternatives**: Other solutions you've considered
   - **Use Cases**: Real-world scenarios

#### Example Feature Request:
```
**Feature**: Add TLS support for secure broker-client communication

**Problem**: Currently all communication is unencrypted, which is a security concern for production deployments.

**Proposed Solution**: Add TLS configuration options to broker and clients using rustls crate.

**Use Cases**:
- Secure message passing in untrusted networks
- Compliance with security regulations
- Production-ready deployment
```

### Contributing Code

#### Development Setup

1. **Fork the Repository**:
   ```bash
   git clone https://github.com/Mahir101/Rafka.git
   cd Rafka
   ```

2. **Install Prerequisites**:
   - Rust: Latest stable version (1.70+)
   - Protocol Buffers compiler: `protoc`
   - Git

3. **Build the Project**:
   ```bash
   cargo build
   ```

4. **Run Tests**:
   ```bash
   cargo test
   ```

5. **Create a Branch**:
   ```bash
   git checkout -b feature/your-feature-name
   # or
   git checkout -b fix/your-bug-fix
   ```

#### Project Structure

```
Rafka/
├── crates/           # Core library crates
│   ├── core/         # Core types and gRPC definitions
│   ├── broker/       # Broker implementation
│   ├── producer/     # Producer client
│   ├── consumer/     # Consumer client
│   └── storage/      # Storage engine
├── src/bin/          # Executable binaries
├── scripts/          # Demo scripts
├── config/           # Configuration files
└── docs/             # Documentation
```

#### Coding Standards

1. **Rust Style Guidelines**:
   - Follow the official [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/)
   - Run `cargo fmt` before committing
   - Run `cargo clippy` to catch common mistakes

2. **Code Formatting**:
   ```bash
   cargo fmt --all
   cargo clippy --all -- -D warnings
   ```

3. **Documentation**:
   - Add doc comments to public API functions
   - Include usage examples in doc comments
   - Update README if adding new features

4. **Examples**:
   ```rust
   /// Publishes a message to the specified topic.
   ///
   /// # Arguments
   /// * `topic` - Topic name
   /// * `payload` - Message content
   /// * `key` - Partition key
   ///
   /// # Returns
   /// Returns a `Result` containing the message ID and offset on success.
   ///
   /// # Example
   /// ```
   /// producer.publish("my-topic".to_string(), "Hello".to_string(), "key-1".to_string()).await?;
   /// ```
   pub async fn publish(&mut self, topic: String, payload: String, key: String) -> Result<PublishResponse> {
       // Implementation
   }
   ```

5. **Error Handling**:
   - Use `Result<T, E>` for operations that can fail
   - Use `thiserror` for custom error types
   - Provide meaningful error messages

6. **Testing**:
   - Write unit tests for new functionality
   - Integration tests for major features
   - Ensure all tests pass before submitting PR

#### Commit Message Guidelines

Follow [Conventional Commits](https://www.conventionalcommits.org/) specification:

**Format**:
```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types**:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding/updating tests
- `chore`: Maintenance tasks

**Examples**:
```
feat(broker): add TLS support for encrypted communication

fix(storage): prevent memory leak in message retention

docs(readme): update installation instructions

refactor(p2p): improve node discovery algorithm
```

#### Pull Request Process

1. **Before Submitting**:
   - [ ] Code compiles without warnings
   - [ ] All tests pass (`cargo test`)
   - [ ] Code is formatted (`cargo fmt`)
   - [ ] No clippy warnings (`cargo clippy`)
   - [ ] Documentation is updated
   - [ ] Commit messages follow guidelines

2. **Create Pull Request**:
   - Fork the repository
   - Create a feature branch
   - Make your changes
   - Commit your changes with good commit messages
   - Push to your fork
   - Open a Pull Request on GitHub

3. **PR Template**:
   Use this template for your PR description:

   ```markdown
   ## Description
   Brief description of changes

   ## Type of Change
   - [ ] Bug fix
   - [ ] New feature
   - [ ] Breaking change
   - [ ] Documentation update

   ## Testing
   Describe the tests you ran and relevant details

   ## Checklist
   - [ ] Code compiles without warnings
   - [ ] All tests pass
   - [ ] Documentation updated
   - [ ] Commit messages follow guidelines
   - [ ] No breaking changes (or documented)
   ```

4. **Review Process**:
   - PR will be reviewed by maintainers
   - Address any review comments
   - Keep PR focused and reasonably sized
   - Update PR description if needed

### Testing Guidelines

1. **Unit Tests**:
   - Test individual functions and methods
   - Use descriptive test names
   - Place tests in `tests/` module in same file or separate `tests/` directory

2. **Integration Tests**:
   - Test complete workflows
   - Test multiple components interacting
   - Place in `tests/integration_tests.rs`

3. **Example Test**:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[tokio::test]
       async fn test_publish_message() {
           let mut producer = Producer::new("127.0.0.1:50051").await.unwrap();
           let result = producer.publish(
               "test-topic".to_string(),
               "test-payload".to_string(),
               "test-key".to_string()
           ).await;
           
           assert!(result.is_ok());
       }
   }
   ```

4. **Running Tests**:
   ```bash
   # Run all tests
   cargo test

   # Run specific test
   cargo test test_publish_message

   # Run with output
   cargo test -- --nocapture
   ```

## Development Workflow

### Branch Strategy

- `main`: Stable, production-ready code
- `develop`: Integration branch for features (future)
- `feature/*`: New features
- `fix/*`: Bug fixes
- `docs/*`: Documentation changes

### Making Changes

1. Sync with upstream:
   ```bash
   git checkout main
   git pull upstream main
   ```

2. Create feature branch:
   ```bash
   git checkout -b feature/your-feature-name
   ```

3. Make changes and test:
   ```bash
   cargo build
   cargo test
   cargo fmt
   cargo clippy
   ```

4. Commit changes:
   ```bash
   git add .
   git commit -m "feat(your-scope): description of changes"
   ```

5. Push to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```

6. Open Pull Request on GitHub

## Areas for Contribution

### High Priority

- **P2P Mesh Implementation**: Distributed node discovery and communication
- **Consensus Algorithms**: Leader election and cluster coordination
- **Replication**: Cross-broker message replication
- **Fault Tolerance**: Node failure detection and recovery
- **Security**: TLS support and authentication

### Medium Priority

- **Performance Optimization**: Message batching and compression
- **Monitoring**: Enhanced Prometheus metrics and Grafana dashboards
- **Documentation**: API documentation and tutorials
- **Client SDKs**: Support for multiple languages (Python, Node.js)
- **Load Balancing**: Better message distribution

### Good First Issues

Look for issues tagged with "good first issue" - these are well-suited for new contributors:
- Documentation improvements
- Adding example code
- Fixing typos
- Adding unit tests

## Getting Help

- **Questions**: Open a discussion on GitHub
- **Bugs**: Open an issue with the bug template
- **Feature Requests**: Open an issue with the feature request template
- **General Chat**: You can reach out to maintainers via GitHub issues

## Recognition

Contributors will be recognized in:
- The project README
- Release notes
- GitHub contributor list

Thank you for contributing to Rafka! 🚀
