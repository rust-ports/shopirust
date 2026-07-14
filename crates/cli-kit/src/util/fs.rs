use std::path::{Path, PathBuf};

pub fn read_file(path: impl AsRef<Path>) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path.as_ref())
}

pub fn write_file(path: impl AsRef<Path>, content: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path.as_ref(), content)
}

pub fn copy_file(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<u64, std::io::Error> {
    if let Some(parent) = to.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(from.as_ref(), to.as_ref())
}

pub fn move_file(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<(), std::io::Error> {
    if let Some(parent) = to.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(from.as_ref(), to.as_ref())?;
    Ok(())
}

pub fn remove_file(path: impl AsRef<Path>) -> Result<(), std::io::Error> {
    std::fs::remove_file(path.as_ref())
}

pub fn remove_dir(path: impl AsRef<Path>) -> Result<(), std::io::Error> {
    std::fs::remove_dir_all(path.as_ref())
}

pub fn create_dir(path: impl AsRef<Path>) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(path.as_ref())
}

pub fn file_exists(path: impl AsRef<Path>) -> bool {
    path.as_ref().exists()
}

pub fn join_paths(base: impl AsRef<Path>, sub: impl AsRef<Path>) -> PathBuf {
    base.as_ref().join(sub.as_ref())
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

pub fn current_dir() -> Result<PathBuf, std::io::Error> {
    std::env::current_dir()
}

pub fn canonicalize(path: impl AsRef<Path>) -> Result<PathBuf, std::io::Error> {
    std::fs::canonicalize(path.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_and_read_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        write_file(&path, "hello world").unwrap();
        assert_eq!(read_file(&path).unwrap(), "hello world");
    }

    #[test]
    fn test_copy_file() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");
        write_file(&src, "content").unwrap();
        copy_file(&src, &dst).unwrap();
        assert!(file_exists(&dst));
        assert_eq!(read_file(&dst).unwrap(), "content");
    }

    #[test]
    fn test_move_file() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");
        write_file(&src, "content").unwrap();
        move_file(&src, &dst).unwrap();
        assert!(!file_exists(&src));
        assert!(file_exists(&dst));
    }

    #[test]
    fn test_remove_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        write_file(&path, "content").unwrap();
        remove_file(&path).unwrap();
        assert!(!file_exists(&path));
    }

    #[test]
    fn test_create_dir_nested() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("a").join("b").join("c");
        create_dir(&nested).unwrap();
        assert!(file_exists(&nested));
    }

    #[test]
    fn test_join_paths() {
        let base = PathBuf::from("/base");
        assert_eq!(join_paths(base, "sub/file.txt"), PathBuf::from("/base/sub/file.txt"));
    }

    #[test]
    fn test_canonicalize() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("..").join(dir.path().file_name().unwrap());
        let canon = canonicalize(&path).unwrap();
        assert_eq!(canon, dir.path().canonicalize().unwrap());
    }
}
