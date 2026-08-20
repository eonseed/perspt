static EMBEDDED_CORPUS: &[(&str, &[u8])] =
    include!(concat!(env!("OUT_DIR"), "/embedded_corpus.rs"));

pub(super) fn materialize_corpus() -> anyhow::Result<tempfile::TempDir> {
    let directory = tempfile::tempdir()?;
    for (relative, contents) in EMBEDDED_CORPUS {
        let mut relative = std::path::PathBuf::from(relative);
        if relative.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml.fixture") {
            relative.set_file_name("Cargo.toml");
        }
        let destination = directory.path().join(relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(destination, contents)?;
    }
    Ok(directory)
}

#[cfg(test)]
pub(super) fn source_corpus_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus")
}
