#!/bin/bash

# 👁️ Perspt Documentation Generation Script
# Generates beautiful documentation with custom assets and styling

echo "👁️ Perspt Documentation Generator"
echo "=================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${CYAN}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    print_error "Not in a Rust project directory. Please run from the project root."
    exit 1
fi

# Check if docs directory exists
if [ ! -d "docs" ]; then
    print_warning "docs/ directory not found. Creating it..."
    mkdir -p docs
fi

# Clean previous documentation
print_status "Cleaning previous documentation..."
cargo clean --doc

# Generate documentation with custom assets
print_status "Generating documentation with custom assets..."

# Set RUSTDOCFLAGS for custom styling
export RUSTDOCFLAGS="--html-in-header docs/header.html --extend-css docs/custom.css --default-theme dark"

# Generate the documentation
cargo doc --no-deps --all-features --open

if [ $? -eq 0 ]; then
    print_success "Documentation generated successfully!"
    
    # Copy additional assets to doc directory
    print_status "Copying additional asset collections..."
    
    # Copy all HTML asset collections
    for asset_file in docs/*.html; do
        if [ -f "$asset_file" ]; then
            cp "$asset_file" target/doc/
            print_success "$(basename "$asset_file") copied"
        fi
    done
    
    # Copy any additional CSS files
    for css_file in docs/*.css; do
        if [ -f "$css_file" ] && [ "$(basename "$css_file")" != "custom.css" ]; then
            cp "$css_file" target/doc/
            print_success "$(basename "$css_file") copied"
        fi
    done
    
    # Copy JavaScript files
    if [ -f "docs/script.js" ]; then
        cp docs/script.js target/doc/
        print_success "JavaScript enhancements copied"
    fi
    
    # Create a README in the doc directory
    cat > target/doc/README.md << EOF
# 👁️ Perspt Documentation

> **Your Terminal's Window to the AI World**

This is the auto-generated documentation for Perspt, enhanced with a comprehensive collection of custom assets and interactive features.

## 🎨 Asset Collections Available

### 🌟 Core Integration
- **Enhanced Documentation** - Custom styled rustdoc with terminal aesthetics
- **Interactive Features** - Copy buttons, keyboard shortcuts, search enhancements

### � Asset Libraries
- **[Banner Assets](banner-assets.html)** - Hero banners and promotional graphics
- **[Icon Collection](icon-collection.html)** - 50+ custom SVG icons
- **[Background Patterns](background-patterns.html)** - Subtle SVG patterns and textures
- **[Interactive Demo](interactive-demo.html)** - Animated terminal demonstrations
- **[Design System](design-system.html)** - Complete design system guide
- **[Logo Assets](logo-assets.html)** - Logo collection and branding
- **[Asset Integration Guide](asset-integration.html)** - Step-by-step integration guide

## ✨ Interactive Features

- 🎨 **Custom Dark Theme** - AI/terminal-inspired color scheme optimized for readability
- 📋 **Copy-to-Clipboard** - One-click copying for all code blocks and examples
- ⌨️ **Keyboard Shortcuts** - Press 'h' for help, 's' or '/' for search
- 🔍 **Enhanced Search** - Live filtering with highlighting and suggestions
- 📱 **Responsive Design** - Mobile-first responsive layouts
- ✨ **Smooth Animations** - CSS transitions and fade-in effects
- 🤖 **Terminal Effects** - Typing animations for code examples

## 🚀 Navigation Tips

- **Search**: Press 's' or '/' to focus the search bar
- **Help**: Press 'h' to show all keyboard shortcuts
- **Navigation**: Use arrow keys to navigate through search results
- **Copy Code**: Hover over code blocks to reveal copy buttons
- **Asset Library**: Click the asset links above to explore collections

## 🛠️ Technical Details

- **Generated**: $(date)
- **Rust Version**: $(rustc --version)
- **Features**: All features enabled (--all-features)
- **Theme**: Custom dark theme with terminal aesthetics
- **Assets**: Complete asset library with 100+ custom elements

## 📚 Documentation Structure

- **API Reference** - Complete function and module documentation
- **Examples** - Working code samples with copy functionality
- **Assets** - Professional SVG icons, banners, and patterns
- **Integration** - Step-by-step guides for using assets

---

*Generated with ❤️ by the Perspt documentation system*
EOF

    print_success "Documentation README created"
    
    echo ""
    echo -e "${PURPLE}📚 Documentation Features:${NC}"
    echo "  🎨 Custom dark theme with AI/terminal aesthetics"
    echo "  📋 Copy-to-clipboard for all code blocks"
    echo "  ⌨️ Keyboard shortcuts (press 'h' for help)"
    echo "  🔍 Enhanced search with live filtering"
    echo "  📱 Responsive design for mobile devices"
    echo "  ✨ Smooth animations and hover effects"
    echo ""
    
    # Show the documentation URL
    DOC_PATH="target/doc/perspt/index.html"
    if [ -f "$DOC_PATH" ]; then
        print_success "Documentation available at: file://$(pwd)/$DOC_PATH"
    fi
    
    # Show file sizes
    print_status "Documentation size:"
    du -sh target/doc/ | sed 's/target\/doc\//Total size: /'
    
else
    print_error "Documentation generation failed!"
    exit 1
fi

echo ""
echo -e "${GREEN}🎉 Documentation generation complete!${NC}"
echo -e "${CYAN}👁️ Perspt: Personal Spectrum Pertaining Thoughts${NC}"
