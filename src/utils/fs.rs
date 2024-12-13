use std::path::Path;

pub fn copy_skeleton<P: AsRef<Path>>(skeleton: P, target: P) -> anyhow::Result<()> {
    std::fs::create_dir_all(&target)?;
    let items = std::fs::read_dir(skeleton)?;
    for item in items {
        let item = item?;
        let path = item.path();
        if path.is_dir() {
            let new_folder_path = target.as_ref().join(item.file_name());
            copy_skeleton(&path, &new_folder_path)?;
        }
        if path.is_file() {
            std::fs::copy(path, target.as_ref().join(item.file_name()))?;
        }
    }
    Ok(())
}

#[test]
fn copy_target_dir() {
    copy_skeleton("./target", "./target2").unwrap();
}
