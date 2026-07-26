use crate::error::{AudioError, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use std::path::Path;
use windows::core::{Interface, PCWSTR};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SHGFI_ICON, SHGFI_LARGEICON,
    SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY, SHFILEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL, HICON};

pub fn icon_data_url_for_path(path: &str) -> Option<String> {
    if path.is_empty() || !Path::new(path).exists() {
        return None;
    }
    extract_icon_png(path)
        .ok()
        .map(|bytes| format!("data:image/png;base64,{}", B64.encode(bytes)))
}

fn extract_icon_png(path: &str) -> Result<Vec<u8>> {
    if let Ok(bytes) = extract_via_shell_item(path, 32) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }
    extract_via_shgetfileinfo(path)
}

fn extract_via_shell_item(path: &str, size: i32) -> Result<Vec<u8>> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let item: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)?;
        let hbmp = item.GetImage(
            windows::Win32::Foundation::SIZE {
                cx: size,
                cy: size,
            },
            SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
        )?;
        let png = hbitmap_to_png(hbmp, size, size)?;
        let _ = DeleteObject(HGDIOBJ(hbmp.0));
        Ok(png)
    }
}

fn extract_via_shgetfileinfo(path: &str) -> Result<Vec<u8>> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = SHFILEINFOW::default();
    unsafe {
        let ok = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
        );
        if ok == 0 || info.hIcon.is_invalid() {
            return Err(AudioError::message("SHGetFileInfoW failed"));
        }
        let png = hicon_to_png(info.hIcon, 32)?;
        let _ = DestroyIcon(info.hIcon);
        Ok(png)
    }
}

fn hicon_to_png(icon: HICON, size: i32) -> Result<Vec<u8>> {
    unsafe {
        let hdc = GetDC(None);
        if hdc.is_invalid() {
            return Err(AudioError::message("GetDC failed"));
        }
        let mem = CreateCompatibleDC(Some(hdc));
        let hbmp = CreateCompatibleBitmap(hdc, size, size);
        let old = SelectObject(mem, HGDIOBJ(hbmp.0));
        let _ = DrawIconEx(mem, 0, 0, icon, size, size, 0, None, DI_NORMAL);
        let png = hbitmap_to_png(hbmp, size, size)?;
        SelectObject(mem, old);
        let _ = DeleteObject(HGDIOBJ(hbmp.0));
        let _ = DeleteDC(mem);
        ReleaseDC(None, hdc);
        Ok(png)
    }
}

fn hbitmap_to_png(hbmp: HBITMAP, width: i32, height: i32) -> Result<Vec<u8>> {
    unsafe {
        let hdc = GetDC(None);
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let lines = GetDIBits(
            hdc,
            hbmp,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        if lines == 0 {
            return Err(AudioError::message("GetDIBits failed"));
        }
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }
        encode_png_rgba(&pixels, width as u32, height as u32)
    }
}

fn encode_png_rgba(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    use std::io::Write;

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFFFFFFu32;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                let mask = if crc & 1 != 0 { 0xEDB88320 } else { 0 };
                crc = (crc >> 1) ^ mask;
            }
        }
        !crc
    }

    fn chunk(tag: &[u8; 4], data: &[u8], out: &mut Vec<u8>) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(tag);
        out.extend_from_slice(data);
        let mut crc_data = Vec::with_capacity(4 + data.len());
        crc_data.extend_from_slice(tag);
        crc_data.extend_from_slice(data);
        out.extend_from_slice(&crc32(&crc_data).to_be_bytes());
    }

    let mut raw = Vec::with_capacity(((width * 4 + 1) * height) as usize);
    for y in 0..height {
        raw.push(0);
        let start = (y * width * 4) as usize;
        let end = start + (width * 4) as usize;
        raw.extend_from_slice(&rgba[start..end]);
    }

    fn adler32(data: &[u8]) -> u32 {
        let mut a = 1u32;
        let mut b = 0u32;
        for &byte in data {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    let mut zlib = Vec::new();
    zlib.push(0x78);
    zlib.push(0x01);
    let mut offset = 0usize;
    while offset < raw.len() {
        let remaining = raw.len() - offset;
        let take = remaining.min(65535);
        let is_last = offset + take >= raw.len();
        zlib.push(if is_last { 0x01 } else { 0x00 });
        let len = take as u16;
        let nlen = !len;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&nlen.to_le_bytes());
        zlib.extend_from_slice(&raw[offset..offset + take]);
        offset += take;
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = Vec::new();
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
    let mut ihdr = Vec::new();
    ihdr.write_all(&width.to_be_bytes()).ok();
    ihdr.write_all(&height.to_be_bytes()).ok();
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr, &mut out);
    chunk(b"IDAT", &zlib, &mut out);
    chunk(b"IEND", &[], &mut out);
    Ok(out)
}
