pub(super) fn materialize_corpus() -> anyhow::Result<tempfile::TempDir> {
    let source = std::env::var_os("PERSPT_BENCHMARK_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus"));
    anyhow::ensure!(
        source.is_dir(),
        "benchmark corpus not found at {}; set PERSPT_BENCHMARK_CORPUS to the private corpus directory",
        source.display()
    );
    let directory = tempfile::tempdir()?;
    copy_corpus(&source, &source, directory.path())?;
    Ok(directory)
}

fn copy_corpus(
    root: &std::path::Path,
    source: &std::path::Path,
    destination: &std::path::Path,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            copy_corpus(root, &path, destination)?;
            continue;
        }
        let mut relative = path.strip_prefix(root)?.to_path_buf();
        if relative.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml.fixture") {
            relative.set_file_name("Cargo.toml");
        }
        let target = destination.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(path, target)?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn source_corpus_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus")
}
