# Makefile for PSP Sphinx documentation

SPHINXBUILD   = uv run python
BUILDDIR      = build

.PHONY: help clean html dirhtml htmlview latex pdf

help:
	@$(SPHINXBUILD) build.py --help

clean:
	rm -rf $(BUILDDIR)

html:
	@$(SPHINXBUILD) build.py

dirhtml:
	@$(SPHINXBUILD) build.py -b dirhtml

latex:
	@$(SPHINXBUILD) build.py -b latex

pdf: latex
	@$(MAKE) -C $(BUILDDIR)/latex PDFLATEX="latexmk -lualatex"

htmlview: html
	python -c "import os, webbrowser; webbrowser.open('file://' + os.path.realpath('build/html/index.html'))"
