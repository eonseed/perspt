#!/bin/bash

# 🔍 Perspt Documentation Validation Script
# Validates that Rustdoc, Guidedog HTML/SSG mounts, and Typst PDFs are built and healthy

set -e

echo "🔍 Perspt Documentation Validation"
echo "==================================="

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

print_status() {
    echo -e "${CYAN}[CHECK]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

print_error() {
    echo -e "${RED}[FAIL]${NC} $1"
}

validation_errors=0

# 1. Check PSP Governance & Index Integrity via Guidedog
print_status "Checking PSP catalog governance via guidedog dsc check..."
if guidedog dsc check --dir=docs/psps; then
    print_success "PSP catalog metadata and index agree"
else
    print_error "PSP catalog check failed"
    validation_errors=$((validation_errors + 1))
fi

# 2. Check Unified Guidedog Site
print_status "Checking unified Guidedog site build..."
if [ -f "docs/_build/site/index.html" ] && [ -f "docs/_build/site/book/index.html" ] && [ -f "docs/_build/site/psps/index.html" ]; then
    print_success "Unified site (home, book, psps) found in docs/_build/site/"
else
    print_error "Unified site missing in docs/_build/site/ (Run: cargo run -p xtask -- docs)"
    validation_errors=$((validation_errors + 1))
fi

# 3. Check Typst Book PDF and PSP PDFs
print_status "Checking Typst PDF builds..."
if [ -f "docs/_build/site/books/perspt-book-en.pdf" ] && [ -f "docs/perspt.pdf" ]; then
    print_success "Perspt Book Typst PDF found"
else
    print_error "Perspt Book Typst PDF missing"
    validation_errors=$((validation_errors + 1))
fi

psp_pdf_count=$(find docs/psps/_build/pdf -maxdepth 1 -name "psp-*.pdf" 2>/dev/null | wc -l)
if [ "$psp_pdf_count" -ge 11 ]; then
    print_success "Found $psp_pdf_count PSP Typst PDFs in docs/psps/_build/pdf/"
else
    print_error "Expected at least 11 PSP PDFs in docs/psps/_build/pdf/, found $psp_pdf_count"
    validation_errors=$((validation_errors + 1))
fi

# 4. Check Rust API Documentation
print_status "Checking Rust API documentation..."
if [ -f "target/doc/perspt/index.html" ]; then
    print_success "Main Rust API documentation found"
else
    print_error "Main Rust API documentation not found (Run: ./generate-docs.sh)"
    validation_errors=$((validation_errors + 1))
fi

echo ""
echo "📊 Validation Summary"
echo "===================="

if [ $validation_errors -eq 0 ]; then
    echo -e "${GREEN}✅ All validation checks passed!${NC}"
    echo ""
    echo "🌐 Access points:"
    echo "  • Unified Site:  docs/_build/site/index.html"
    echo "  • Perspt Book:   docs/_build/site/book/index.html"
    echo "  • Book PDF:      docs/perspt.pdf"
    echo "  • PSP Catalog:   docs/_build/site/psps/index.html"
    echo "  • Rust API docs: target/doc/perspt/index.html"
    exit 0
else
    echo -e "${RED}❌ Validation failed with $validation_errors error(s)${NC}"
    echo -e "${YELLOW}💡 Run './generate-docs.sh' to build all documentation artifacts.${NC}"
    exit 1
fi
