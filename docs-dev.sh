#!/bin/bash

# 📚 Perspt Documentation Development Helper
# Streamlines Guidedog & Typst documentation tasks via cargo xtask

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
NC='\033[0m'

print_header() {
    echo -e "${PURPLE}📚 Perspt Documentation Helper (Guidedog + Typst)${NC}"
    echo -e "${PURPLE}=================================================${NC}"
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

show_usage() {
    print_header
    echo "Usage: $0 [command]"
    echo
    echo "Commands:"
    echo "  build-assets   Compile Typst SVG diagrams and cover artwork"
    echo "  build-html     Build HTML documentation (Book, PSPs, and unified site)"
    echo "  build-pdf      Build Typst PDF documentation (Book + PSPs)"
    echo "  build-all      Build assets, HTML, PDFs, and unified site"
    echo "  clean          Clean documentation _build directories"
    echo "  watch          Start live-reload preview server on http://127.0.0.1:8000"
    echo "  open-html      Open unified site in browser"
    echo "  open-pdf       Open Perspt Book PDF"
    echo "  validate       Validate all documentation outputs"
    echo "  stats          Show documentation statistics"
    echo "  help           Show this help message"
    echo
}

build_assets() {
    print_info "Compiling Typst diagrams and cover artwork..."
    cargo run -p xtask -- assets
    print_success "Typst assets compiled into docs/shared/_static/"
}

build_html() {
    print_info "Building HTML documentation..."
    cargo run -p xtask -- docs --no-pdf
    print_success "HTML documentation built at docs/_build/site/index.html"
}

build_pdf() {
    print_info "Building Typst PDF documentation (Book + PSPs)..."
    cargo run -p xtask -- docs
    print_success "PDF documentation built at docs/perspt.pdf and docs/_build/site/books/perspt-book-en.pdf"
}

build_all() {
    print_info "Building all documentation..."
    cargo run -p xtask -- docs
    print_success "All documentation built successfully!"
}

clean_build() {
    print_info "Cleaning documentation _build directories..."
    rm -rf docs/_build docs/perspt_book/_build docs/psps/_build docs/site/_build docs/perspt.pdf
    print_success "Build directories cleaned!"
}

start_watch() {
    print_info "Starting live-reload preview server at http://127.0.0.1:8000 ..."
    cargo run -p xtask -- serve
}

open_html() {
    html_file="docs/_build/site/index.html"
    if [[ -f "$html_file" ]]; then
        xdg-open "$html_file" 2>/dev/null || open "$html_file"
    else
        print_error "Site not found at $html_file. Run '$0 build-all' first."
        return 1
    fi
}

open_pdf() {
    pdf_file="docs/perspt.pdf"
    if [[ -f "$pdf_file" ]]; then
        xdg-open "$pdf_file" 2>/dev/null || open "$pdf_file"
    else
        print_error "PDF not found at $pdf_file. Run '$0 build-pdf' first."
        return 1
    fi
}

validate_docs() {
    ./validate-docs.sh
}

show_stats() {
    print_header
    rst_files=$(find docs/perspt_book docs/psps docs/site -name "*.rst" | wc -l)
    typ_files=$(find docs/diagrams docs/perspt_book/_templates docs/psps/_templates -name "*.typ" | wc -l)
    print_info "reStructuredText source files: $rst_files"
    print_info "Typst diagram/template files: $typ_files"
    if [[ -d "docs/_build/site" ]]; then
        html_files=$(find docs/_build/site -name "*.html" | wc -l)
        site_size=$(du -sh docs/_build/site 2>/dev/null | cut -f1 || echo "unknown")
        print_info "Unified site HTML files: $html_files (size: $site_size)"
    fi
    if [[ -f "docs/perspt.pdf" ]]; then
        pdf_size=$(du -h docs/perspt.pdf | cut -f1)
        print_info "Perspt Book PDF: $pdf_size"
    fi
}

case "${1:-help}" in
    "build-assets") build_assets ;;
    "build-html")   build_html ;;
    "build-pdf")    build_pdf ;;
    "build-all")    build_all ;;
    "clean")        clean_build ;;
    "watch")        start_watch ;;
    "open-html")    open_html ;;
    "open-pdf")     open_pdf ;;
    "validate")     validate_docs ;;
    "stats")        show_stats ;;
    "help"|"--help"|"-h") show_usage ;;
    *)
        print_error "Unknown command: $1"
        show_usage
        exit 1
        ;;
esac
