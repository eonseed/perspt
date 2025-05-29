# Documentation Build Requirements

This document explains the font and LaTeX requirements for building the Perspt documentation.

## 🎯 Overview

The Perspt documentation uses Sphinx with LaTeX/PDF output that includes:
- **Unicode support** with LuaLaTeX engine (XeLaTeX for development)
- **Custom fonts** for professional appearance
- **Color emoji rendering** for enhanced visual appeal
- **Cross-platform compatibility** between development and CI environments

## 📋 Font Requirements

### Development Environment (macOS)
The local `conf.py` uses macOS-specific fonts:
- **Main font**: Times New Roman
- **Sans font**: Helvetica Neue  
- **Mono font**: Menlo
- **Emoji font**: NotoEmoji-Regular (from user Library/Fonts)
- **LaTeX Engine**: XeLaTeX

### CI Environment (Ubuntu)
The CI configuration (`conf_ci.py`) uses system fonts available on Ubuntu:
- **Main font**: Liberation Serif
- **Sans font**: Liberation Sans
- **Mono font**: Liberation Mono
- **Emoji font**: Noto Color Emoji (with HarfBuzz renderer)
- **LaTeX Engine**: LuaLaTeX (LuaHBTeX)

## 🚀 LaTeX Engine Selection

### Why LuaLaTeX for CI?
We switched from XeLaTeX to LuaLaTeX for CI builds because:
1. **Better color emoji support**: LuaHBTeX handles color emojis more reliably
2. **HarfBuzz integration**: Superior text shaping and font fallback
3. **Improved Unicode handling**: Better support for complex scripts
4. **Font fallback system**: More robust handling of missing fonts

### Font Fallback Strategy
The CI configuration implements a sophisticated fallback system:
```latex
\directlua{
    luaotfload.add_fallback("emojifallback", {
        "Noto Color Emoji:mode=harf;",
        "Noto Emoji:mode=harf;", 
        "DejaVu Sans:mode=harf;",
        "Liberation Sans:mode=harf;"
    })
}
```

## 🔧 LaTeX Packages Required

### Essential Packages
```bash
# LuaLaTeX and core LaTeX (primary for CI)
texlive-luatex
texlive-latex-recommended
texlive-latex-extra

# XeLaTeX (backup/development)
texlive-xetex

# Font support
texlive-fonts-recommended
texlive-fonts-extra

# System fonts
fonts-liberation
fonts-dejavu
fonts-noto
fonts-noto-color-emoji
fonts-noto-extra
fontconfig
```

### Additional LaTeX Packages Used
- `fontspec` - Advanced font selection
- `unicode-math` - Unicode mathematics
- `xcolor` - Color support
- `titlesec` - Title formatting
- `microtype` - Enhanced typography
- `enumitem` - List customization
- `newunicodechar` - Unicode character definitions

## 🏗️ Build Process

### Local Development
```bash
cd docs/perspt_book
uv run make html    # HTML documentation
uv run make latexpdf # PDF documentation (requires fonts)
```

### CI Environment
The CI automatically:
1. **Installs LaTeX packages** and system fonts
2. **Updates font cache** with `fc-cache -fv`
3. **Switches to CI configuration** that uses system fonts
4. **Builds documentation** with fallback error handling

## 🚨 Common Issues and Solutions

### Font Not Found Errors
**Problem**: `! LaTeX Error: Cannot find font 'Times New Roman'`
**Solution**: The CI configuration automatically handles this by using Liberation fonts

### Emoji Rendering Issues
**Problem**: Emojis appear as boxes or missing characters
**Solution**: The CI config includes fallback text alternatives:
```latex
\newunicodechar{🚀}{\safeunicode{🚀}{[rocket]}}
```

### Build Failures
**Problem**: PDF build fails completely
**Solution**: CI continues with warning; HTML docs still work

## 🔄 Automatic Configuration Switching

The `conf_ci.py` detects CI environment and automatically:
```python
if os.environ.get('CI') or os.environ.get('GITHUB_ACTIONS'):
    # Use CI-optimized LaTeX configuration
    latex_elements = { ... }  # System fonts
else:
    # Use development configuration  
    from conf import *        # Original settings
```

## 🎨 Font Customization

### Adding New Fonts
1. **For local development**: Install fonts in system/user font directory
2. **For CI**: Add to Ubuntu package installation in workflow
3. **Update both configurations**: `conf.py` and `conf_ci.py`

### Adding New Emojis
1. **Add to both LaTeX configurations**:
```latex
\newunicodechar{🆕}{\safeunicode{🆕}{[new]}}
```
2. **Include fallback text** for accessibility

## 📊 Testing Font Configuration

### Local Testing
```bash
# Test font availability
fc-list | grep "Liberation"
fc-list | grep "Noto"

# Test LaTeX build
cd docs/perspt_book
uv run sphinx-build -b latex source build/latex
cd build/latex && make
```

### CI Testing
The workflows include font cache updates and graceful error handling to ensure builds succeed even with font issues.

## 🔍 Debugging Font Issues

### Check Available Fonts
```bash
# List all available fonts
fc-list

# Check specific font family
fc-list | grep -i "liberation"
fc-list | grep -i "noto"

# Test font in LaTeX
xelatex -version
```

### LaTeX Font Debugging
```latex
% Add to preamble for debugging
\listfiles  % List all loaded files
\showfont{font-name}  % Show font details
```

## 🌐 Cross-Platform Considerations

- **Font paths**: Avoid hardcoded paths; use font names
- **Font fallbacks**: Always provide fallback fonts
- **Emoji support**: Test on different systems
- **CI compatibility**: Use commonly available system fonts

The current setup ensures documentation builds successfully across macOS development environments and Ubuntu CI runners while maintaining visual consistency.
