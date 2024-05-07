use std::io::{Read, Write};
use base64::Engine;
use js_sys::Function;
use wasm_bindgen::prelude::*;
use zip::ZipArchive;
use serde::{Deserialize, Serialize};
use web_sys::console;
use zip::write::FileOptions;

mod utils;

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct CFBundlePrimaryIcon {
    pub CFBundleIconName: Option<String>,
    pub CFBundleIconFiles: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct CFBundleIcons {
    pub CFBundlePrimaryIcon: Option<CFBundlePrimaryIcon>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct IpaXmlInfo {
    pub CFBundleName: Option<String>,
    //应用名称
    pub CFBundleDisplayName: Option<String>,
    //显示名称
    pub CFBundleIcons: Option<CFBundleIcons>,
    pub CFBundleIconFiles: Option<Vec<String>>,
    //icon图标
    pub CFBundleIdentifier: Option<String>,
    //版本号
    pub CFBundleShortVersionString: Option<String>,
    //最低运行系统
    pub MinimumOSVersion: Option<String>,
}

impl IpaXmlInfo {
    pub fn icon_files(&self) -> Vec<String> {
        let mut icon_list = Vec::new();
        if let Some(icons) = &self.CFBundleIcons {
            if let Some(primary_icon) = &icons.CFBundlePrimaryIcon {
                if let Some(icon_files) = &primary_icon.CFBundleIconFiles {
                    icon_list = icon_files.clone();
                }
            }
        }
        //append icon file
        if let Some(icon_files) = &self.CFBundleIconFiles {
            icon_list.extend(icon_files.clone());
        }
        icon_list.sort();
        icon_list.reverse();
        return icon_list;
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct IpaInfo {
    app_name: String,
    app_display_name: String,
    app_bundle_id: String,
    app_version: String,
    app_min_os_version: String,
    app_icon: Vec<u8>,
    plist: String,
}

pub fn get_relative_path<T: AsRef<str>>(file_name: &T, input_dir_str: &T) -> String {
    let file_name = file_name.as_ref();
    let input_dir_str = input_dir_str.as_ref();
    let file_name = &file_name[(input_dir_str.len() + 1)..];
    return file_name.to_string();
}

#[wasm_bindgen]
pub fn create(zip_file_bytes: &[u8], app_name: String, app_bundle_id: String, app_version: String, app_min_os_version: String, plist: String, callback: Option<Function>) -> Result<Vec<u8>, String> {
    // console::log_1(&"create".into());
    // console::log_1(&format!("app_name: {}", app_name).into());
    // console::log_1(&format!("app_bundle_id: {}", app_bundle_id).into());
    // console::log_1(&format!("app_version: {}", app_version).into());
    // console::log_1(&format!("app_min_os_version: {}", app_min_os_version).into());
    // console::log_1(&format!("plist: {}", plist).into());
    let mut plist_xml: plist::Value = plist::from_bytes(plist.as_bytes())
        .map_err(|e| format!("Failed to parse plist: {}", e))?;
    //set app_name
    let mut xml = plist_xml.as_dictionary_mut()
        .ok_or("Failed to parse plist")?;
    xml.insert("CFBundleName".to_string(), plist::Value::String(app_name));
    //set app_bundle_id
    xml.insert("CFBundleIdentifier".to_string(), plist::Value::String(app_bundle_id));
    //set app_version
    xml.insert("CFBundleShortVersionString".to_string(), plist::Value::String(app_version));
    //set app_min_os_version
    xml.insert("MinimumOSVersion".to_string(), plist::Value::String(app_min_os_version));

    let mut info_plist_buf = Vec::new();
    plist_xml.to_writer_xml(&mut info_plist_buf)
        .map_err(|e| format!("Failed to write xml: {}", e))?;

    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::DEFLATE)
        .compression_level(Some(3))
        .unix_permissions(0o755);

    let zip_file = std::io::Cursor::new(zip_file_bytes);
    let mut zip_archive = zip::ZipArchive::new(zip_file)
        .map_err(|e| format!("Failed to parse zip: {}", e))?;

    let mut unzip_total_bytes = 0;//总解压大小
    for i in 0..zip_archive.len() {
        let mut file_entry = zip_archive.by_index(i)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        unzip_total_bytes += file_entry.size();
    }

    let mut zip_bytes = 0;
    let mut pre_process = 0;
    for i in 0..zip_archive.len() {
        let mut file_entry = zip_archive.by_index(i)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        let entry_name = file_entry.name().to_string();

        //判断是不是文件夹
        if entry_name.ends_with("/") {
            zip.add_directory(entry_name, options)
                .map_err(|e| format!("Failed to add directory: {}", e))?;
            zip_bytes += file_entry.size();
            continue;
        }

        let split_count = entry_name.chars().filter(|&c| c == '/').count();
        if entry_name.starts_with("Payload/") && split_count == 2 && entry_name.ends_with(".app/Info.plist") {
            zip.start_file(entry_name, options)
                .map_err(|e| format!("Failed to start file: {}", e))?;
            zip.write_all(&info_plist_buf)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        } else {
            zip.start_file(entry_name, options)
                .map_err(|e| format!("Failed to start file: {}", e))?;

            let batch_wirte_byte = 1024 * 1024 * 8;
            let mut buf = vec![0; batch_wirte_byte];
            let mut read_bytes = 0;
            loop {
                let n = file_entry.read(&mut buf)
                    .map_err(|e| format!("Failed to read file: {}", e))?;
                if n == 0 {
                    break;
                }
                zip.write_all(&buf[..n])
                    .map_err(|e| format!("Failed to write file: {}", e))?;
                read_bytes += n as u64;
                zip_bytes += n as u64;
                let process = (zip_bytes as f64 / unzip_total_bytes as f64 * 100.0).min(100.0) as u8;
                if (process != pre_process) {
                    if let Some(callback) = &callback {
                        callback.call1(&JsValue::NULL, &JsValue::from(process)).unwrap();
                    }
                    pre_process = process;
                }
            }
        }
    }
    //返回压缩后的文件byte
    let zip_bytes = zip.finish()
        .map_err(|e| format!("Failed to finish zip: {}", e))?
        .into_inner();
    return Ok(zip_bytes);
}

#[wasm_bindgen]
pub fn parser(bytes: &[u8], callback: Option<Function>) -> Result<JsValue, String> {
    let seek = std::io::Cursor::new(bytes);
    let mut entrys = ZipArchive::new(seek)
        .map_err(|e| format!("Failed to parse zip: {}", e))?;
    let mut plist_bytes = Vec::new();
    let mut macho_name = None;
    for i in 0..entrys.len() {
        let mut file_entry = entrys.by_index(i)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        let entry_name = file_entry.name().to_string();
        let split_count = entry_name.chars().filter(|&c| c == '/').count();
        if split_count != 2 {
            continue;
        }
        if entry_name.starts_with("Payload/") && entry_name.ends_with(".app/Info.plist") {
            macho_name = Some(entry_name.split("/").collect::<Vec<&str>>()[1].to_string());
            file_entry.read_to_end(&mut plist_bytes)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            break;
        }
    }
    if plist_bytes.len() == 0 {
        return Err("Failed to find Info.plist".to_string());
    }
    let macho_name = macho_name
        .ok_or("Failed to find macho name")?;
    let plist_info: IpaXmlInfo = plist::from_bytes(&plist_bytes)
        .map_err(|e| format!("Failed to parse plist: {}", e))?;

    let mut info_plist_buf = Vec::new();
    if !plist_bytes.starts_with(b"<?xml") {
        //plist转xml
        let plist_origin: plist::Value = plist::from_bytes(&plist_bytes)
            .map_err(|e| format!("Failed to parse plist: {}", e))?;
        plist_origin.to_writer_xml(&mut info_plist_buf)
            .map_err(|e| format!("Failed to write xml: {}", e))?;
    } else {
        info_plist_buf = plist_bytes;
    }

    let mut icon = None;
    if !&plist_info.icon_files().is_empty() {
        let mut icons = Vec::new();
        for i in 0..entrys.len() {
            let mut file_entry = entrys.by_index(i)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            let entry_name = file_entry.name().to_string();
            let split_count = entry_name.chars().filter(|&c| c == '/').count();
            if split_count != 2 {
                continue;
            }
            let mut find = false;
            for icon_file_name in plist_info.icon_files() {
                if format!("Payload/{}/{}", macho_name, icon_file_name) == entry_name
                    || (entry_name.starts_with("Payload/")
                    && entry_name.contains(format!(".app/{}", icon_file_name).as_str())
                    && entry_name.ends_with(".png"))
                {
                    icons.push(entry_name);
                    find = true;
                    let mut buf = Vec::new();
                    file_entry.read_to_end(&mut buf)
                        .map_err(|e| format!("Failed to read file: {}", e))?;
                    icon = Some(buf);
                    break;
                }
            }
            if find {
                break;
            }
        }
    }
    let app_name = plist_info.CFBundleName.unwrap_or("".to_string());
    let app_display_name = plist_info.CFBundleDisplayName.unwrap_or("".to_string());
    let app_bundle_id = plist_info.CFBundleIdentifier.unwrap_or("".to_string());
    let app_version = plist_info.CFBundleShortVersionString.unwrap_or("".to_string());
    let app_min_os_version = plist_info.MinimumOSVersion.unwrap_or("".to_string());
    let plist = String::from_utf8_lossy(&info_plist_buf).to_string();
    let info = IpaInfo {
        app_name,
        app_display_name,
        app_bundle_id,
        app_version,
        app_min_os_version,
        app_icon: icon.unwrap_or(Vec::new()),
        plist,
    };
    let info = serde_wasm_bindgen::to_value(&info).unwrap();
    return Ok(info);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser() {
        let file = std::path::Path::new("./app2.ipa");
        let bytes = std::fs::read(file).unwrap();
        match parser(&bytes, None) {
            Ok(_) => {}
            Err(e) => {
                println!("{}", e);
                assert!(false);
            }
        }
    }
}