use js_sys::Function;
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Cursor, Read, Write};
use wasm_bindgen::prelude::*;
use zip::ZipArchive;

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

pub fn convert_icon_to_png(icon_bytes: &[u8]) -> Result<Vec<u8>, String> {
    // let reader = Cursor::new(icon_bytes);
    // let input = image_convert::ImageResource::from_reader(reader)
    //     .map_err(|e| format!("Failed to load icon: {}", e))?;
    //
    // let mut output = image_convert::ImageResource::with_capacity(1024 * 1024 * 8);
    // let config = image_convert::PNGConfig::new();
    // image_convert::to_png(&mut output, &input, &config)
    //     .map_err(|e| format!("Failed to convert icon: {}", e))?;
    // let bytes = output.into_vec().ok_or("Failed to get icon bytes")?;

    // let image =
    //     image::load_from_memory(icon_bytes).map_err(|e| format!("Failed to load icon1: {}", e))?;
    // let mut cursor_bytes = Cursor::new(Vec::new());
    // image
    //     .write_to(&mut cursor_bytes, image::ImageFormat::Png)
    //     .map_err(|e| format!("Failed to write icon2: {}", e))?;
    // let bytes = cursor_bytes.into_inner();

    //判断是不是jpg
    if icon_bytes.len() > 3
        && icon_bytes[0] == 0xFF
        && icon_bytes[1] == 0xD8
        && icon_bytes[2] == 0xFF
    {
        let reader = Cursor::new(icon_bytes);
        let mut buf_reader = BufReader::new(reader);
        let mut decoder = jpeg_decoder::Decoder::new(&mut buf_reader);
        let pixels = decoder.decode().expect("failed to decode image");
        let metadata = decoder.info().unwrap();

        let mut bytes = Vec::new();
        let wirter = Cursor::new(&mut bytes);
        let mut encoder = png::Encoder::new(wirter, metadata.width as u32, metadata.height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
        writer
            .finish()
            .map_err(|e| format!("Failed to write icon2: {}", e))?;
        //print icon size
        // console::log_1(&format!("icon size: {}", bytes.len()).into());
        return Ok(bytes);
    }

    return Ok(icon_bytes.to_vec());
}

#[wasm_bindgen]
pub fn create(
    zip_file_bytes: &[u8],
    icon_bytes: &[u8],
    app_name: String,
    app_bundle_id: String,
    app_version: String,
    plist: String,
    remove_device_limit: bool,
    remove_jump: bool,
    open_file_share: bool,
    zip_level: u8,
    callback: Option<Function>,
) -> Result<Vec<u8>, String> {
    if app_name.trim().is_empty() {
        return Err("app_name required".to_string());
    }
    if app_bundle_id.trim().is_empty() {
        return Err("app_bundle_id required".to_string());
    }
    if app_version.trim().is_empty() {
        return Err("app_version required".to_string());
    }
    if plist.trim().is_empty() {
        return Err("plist required".to_string());
    }

    let icon = if icon_bytes.len() > 0 {
        Some(
            convert_icon_to_png(icon_bytes)
                .map_err(|e| format!("Failed to convert icon3: {}", e))?,
        )
    } else {
        None
    };

    let mut plist_xml: plist::Value =
        plist::from_bytes(plist.as_bytes()).map_err(|e| format!("Failed to parse plist: {}", e))?;
    //set app_name
    let xml = plist_xml
        .as_dictionary_mut()
        .ok_or("Failed to parse plist")?;
    xml.insert("CFBundleName".to_string(), plist::Value::String(app_name));
    //set app_bundle_id
    xml.insert(
        "CFBundleIdentifier".to_string(),
        plist::Value::String(app_bundle_id),
    );
    //set app_version
    xml.insert(
        "CFBundleShortVersionString".to_string(),
        plist::Value::String(app_version),
    );
    if remove_device_limit {
        xml.insert(
            "MinimumOSVersion".to_string(),
            plist::Value::String("10.0".to_string()),
        );
    }
    if remove_jump {
        xml.insert("CFBundleURLTypes".to_string(), plist::Value::Array(vec![]));
    }
    if open_file_share {
        xml.insert(
            "UIFileSharingEnabled".to_string(),
            plist::Value::Boolean(true),
        );
        xml.insert(
            "UISupportsDocumentBrowser".to_string(),
            plist::Value::Boolean(true),
        );
    }
    //set new app_icon
    if let Some(ref _icon) = icon {
        let mut cfbundle_primary_icon = plist::Dictionary::new();
        let cfbundle_icon_files = vec!["icon_app_ipadump_com.png".to_string()];
        cfbundle_primary_icon.insert(
            "CFBundleIconFiles".to_string(),
            plist::Value::Array(
                cfbundle_icon_files
                    .into_iter()
                    .map(plist::Value::String)
                    .collect(),
            ),
        );
        cfbundle_primary_icon.insert(
            "CFBundleIconName".to_string(),
            plist::Value::String("icon_app_ipadump_com".to_string()),
        );
        let mut cfbundle_primary_icon_wrap = plist::Dictionary::new();
        cfbundle_primary_icon_wrap.insert(
            "CFBundlePrimaryIcon".to_string(),
            plist::Value::Dictionary(cfbundle_primary_icon),
        );
        xml.insert(
            "CFBundleIcons".to_string(),
            plist::Value::Dictionary(cfbundle_primary_icon_wrap),
        );
    }

    let mut info_plist_buf = Vec::new();
    plist_xml
        .to_writer_xml(&mut info_plist_buf)
        .map_err(|e| format!("Failed to write xml: {}", e))?;

    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let zip_leve = zip_level.min(9).max(1);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::DEFLATE)
        .compression_level(Some(zip_leve as i64))
        .unix_permissions(0o755);

    let zip_file = std::io::Cursor::new(zip_file_bytes);
    let mut zip_archive =
        zip::ZipArchive::new(zip_file).map_err(|e| format!("Failed to parse zip: {}", e))?;

    let mut unzip_total_bytes = 0; //总解压大小
    for i in 0..zip_archive.len() {
        let file_entry = zip_archive
            .by_index(i)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        unzip_total_bytes += file_entry.size();
    }

    let mut zip_bytes = 0;
    let mut pre_process = 0;

    let mut macho_name = None;
    for i in 0..zip_archive.len() {
        let mut file_entry = zip_archive
            .by_index(i)
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
        if entry_name.starts_with("Payload/")
            && split_count == 2
            && entry_name.ends_with(".app/Info.plist")
        {
            macho_name = Some(entry_name.split("/").collect::<Vec<&str>>()[1].to_string());
            zip.start_file(entry_name, options)
                .map_err(|e| format!("Failed to start file: {}", e))?;
            zip.write_all(&info_plist_buf)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        } else {
            zip.start_file(entry_name, options)
                .map_err(|e| format!("Failed to start file: {}", e))?;

            let batch_wirte_byte = 1024 * 1024 * 8;
            let mut buf = vec![0; batch_wirte_byte];
            loop {
                let n = file_entry
                    .read(&mut buf)
                    .map_err(|e| format!("Failed to read file: {}", e))?;
                if n == 0 {
                    break;
                }
                zip.write_all(&buf[..n])
                    .map_err(|e| format!("Failed to write file: {}", e))?;
                zip_bytes += n as u64;
                let process =
                    (zip_bytes as f64 / unzip_total_bytes as f64 * 100.0).min(100.0) as u8;
                if process != pre_process {
                    if let Some(callback) = &callback {
                        callback
                            .call1(&JsValue::NULL, &JsValue::from(process))
                            .unwrap();
                    }
                    pre_process = process;
                }
            }
        }
    }
    let macho_name = macho_name.ok_or("Failed to find macho name")?;
    if let Some(icon) = icon {
        zip.start_file(
            format!("Payload/{}/icon_app_ipadump_com.png", macho_name),
            options,
        )
        .map_err(|e| format!("Failed to start file: {}", e))?;
        zip.write_all(&icon)
            .map_err(|e| format!("Failed to write file: {}", e))?;
    }

    //返回压缩后的文件byte
    let zip_bytes = zip
        .finish()
        .map_err(|e| format!("Failed to finish zip: {}", e))?
        .into_inner();
    return Ok(zip_bytes);
}

