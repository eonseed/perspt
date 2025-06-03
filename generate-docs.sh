#!/bin/bash

# 👁️ Perspt Documentation Generation Script
# Generates beautiful Rust API documentation with custom styling

echo "👁️ Generating Perspt Documentation"
echo "=================================="

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Not in a Rust project directory. Please run from the project root.${NC}"
    exit 1
fi

# Check if required files exist
if [ ! -f "docs/header.html" ] || [ ! -f "docs/custom.css" ]; then
    echo -e "${YELLOW}Warning: Custom styling files not found. Generating basic docs...${NC}"
    cargo doc --no-deps --all-features
    exit 0
fi

echo "🎨 Generating documentation with custom styling..."

# Set RUSTDOCFLAGS for custom styling
export RUSTDOCFLAGS="--html-in-header docs/header.html --extend-css docs/custom.css --default-theme dark"

# Generate the documentation
cargo doc --no-deps --all-features

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Documentation generated successfully!${NC}"
    
    # Show the documentation URL
    DOC_PATH="target/doc/perspt/index.html"
    if [ -f "$DOC_PATH" ]; then
        echo -e "${GREEN}📚 Documentation available at: file://$(pwd)/$DOC_PATH${NC}"
    fi
    
    echo -e "${GREEN}🎉 Beautiful Rust API docs ready!${NC}"
else
    echo -e "${RED}❌ Documentation generation failed!${NC}"
    exit 1
fi
