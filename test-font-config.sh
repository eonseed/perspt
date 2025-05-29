#!/bin/bash
# Test script to verify font configuration for documentation builds

echo "🔍 Font Configuration Test"
echo "=========================="

# Check if we're in CI
if [ "$CI" = "true" ] || [ "$GITHUB_ACTIONS" = "true" ]; then
    echo "🤖 CI environment detected"
    CONF_FILE="docs/perspt_book/source/conf_ci.py"
else
    echo "🖥️  Local development environment"
    CONF_FILE="docs/perspt_book/source/conf.py"
fi

# Check required fonts
echo ""
echo "📋 Checking available fonts:"
echo "----------------------------"

if command -v fc-list >/dev/null 2>&1; then
    echo "✅ fontconfig available"
    
    echo "🔍 Noto Color Emoji:"
    fc-list | grep -i "noto.*emoji" | head -3 || echo "❌ Not found"
    
    echo "🔍 Liberation fonts:"
    fc-list | grep -i "liberation" | head -3 || echo "❌ Not found"
    
    echo "🔍 DejaVu fonts:"
    fc-list | grep -i "dejavu" | head -3 || echo "❌ Not found"
else
    echo "❌ fontconfig not available"
fi

# Check LaTeX engines
echo ""
echo "🔧 Checking LaTeX engines:"
echo "---------------------------"

if command -v lualatex >/dev/null 2>&1; then
    echo "✅ LuaLaTeX available"
    lualatex --version | head -1
else
    echo "❌ LuaLaTeX not available"
fi

if command -v xelatex >/dev/null 2>&1; then
    echo "✅ XeLaTeX available"
    xelatex --version | head -1
else
    echo "❌ XeLaTeX not available"
fi

# Check Python environment
echo ""
echo "🐍 Python environment:"
echo "----------------------"

if command -v uv >/dev/null 2>&1; then
    echo "✅ uv available"
    uv --version
else
    echo "❌ uv not available"
fi

if [ -f "$CONF_FILE" ]; then
    echo "✅ Configuration file found: $CONF_FILE"
else
    echo "❌ Configuration file not found: $CONF_FILE"
fi

echo ""
echo "🎯 Test complete!"
