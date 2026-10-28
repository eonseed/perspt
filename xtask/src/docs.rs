//! Documentation and Typst asset compilation pipeline (`cargo xtask assets` / `cargo xtask docs`).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

const DIAGRAMS: &[(&str, &str)] = &[
    (
        "docs/diagrams/front-page-abm.typ",
        "docs/shared/_static/front-page-abm.svg",
    ),
    (
        "docs/diagrams/reliability-scaling.typ",
        "docs/shared/_static/reliability-scaling.svg",
    ),
    (
        "docs/diagrams/psp-000010-search-trajectories.typ",
        "docs/shared/_static/psp-000010-search-trajectories.svg",
    ),
    (
        "docs/diagrams/srbn-control-sequence.typ",
        "docs/shared/_static/srbn-control-sequence.svg",
    ),
    (
        "docs/diagrams/mcp-admission-sequence.typ",
        "docs/shared/_static/mcp-admission-sequence.svg",
    ),
    (
        "docs/diagrams/workspace-architecture.typ",
        "docs/shared/_static/workspace-architecture.svg",
    ),
    (
        "docs/diagrams/agent-workflow-overview.typ",
        "docs/shared/_static/agent-workflow-overview.svg",
    ),
];

/// Compile all Typst diagram sources and cover artwork into `docs/shared/_static/`.
pub(crate) fn build_assets(root: &Path) -> Result<()> {
    let typst = crate::toolchain::require(
        "typst",
        "The Typst CLI renders SVG diagrams and book cover previews.",
        "Install Typst from https://typst.app/ and ensure `typst --version` succeeds.",
    )?;
    std::fs::create_dir_all(root.join("docs/shared/_static"))
        .context("creating docs/shared/_static")?;

    for (src, dst) in DIAGRAMS {
        run_cmd(
            Command::new(&typst)
                .current_dir(root)
                .args(["compile", "--root", ".", src, dst]),
        )?;
    }

    run_cmd(Command::new(&typst).current_dir(root).args([
        "compile",
        "--root",
        ".",
        "docs/perspt_book/_templates/render-cover.typ",
        "docs/shared/_static/cover-art.svg",
    ]))?;

    let version = format!("version={}", env!("CARGO_PKG_VERSION"));
    run_cmd(
        Command::new(&typst)
            .current_dir(root)
            .args([
                "compile", "--root", ".", "--pages", "1", "--input", &version,
            ])
            .args([
                "docs/perspt_book/_templates/cover-preview.typ",
                "docs/shared/_static/book-cover.png",
            ]),
    )?;

    println!("Typst SVG diagrams and cover assets compiled into docs/shared/_static/.");
    Ok(())
}

/// Build the PSP catalog, Perspt Book, and unified Guidedog static site.
pub(crate) fn build_docs(root: &Path, no_pdf: bool, guidedog_bin: &str) -> Result<()> {
    let search = std::env::var_os("PATH").unwrap_or_default();
    if crate::toolchain::resolve("typst", &search).is_some() {
        build_assets(root)?;
    } else if root.join("docs/shared/_static/cover-art.svg").is_file() {
        println!("Using prepared SVG and cover assets in docs/shared/_static/.");
    } else {
        build_assets(root)?;
    }
    let guidedog = resolve_guidedog(guidedog_bin)?;

    run_dsc(root, &guidedog, no_pdf)?;
    run_strict_html(root, &guidedog)?;
    run_ssg_site(root, &guidedog, no_pdf)?;

    if !no_pdf {
        stage_pdfs(root)?;
    }
    println!("Unified site ready at docs/_build/site/index.html");
    Ok(())
}

pub(crate) fn resolve_guidedog(guidedog_bin: &str) -> Result<PathBuf> {
    crate::toolchain::require(
        guidedog_bin,
        "Guidedog builds the PSP catalog, the Perspt Book, and the unified static site.",
        "Install Guidedog (`guidedog --version`) and ensure it is on PATH.",
    )
}

fn run_dsc(root: &Path, guidedog: &Path, no_pdf: bool) -> Result<()> {
    run_cmd(
        Command::new(guidedog)
            .current_dir(root)
            .args(["dsc", "index", "--dir=docs/psps"]),
    )?;
    run_cmd(
        Command::new(guidedog)
            .current_dir(root)
            .args(["dsc", "check", "--dir=docs/psps"]),
    )?;
    if !no_pdf {
        run_cmd(Command::new(guidedog).current_dir(root).args([
            "dsc",
            "build",
            "--dir=docs/psps",
        ]))?;
    }
    Ok(())
}

fn run_strict_html(root: &Path, guidedog: &Path) -> Result<()> {
    run_cmd(Command::new(guidedog).current_dir(root).args([
        "build",
        "html",
        "docs/psps",
        "-W",
        "-n",
    ]))?;
    run_cmd(Command::new(guidedog).current_dir(root).args([
        "build",
        "html",
        "docs/perspt_book",
        "-W",
        "-n",
    ]))
}

fn run_ssg_site(root: &Path, guidedog: &Path, no_pdf: bool) -> Result<()> {
    let mut cmd = Command::new(guidedog);
    cmd.current_dir(root)
        .args(["ssg", "build", "docs", "--out", "docs/_build/site"]);
    if no_pdf {
        cmd.arg("--no-pdf");
    }
    run_cmd(&mut cmd)
}

fn stage_pdfs(root: &Path) -> Result<()> {
    let psp_pdf_src = root.join("docs/psps/_build/pdf");
    let psp_pdf_dst = root.join("docs/_build/site/psps/pdf");
    if psp_pdf_src.is_dir() {
        std::fs::create_dir_all(&psp_pdf_dst).context("creating site psps/pdf directory")?;
        for entry in std::fs::read_dir(&psp_pdf_src).context("reading docs/psps/_build/pdf")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "pdf") {
                std::fs::copy(&path, psp_pdf_dst.join(entry.file_name()))
                    .context("copying PSP PDF into site")?;
            }
        }
    }
    let book_pdf = root.join("docs/_build/site/books/perspt-book-en.pdf");
    if book_pdf.is_file() {
        std::fs::copy(&book_pdf, root.join("docs/perspt.pdf"))
            .context("copying docs/perspt.pdf")?;
        println!("Book PDF ready at docs/perspt.pdf and docs/_build/site/books/perspt-book-en.pdf");
    }
    Ok(())
}

fn run_cmd(command: &mut Command) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("running {command:?}"))?;
    if !output.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.status.success() {
        bail!(
            "{command:?} failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
