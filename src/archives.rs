#[cfg(unix)]
use std::fs::remove_dir_all;
#[cfg(windows)]
use std::fs::File;
#[cfg(windows)]
use std::io::copy;
#[cfg(unix)]
use std::path::PathBuf;
use std::{fs::create_dir_all, io::Cursor, path::Path};

use anyhow::Result;
#[cfg(unix)]
use flate2::read::GzDecoder;
use reqwest::blocking::Response;
#[cfg(unix)]
use tar::{Archive, Unpacked};
#[cfg(target_os = "windows")]
use zip::ZipArchive;

#[cfg(target_os = "windows")]
pub fn extract_archive(bytes: Response, path: &Path) -> Result<()> {
    let reader = Cursor::new(bytes.bytes().unwrap());
    let mut archive = ZipArchive::new(reader).unwrap();

    println!("Extracting...");

    for i in 0..archive.len() {
        let mut item = archive.by_index(i).unwrap();
        let file_path = item.sanitized_name();
        let file_path = file_path.to_string_lossy();

        let mut new_path = path.to_owned();
        if let Some(index) = file_path.find('\\') {
            new_path.push(file_path[index + 1..].to_owned());
        }

        if item.is_dir() && !new_path.exists() {
            create_dir_all(new_path.to_owned())
                .unwrap_or_else(|_| panic!("Could not create new folder: {:?}", new_path));
        }

        if item.is_file() {
            let mut file = File::create(&*new_path)?;
            copy(&mut item, &mut file)
                .unwrap_or_else(|_| panic!("Couldn't write to {:?}", new_path));
        }
    }

    println!(
        "Extracted to {}",
        // Have to remove \\?\ prefix 🤮
        path.to_str()
            .unwrap()
            .strip_prefix("\\\\?\\")
            .unwrap_or(path.to_str().unwrap())
    );

    Result::Ok(())
}

#[cfg(unix)]
pub fn extract_archive(bytes: Response, path: &Path) -> Result<()> {
    let reader = Cursor::new(bytes.bytes().unwrap());
    let tar = GzDecoder::new(reader);
    let mut archive = Archive::new(tar);

    let version_dir_path = path.to_owned();
    create_dir_all(version_dir_path.to_owned()).expect("fuck");

    println!("Extracting...");

    let result = archive
        .entries()
        .map_err(anyhow::Error::from)?
        .filter_map(|e| e.ok())
        .map(|mut entry| -> Result<Unpacked> {
            let file_path = entry.path()?.to_owned();
            let file_path = file_path.to_str().unwrap();

            let new_path: PathBuf = if let Some(index) = file_path.find('/') {
                path.to_owned().join(file_path[index + 1..].to_owned())
            } else {
                // This happens if it's the root index, the base folder
                path.to_owned()
            };

            entry.set_preserve_permissions(false);
            entry.unpack(&new_path).map_err(anyhow::Error::from)
        });

    let errors: Vec<anyhow::Error> = result
        .into_iter()
        .filter(|result| result.is_err())
        .map(|result| result.unwrap_err())
        .collect();

    if !errors.is_empty() {
        remove_dir_all(version_dir_path).expect("Couldn't clean up version.");

        return Result::Err(anyhow::anyhow!(
            "Failed to extract all files:\n{:?}",
            errors
                .into_iter()
                .map(|err| err.to_string())
                .collect::<Vec<String>>()
                .join("/n")
        ));
    }

    println!("Extracted to {:?}", version_dir_path);

    Result::Ok(())
}
