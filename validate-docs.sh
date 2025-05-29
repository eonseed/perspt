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

# Check if documentation was generated
print_status "Checking if documentation exists..."
if [ -f "target/doc/perspt/index.html" ]; then
    print_success "Main Rust documentation found"
else
    print_error "Main Rust documentation not found"
    validation_errors=$((validation_errors + 1))
fi

# Check if asset library exists
print_status "Checking if asset library exists..."
if [ -f "target/doc/asset-library.html" ]; then
    print_success "Asset library found"
else
    print_error "Asset library not found"
    validation_errors=$((validation_errors + 1))
fi

# Check asset collections
print_status "Validating asset collections..."
asset_files=(
    "target/doc/banner-assets.html"
    "target/doc/icon-collection.html"
    "target/doc/background-patterns.html"
    "target/doc/interactive-demo.html"
    "target/doc/design-system.html"
    "target/doc/asset-integration.html"
    "target/doc/asset-library.html"
)

for file in "${asset_files[@]}"; do
    if [ -f "$file" ]; then
        print_success "$(basename "$file") present"
    else
        print_error "$(basename "$file") missing"
        validation_errors=$((validation_errors + 1))
    fi
done

# Check custom assets
print_status "Validating custom assets..."
if [ -f "target/doc/script.js" ]; then
    print_success "JavaScript enhancements present"
else
    print_error "JavaScript enhancements missing"
    validation_errors=$((validation_errors + 1))
fi

if [ -f "target/doc/theme.css" ]; then
    print_success "Theme CSS present"
else
    print_error "Theme CSS missing"
    validation_errors=$((validation_errors + 1))
fi

# Check README
print_status "Validating documentation README..."
if [ -f "target/doc/README.md" ]; then
    print_success "Documentation README present"
    
    # Check if README contains expected content
    if grep -q "Asset Collections Available" target/doc/README.md; then
        print_success "README contains asset collection information"
    else
        print_error "README missing asset collection information"
        validation_errors=$((validation_errors + 1))
    fi
else
    print_error "Documentation README missing"
    validation_errors=$((validation_errors + 1))
fi

# Check HTML structure for custom styling
print_status "Validating HTML integration..."
if grep -q "perspt-" target/doc/perspt/index.html; then
    print_success "Custom CSS classes found in documentation"
else
    print_error "Custom CSS classes not found in documentation"
    validation_errors=$((validation_errors + 1))
fi

# Validate file sizes (ensure assets aren't empty)
print_status "Validating asset file sizes..."
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
    echo -e "${CYAN}📚 Documentation is ready with all custom assets.${NC}"
    echo ""
    echo "🌐 Access points:"
    echo "  • Main docs: target/doc/perspt/index.html"
    echo "  • Asset library: target/doc/asset-library.html"
    echo "  • Integration guide: target/doc/asset-integration.html"
    echo ""
    echo "🎯 Features validated:"
    echo "  • ✅ Custom dark theme"
    echo "  • ✅ Asset collections (banners, icons, patterns)"
    echo "  • ✅ Interactive features"
    echo "  • ✅ Design system integration"
    echo "  • ✅ Documentation README"
    exit 0
else
    echo -e "${RED}❌ Validation failed with $validation_errors error(s)${NC}"
    echo -e "${YELLOW}💡 Try running './generate-docs.sh' to regenerate documentation${NC}"
    exit 1
fi
