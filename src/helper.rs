use std::ffi::c_void;
use std::io::Cursor;

use image::{ImageBuffer, Rgba};
use scopeguard::defer;
use windows::core::HSTRING;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{GetLastError, HWND, RECT};

use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, GetBitmapBits,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GetClassNameW, GetIconInfo, GetParent, GetWindowInfo, GetWindowTextW, LoadIconW, SetWindowDisplayAffinity, ICONINFO,
    WDA_EXCLUDEFROMCAPTURE, WDA_NONE, WINDOWINFO,
};

#[derive(Default, Debug)]
pub struct WindowInfo {
    pub hwnd: u64,
    pub text: String,
    pub class_name: String,
    pub style: u32,
    pub ex_style: u32,
    pub rect: RECT,
    pub parent_hwnd: u64,
    pub parent_text: String,
    pub parent_class_name: String,
}

pub fn get_parent_window(hwnd: u64) -> u64 {
    unsafe {
        let hwnd = GetParent(HWND(hwnd as _));
        match hwnd {
            Ok(hwnd) => hwnd.0 as _,
            Err(_) => 0,
        }
    }
}
pub fn get_window_info(hwnd: u64) -> WindowInfo {
    unsafe {
        let info = &mut WINDOWINFO::default();
        GetWindowInfo(HWND(hwnd as _), info).unwrap();
        let p_hwnd = get_parent_window(hwnd);
        WindowInfo {
            hwnd: hwnd,
            text: get_window_text(hwnd),
            class_name: get_class_name(hwnd),
            style: info.dwStyle.0,
            ex_style: info.dwExStyle.0,
            rect: info.rcWindow,
            parent_hwnd: p_hwnd,
            parent_text: if p_hwnd != 0 {
                get_window_text(p_hwnd)
            } else {
                String::new()
            },
            parent_class_name: if p_hwnd != 0 {
                get_class_name(p_hwnd)
            } else {
                String::new()
            },
        }
    }
}

pub fn get_window_text(hwnd: u64) -> String {
    unsafe {
        let text: &mut [u16] = &mut [0; 255];
        let size = GetWindowTextW(HWND(hwnd as _), text);
        let text = &text[0..size as usize];
        String::from_utf16_lossy(text)
    }
}
pub fn get_class_name(hwnd: u64) -> String {
    unsafe {
        let text: &mut [u16] = &mut [0; 255];
        let size = GetClassNameW(HWND(hwnd as _), text);
        let text = &text[0..size as usize];
        String::from_utf16_lossy(text)
    }
}

pub fn set_window_deny_capture(hwnd: u64, flag: bool) -> bool {
    let show = if flag {
        WDA_EXCLUDEFROMCAPTURE
    } else {
        WDA_NONE
    };
    unsafe { SetWindowDisplayAffinity(HWND(hwnd as _), show).is_ok() }
}


pub fn load_icon_to_png(resource_name: &str) -> Result<Vec<u8>, String> {
    unsafe {
        let h_module = GetModuleHandleW(None).unwrap();
        let resource_name = PCWSTR(HSTRING::from(resource_name).as_ptr());
        let icon = LoadIconW(h_module, resource_name);
        if icon.is_err() {
            return Err(format!("LoadIconW failed: {}", GetLastError().0));
        }
        let icon = icon.unwrap();
        let mut info = ICONINFO::default();
        if GetIconInfo(icon, &mut info).is_err() {
            return Err(format!("GetIconInfo failed: {}", GetLastError().0));
        }

        let hdc = CreateCompatibleDC(None);
        if hdc.is_invalid() {
            return Err("CreateCompatibleDC failed".to_string());
        }
        defer! {DeleteDC(hdc).as_bool();}

        let width = info.xHotspot * 2; // icon width is twice the hotspot x coordinate
        let height = info.yHotspot * 2; // icon height is twice the hotspot y coordinate

        // 获取掩码位图
        let hbm_mask = info.hbmMask;

        let mut mask_pixels = vec![0u8; (width * height) as usize];
        let _bytes_written = GetBitmapBits(
            hbm_mask,
            mask_pixels.len() as i32,
            mask_pixels.as_mut_ptr() as *mut c_void,
        );

        // 获取颜色位图
        let hbm_color = info.hbmColor;
        // println!("hbmColor: {}", hbm_color.is_invalid());
        let mut color_pixels = vec![0u8; (width * height * 4) as usize];
        let _bytes_written = GetBitmapBits(
            hbm_color,
            color_pixels.len() as i32,
            color_pixels.as_mut_ptr() as *mut c_void,
        );

        // 合并掩码和颜色位图
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let color_index = (y * width * 4 + x * 4) as usize;
                let mask_index = (y * width + x) as usize;
                let inner_mask_index = mask_index / 8;
                let inner_mask_bit_index = 8 - (mask_index % 8 + 1);
                let mask_value = mask_pixels[inner_mask_index];
                let alpha = (mask_value & (1 << inner_mask_bit_index)) >> inner_mask_bit_index;

                pixels[color_index] = color_pixels[color_index + 2]; // 红R
                pixels[color_index + 1] = color_pixels[color_index + 1]; // 绿G
                pixels[color_index + 2] = color_pixels[color_index + 0]; // 蓝B
                pixels[color_index + 3] = (1 - alpha) * 255; // alpha
            }
        }

        DestroyIcon(icon).unwrap();

        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, &*pixels).unwrap();
        let png_data = Vec::new();
        let mut cursor = Cursor::new(png_data);
        // img.save_with_format("f:/tmp/testicon9.png", image::ImageFormat::Png)
        //     .unwrap();
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        let data = cursor.get_mut().to_owned();
        // println!("data len: {}", data.len());
        Ok(data)
    }
}

