#!/bin/bash

# 🔍 Perspt Documentation Validation Script
# Validates that all documentation assets are properly integrated

set -e

echo "🔍 Perspt Documentation Validation"
echo "==================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

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

# Check if Rust API documentation was generated
print_status "Checking if Rust API documentation exists..."
if [ -f "target/doc/perspt/index.html" ]; then
    print_success "Main Rust API documentation found"
else
    print_error "Main Rust API documentation not found"
    validation_errors=$((validation_errors + 1))
fi

# Check custom theme CSS for Rust API Docs
print_status "Checking if custom theme CSS for Rust API docs is present..."
if [ -f "target/doc/theme.css" ]; then
    print_success "Theme CSS present"
else
    print_error "Theme CSS missing"
    validation_errors=$((validation_errors + 1))
fi

# Check HTML structure for custom styling in Rust API docs
print_status "Validating Rust API HTML integration..."
if grep -q "perspt-" target/doc/perspt/index.html; then
    print_success "Custom CSS classes found in Rust API documentation"
else
    print_error "Custom CSS classes not found in Rust API documentation"
    validation_errors=$((validation_errors + 1))
fi

# Check Sphinx Documentation Book
print_status "Checking if Perspt Sphinx Book exists..."
if [ -f "docs/perspt_book/build/html/index.html" ]; then
    print_success "Perspt Book HTML build found"
else
    print_error "Perspt Book HTML build missing (Try running uv run make html under docs/perspt_book)"
    validation_errors=$((validation_errors + 1))
fi

# Check PSP documentation
print_status "Checking if PSP documentation exists..."
if [ -f "docs/psps/build/html/index.html" ] && [ -f "docs/psps/build/html/psp-000000.html" ]; then
    print_success "PSP Documentation build found"
else
    print_error "PSP Documentation build missing (Try running python build.py under docs/psps)"
    validation_errors=$((validation_errors + 1))
fi

# Validate file sizes (ensure target docs aren't empty)
print_status "Validating generated documentation file sizes..."
for file in target/doc/*.html; do
    if [ -f "$file" ] && [ $(wc -c < "$file") -gt 1000 ]; then
        print_success "$(basename "$file") has substantial content"
    elif [ -f "$file" ]; then
        print_error "$(basename "$file") appears to be too small"
        validation_errors=$((validation_errors + 1))
    fi
done

# Summary
echo ""
echo "📊 Validation Summary"
echo "===================="

if [ $validation_errors -eq 0 ]; then
    echo -e "${GREEN}✅ All validation checks passed!${NC}"
    echo -e "${CYAN}📚 Documentation is ready with all components built.${NC}"
    echo ""
    echo "🌐 Access points:"
    echo "  • Rust API docs: target/doc/perspt/index.html"
    echo "  • Perspt Book: docs/perspt_book/build/html/index.html"
    echo "  • PSP Proposals: docs/psps/build/html/index.html"
    echo ""
    echo "🎯 Features validated:"
    echo "  • ✅ Custom dark theme"
    echo "  • ✅ Rust API documentation"
    echo "  • ✅ Perspt Book documentation"
    echo "  • ✅ PSP documentation"
    exit 0
else
    echo -e "${RED}❌ Validation failed with $validation_errors error(s)${NC}"
    echo -e "${YELLOW}💡 Run './generate-docs.sh' and build sphinx/psp docs to regenerate.${NC}"
    exit 1
fi