#[wasm_bindgen]
pub fn parser(bytes: &[u8], _callback: Option<Function>) -> Result<JsValue, String> {
    let seek = std::io::Cursor::new(bytes);
    let mut entrys = ZipArchive::new(seek).map_err(|e| format!("Failed to parse zip: {}", e))?;
    let mut plist_bytes = Vec::new();
    let mut macho_name = None;
    for i in 0..entrys.len() {
        let mut file_entry = entrys
            .by_index(i)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        let entry_name = file_entry.name().to_string();
        let split_count = entry_name.chars().filter(|&c| c == '/').count();
        if split_count != 2 {
            continue;
        }
        if entry_name.starts_with("Payload/") && entry_name.ends_with(".app/Info.plist") {
            macho_name = Some(entry_name.split("/").collect::<Vec<&str>>()[1].to_string());
            file_entry
                .read_to_end(&mut plist_bytes)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            break;
        }
    }
    if plist_bytes.len() == 0 {
        return Err("Failed to find Info.plist".to_string());
    }
    let macho_name = macho_name.ok_or("Failed to find macho name")?;
    let plist_info: IpaXmlInfo =
        plist::from_bytes(&plist_bytes).map_err(|e| format!("Failed to parse plist: {}", e))?;

    let mut info_plist_buf = Vec::new();
    if !plist_bytes.starts_with(b"<?xml") {
        //plist转xml
        let plist_origin: plist::Value =
            plist::from_bytes(&plist_bytes).map_err(|e| format!("Failed to parse plist: {}", e))?;
        plist_origin
            .to_writer_xml(&mut info_plist_buf)
            .map_err(|e| format!("Failed to write xml: {}", e))?;
    } else {
        info_plist_buf = plist_bytes;
    }

    let mut icon = None;
    if !&plist_info.icon_files().is_empty() {
        let mut icons = Vec::new();
        for i in 0..entrys.len() {
            let mut file_entry = entrys
                .by_index(i)
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
                    file_entry
                        .read_to_end(&mut buf)
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
    let app_version = plist_info
        .CFBundleShortVersionString
        .unwrap_or("".to_string());
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
    fn test_png_covert() {
        let file = std::path::Path::new("./icon.png");
        let bytes = std::fs::read(file).unwrap();
        match convert_icon_to_png(&bytes) {
            Ok(_) => {}
            Err(e) => {
                println!("{}", e);
                assert!(false);
            }
        }
    }

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
