#!/usr/bin/env python3
"""Build script for PSP Sphinx documentation"""

import argparse
import os
from pathlib import Path

from sphinx.application import Sphinx


def create_parser():
    """Create argument parser."""
    parser = argparse.ArgumentParser(description="Build PSP documentation")
    parser.add_argument(
        "-b", "--builder",
        default="html",
        help="Builder to use (default: html)"
    )
    parser.add_argument(
        "-d", "--output-dir",
        default="build",
        help="Output directory (default: build)"
    )
    parser.add_argument(
        "-j", "--jobs",
        type=int,
        default=os.cpu_count() or 1,
        help="Number of parallel jobs"
    )
    parser.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Verbose output"
    )
    return parser.parse_args()


def create_index_file(html_root: Path, builder: str) -> None:
    """Copies PSP 0 to the root index.html so that /psps/ works."""
    if builder == "dirhtml":
        psp_zero_file = html_root / "psp-0000" / "index.html"
        index_file = html_root / "index.html"
    else:
        psp_zero_file = html_root / "psp-0000.html"
        index_file = html_root / "index.html"

    if psp_zero_file.exists():
        index_file.write_text(psp_zero_file.read_text(encoding="utf-8"), encoding="utf-8")


if __name__ == "__main__":
    args = create_parser()

    root_directory = Path(__file__).resolve().parent
    source_directory = root_directory / "source"
    build_directory = root_directory / args.output_dir

    # builder configuration
    sphinx_builder = args.builder

    app = Sphinx(
        source_directory,
        confdir=source_directory,
        outdir=build_directory / sphinx_builder,
        doctreedir=build_directory / "doctrees",
        buildername=sphinx_builder,
        warningiserror=False,  # Temporarily disable warnings as errors
        parallel=1,  # Disable parallel processing to get better error messages
        tags=["internal_builder"],
        keep_going=True,
        verbosity=1 if args.verbose else 0,
    )
    app.build()

    if sphinx_builder in ["html", "dirhtml"]:
        create_index_file(build_directory / sphinx_builder, sphinx_builder)
