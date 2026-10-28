#!/bin/bash

# 👁️ Perspt Documentation Generation Script
# Generates the Guidedog + Typst unified site, Perspt Book, PSP Catalog, and Rust API documentation

set -e

echo "👁️ Generating Perspt Documentation"
echo "=================================="

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Not in a Rust project directory. Please run from the project root.${NC}"
    exit 1
fi

echo "📐 Building Typst diagrams, PSP catalog, Perspt Book, and unified Guidedog site..."
cargo run -p xtask -- docs

if [ -f "docs/header.html" ] && [ -f "docs/custom.css" ]; then
    echo "🎨 Generating Rust API documentation with custom styling..."
    export RUSTDOCFLAGS="--html-in-header $(pwd)/docs/header.html --extend-css $(pwd)/docs/custom.css --default-theme dark"
else
    echo -e "${YELLOW}Generating Rust API documentation...${NC}"
fi

cargo doc --workspace --no-deps

if [ -d "target/doc" ] && [ -d "docs/_build/site" ]; then
    cp -rn target/doc/* docs/_build/site/ 2>/dev/null || true
fi

echo -e "${GREEN}✅ Documentation generated successfully!${NC}"
echo -e "${GREEN}📚 Unified Site:  file://$(pwd)/docs/_build/site/index.html${NC}"
echo -e "${GREEN}📖 Perspt Book:   file://$(pwd)/docs/_build/site/book/index.html${NC}"
echo -e "${GREEN}📕 Book PDF:      file://$(pwd)/docs/perspt.pdf${NC}"
echo -e "${GREEN}📜 PSP Catalog:   file://$(pwd)/docs/_build/site/psps/index.html${NC}"
echo -e "${GREEN}🦀 Rust API Docs: file://$(pwd)/target/doc/perspt/index.html${NC}"
