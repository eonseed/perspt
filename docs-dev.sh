#!/bin/bash

# 📚 Perspt Documentation Development Helper
# Streamlines common documentation development tasks

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Function to print colored output
print_header() {
    echo -e "${PURPLE}📚 Perspt Documentation Helper${NC}"
    echo -e "${PURPLE}===============================${NC}"
    echo
}

print_info() {
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

# Function to show usage
show_usage() {
    print_header
    echo "Usage: $0 [command]"
    echo
    echo "Commands:"
    echo "  build-html     Build HTML documentation"
    echo "  build-pdf      Build PDF documentation"
    echo "  build-all      Build both HTML and PDF documentation"
    echo "  clean          Clean build directories"
    echo "  watch          Start live-reload development server"
    echo "  open-html      Open HTML documentation in browser"
    echo "  open-pdf       Open PDF documentation"
    echo "  validate       Validate documentation links and structure"
    echo "  stats          Show documentation statistics"
    echo "  help           Show this help message"
    echo
}

# Function to build HTML documentation
build_html() {
    print_info "Building HTML documentation..."
    cd docs/perspt_book
    if uv run make html; then
        print_success "HTML documentation built successfully!"
        print_info "Location: docs/perspt_book/build/html/index.html"
    else
        print_error "Failed to build HTML documentation"
        return 1
    fi
}

# Function to build PDF documentation
build_pdf() {
    print_info "Building PDF documentation..."
    cd docs/perspt_book
    if uv run make latexpdf; then
        print_success "PDF documentation built successfully!"
        print_info "Location: docs/perspt_book/build/latex/perspt.pdf"
        
        # Show PDF stats
        pdf_file="build/latex/perspt.pdf"
        if [[ -f "$pdf_file" ]]; then
            pdf_size=$(du -h "$pdf_file" | cut -f1)
            pdf_pages=$(pdfinfo "$pdf_file" 2>/dev/null | grep "Pages:" | awk '{print $2}' || echo "unknown")
            print_info "PDF size: $pdf_size, Pages: $pdf_pages"
        fi
    else
        print_error "Failed to build PDF documentation"
        return 1
    fi
}

# Function to build all documentation
build_all() {
    print_info "Building all documentation..."
    clean_build
    build_html
    build_pdf
    print_success "All documentation built successfully!"
}

# Function to clean build directories
clean_build() {
    print_info "Cleaning build directories..."
    cd docs/perspt_book
    if uv run make clean; then
        print_success "Build directories cleaned!"
    else
        print_warning "Failed to clean build directories"
    fi
}

# Function to start development server
start_watch() {
    print_info "Starting live-reload development server..."
    print_info "Documentation will auto-rebuild when files change"
    print_info "Server will be available at: http://localhost:8000"
    print_info "Press Ctrl+C to stop"
    echo
    
    cd docs/perspt_book
    uv run sphinx-autobuild source build/html --host 0.0.0.0 --port 8000
}

# Function to open HTML documentation
open_html() {
    html_file="docs/perspt_book/build/html/index.html"
    if [[ -f "$html_file" ]]; then
        print_info "Opening HTML documentation..."
        open "$html_file"
        print_success "HTML documentation opened in browser"
    else
        print_error "HTML documentation not found. Run 'build-html' first."
        return 1
    fi
}

# Function to open PDF documentation
open_pdf() {
    pdf_file="docs/perspt_book/build/latex/perspt.pdf"
    if [[ -f "$pdf_file" ]]; then
        print_info "Opening PDF documentation..."
        open "$pdf_file"
        print_success "PDF documentation opened"
    else
        print_error "PDF documentation not found. Run 'build-pdf' first."
        return 1
    fi
}

# Function to validate documentation
validate_docs() {
    print_info "Validating documentation..."
    if [[ -f "./validate-docs.sh" ]]; then
        ./validate-docs.sh
    else
        print_warning "validate-docs.sh script not found"
        
        # Basic validation
        print_info "Performing basic validation..."
        
        # Check if HTML build directory exists
        if [[ -d "docs/perspt_book/build/html" ]]; then
            html_files=$(find docs/perspt_book/build/html -name "*.html" | wc -l)
            print_success "Found $html_files HTML files"
        else
            print_warning "HTML build directory not found"
        fi
        
        # Check if PDF exists
        if [[ -f "docs/perspt_book/build/latex/perspt.pdf" ]]; then
            print_success "PDF documentation exists"
        else
            print_warning "PDF documentation not found"
        fi
    fi
}

# Function to show documentation statistics
show_stats() {
    print_header
    echo "📊 Documentation Statistics"
    echo "========================="
    echo
    
    # Source files
    rst_files=$(find docs/perspt_book/source -name "*.rst" | wc -l)
    md_files=$(find docs/perspt_book/source -name "*.md" | wc -l)
    print_info "RST files: $rst_files"
    print_info "Markdown files: $md_files"
    
    # Build artifacts
    if [[ -d "docs/perspt_book/build/html" ]]; then
        html_files=$(find docs/perspt_book/build/html -name "*.html" | wc -l)
        html_size=$(du -sh docs/perspt_book/build/html 2>/dev/null | cut -f1 || echo "unknown")
        print_info "HTML files: $html_files (size: $html_size)"
    fi
    
    if [[ -f "docs/perspt_book/build/latex/perspt.pdf" ]]; then
        pdf_size=$(du -h docs/perspt_book/build/latex/perspt.pdf | cut -f1)
        pdf_pages=$(pdfinfo docs/perspt_book/build/latex/perspt.pdf 2>/dev/null | grep "Pages:" | awk '{print $2}' || echo "unknown")
        print_info "PDF: $pdf_size, $pdf_pages pages"
    fi
    
    # Rust docs
    if [[ -d "target/doc" ]]; then
        rust_doc_size=$(du -sh target/doc 2>/dev/null | cut -f1 || echo "unknown")
        print_info "Rust documentation: $rust_doc_size"
    fi
    
    echo
}

# Main script logic
case "${1:-help}" in
    "build-html")
        build_html
        ;;
    "build-pdf")
        build_pdf
        ;;
    "build-all")
        build_all
        ;;
    "clean")
        clean_build
        ;;
    "watch")
        start_watch
        ;;
    "open-html")
        open_html
        ;;
    "open-pdf")
        open_pdf
        ;;
    "validate")
        validate_docs
        ;;
    "stats")
        show_stats
        ;;
    "help"|"--help"|"-h")
        show_usage
        ;;
    *)
        print_error "Unknown command: $1"
        echo
        show_usage
        exit 1
        ;;
esac
