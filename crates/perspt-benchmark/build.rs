use std::path::{Path, PathBuf};

fn collect(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = std::fs::read_dir(directory)
        .expect("read benchmark corpus")
        .collect::<Result<Vec<_>, _>>()
        .expect("read benchmark corpus entries");
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if entry.file_type().expect("read corpus file type").is_dir() {
            collect(root, &path, files);
        } else {
            files.push(
                path.strip_prefix(root)
                    .expect("relative corpus path")
                    .into(),
            );
        }
    }
}

fn main() {
    let root = PathBuf::from("corpus");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    let canonical_root = root.canonicalize().expect("canonical benchmark corpus");
    let mut generated = String::from("&[\n");
    for relative in files {
        let source = canonical_root.join(&relative);
        generated.push_str(&format!(
            "({:?}, include_bytes!({:?}) as &'static [u8]),\n",
            relative.to_string_lossy(),
            source,
        ));
    }
    generated.push_str("]\n");
    let output =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("embedded_corpus.rs");
    std::fs::write(output, generated).expect("write embedded corpus manifest");
}
