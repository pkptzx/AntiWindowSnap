#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::{
    ffi::{c_void, OsStr, OsString},
    fs::File,
    io::{BufRead, BufReader},
    os::windows::process::CommandExt,
};

use anti_window_snap::anti_window;
use dashmap::DashMap;
use fltk::enums::{Color, Event};
use fltk::image::PngImage;
use fltk::input::Input;
use fltk::{app, enums, prelude::*, window::Window};
use fltk::{app::Scheme, enums::Align};
use fltk::{button, dialog, frame, group, input, text};
use once_cell::sync::Lazy;
use windows::{
    core::w,
    Win32::{
        Foundation::{BOOL, COLORREF, HWND, POINT, RECT},
        Graphics::Gdi::{
            ClientToScreen, CombineRgn, CreateRectRgn, CreateRectRgnIndirect, CreateSolidBrush, FillRgn, GetDC, GetWindowDC, InflateRect, InvertRect,
            OffsetRect, RedrawWindow, ReleaseDC, SetRect, RDW_FRAME, RDW_INTERNALPAINT, RDW_INVALIDATE,
            RDW_UPDATENOW, RGN_DIFF,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
            WindowsAndMessaging::{
                EnumWindows, GetClientRect, GetParent, GetPhysicalCursorPos, GetWindowLongPtrW,
                GetWindowRect, GetWindowTextW, LoadCursorW, SetCursor, WindowFromPhysicalPoint,
                CHILDID_SELF, EVENT_OBJECT_CREATE, EVENT_OBJECT_NAMECHANGE, GWL_STYLE,
                OBJID_WINDOW, WINDOW_STYLE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNTHREAD,
                WS_CHILD,
            },
        },
    },
};
mod helper;

#[derive(Debug, PartialEq, Eq)]
enum STATE {
    UnProcessed,
    Processing,
    Processed,
    Completed,
}

static mut WINDOW_CACHE: Lazy<DashMap<u64, (STATE, STATE)>> = Lazy::new(|| initialize_map());
static mut ROOT_WINDOW_CACHE: Lazy<DashMap<u64, bool>> = Lazy::new(|| initialize_root_window_map());
static mut CONFIG_WINDOW_TITLES: Vec<String> = Vec::new();
static mut LABELS_CACHE: Lazy<DashMap<String, frame::Frame>> = Lazy::new(|| initialize_tips_map());

fn initialize_map() -> DashMap<u64, (STATE, STATE)> {
    DashMap::new()
}
fn initialize_root_window_map() -> DashMap<u64, bool> {
    DashMap::new()
}

fn initialize_tips_map() -> DashMap<String, frame::Frame> {
    DashMap::new()
}

fn main() {
    let path = std::env::current_dir()
        .unwrap_or_default()
        .join("config.txt");
    let mut txt_buf = text::TextBuffer::default();
    if path.exists() {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);

        for line_result in reader.lines() {
            let line = line_result.unwrap();
            if !line.trim().is_empty() {
                unsafe {
                    // println!("已加载配置:{}", line);
                    CONFIG_WINDOW_TITLES.push(line.clone());
                    txt_buf.append((line + "\n").as_str());
                }
            };
        }
    }

    let app = app::App::default().with_scheme(Scheme::Gtk);
    let mut wind = Window::default().with_size(640, 380).center_screen();

    let mut col = group::Flex::default_fill().column();
    let mut mp = group::Flex::default().row();
    frame::Frame::default();

    let mut left_panel_layout = group::Flex::default().column();

    let mut tip1_label = frame::Frame::default()
        .with_label("窗口标题一行一个")
        .with_align(enums::Align::Inside | enums::Align::Left);
    tip1_label.set_label_color(Color::Red);
    let mut txt = text::TextEditor::default().with_size(190, 290);
    txt.set_buffer(txt_buf.clone());
    txt.set_text_color(Color::DarkGreen);
    txt.set_scrollbar_align(Align::Right);

    let mut brow = group::Flex::default().row();
    {
        frame::Frame::default();
        let tip2_label = frame::Frame::default()
            .with_label("保存位置config.txt")
            .with_align(enums::Align::Inside | enums::Align::Left);
        let mut save = create_button("保存");
        save.set_callback(move |_| {
            let path = std::env::current_dir()
                .unwrap_or_default()
                .join("config.txt");
            match txt_buf.clone().save_file(path) {
                Ok(_) => {
                    unsafe {
                        CONFIG_WINDOW_TITLES.clear();
                    }
                    let content = txt_buf.text();
                    for line in content.lines() {
                        if !line.trim().is_empty() {
                            unsafe {
                                CONFIG_WINDOW_TITLES.push(line.to_string());
                            }
                        };
                    }
                    do_allwindow();
                }
                Err(_) => {
                    dialog::message_default("保存失败");
                }
            }
        });
        brow.fixed(&tip2_label, 120);
        brow.fixed(&save, 60);
        brow.end();
    }
    left_panel_layout.fixed(&tip1_label, 30);
    left_panel_layout.fixed(&txt, 300);
    left_panel_layout.fixed(&brow, 30);
    left_panel_layout.end();

    let spacer = frame::Frame::default();

    let mut right_panel_layout = group::Flex::default().column();
    right_panel(&mut right_panel_layout);
    right_panel_layout.end();

    frame::Frame::default();
    mp.fixed(&left_panel_layout, 300);
    mp.fixed(&spacer, 10);
    mp.fixed(&right_panel_layout, 300);
    frame::Frame::default();
    mp.end();
    frame::Frame::default();
    col.fixed(&mp, 500);

    col.end();
    // wind.resizable(&col);
    wind.set_color(enums::Color::from_rgb(250, 250, 250));
    wind.end();

    // wind.size_range(600, 400, 1024, 768);

    wind.show();
    helper::set_window_deny_capture(wind.raw_handle() as _, true);
    let image = PngImage::from_data(&helper::load_icon_to_png("IDI_1").unwrap()).unwrap();
    wind.set_icon(Some(image));
    wind.set_label(
        format!(
            "{} {}",
            wind.raw_handle() as u64,
            "  开源地址:github.com/pkptzx/AntiWindowSnap"
        )
        .as_str(),
    );

    do_allwindow();

    let hook = unsafe {
        SetWinEventHook(
            EVENT_OBJECT_CREATE,
            EVENT_OBJECT_NAMECHANGE,
            None,
            Some(win_event_hook_callback),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNTHREAD,
        )
    };
    assert!(!hook.is_invalid(), "Failed to install hook");

    wind.set_callback(move |_| {
        if fltk::app::event() == fltk::enums::Event::Close {
            dialog::message_title("想好了?");
            let choice = dialog::choice2_default(
                "退出是不会取消已经设置过防截屏的窗口状态",
                "取消",
                "臣退了",
                "开源地址:github.com/pkptzx/AntiWindowSnap",
            );
            if !choice.is_none() && choice.unwrap() == 1 {
                unsafe {
                    let _ = UnhookWinEvent(hook);
                }
                app::quit();
            } else if !choice.is_none() && choice.unwrap() == 2 {
                let mut cmd = std::process::Command::new("cmd");
                cmd.arg("/c")
                    .arg("start")
                    .raw_arg("\"\"")
                    // .raw_arg(wrap_in_quotes(app.into()))
                    .raw_arg(wrap_in_quotes("https://github.com/pkptzx/AntiWindowSnap"))
                    .creation_flags(0x08000000);
                cmd.status().unwrap();
            }
        }
    });
    app.run().unwrap();

    unsafe {
        let _ = UnhookWinEvent(hook);
    }
}
fn wrap_in_quotes<T: AsRef<OsStr>>(path: T) -> OsString {
    let mut result = OsString::from("\"");
    result.push(path);
    result.push("\"");

    result
}
unsafe extern "system" fn win_event_hook_callback(
    _hook_handle: HWINEVENTHOOK,
    _event_id: u32,
    _window_handle: HWND,
    _object_id: i32,
    _child_id: i32,
    _thread_id: u32,
    _timestamp: u32,
) {
    let hwnd = _window_handle.0 as u64;
    if _event_id == EVENT_OBJECT_CREATE
        && _object_id == OBJID_WINDOW.0
        && _child_id as u32 == CHILDID_SELF
    {
        if !is_root_window_by_cache(hwnd) {
            return;
        }

        // println!(
        //     "创建窗体事件: _hook_handle:{:?} _event_id:{} _window_handle:{:?} _object_id:{} _child_id:{} _thread_id:{} _timestamp:{}",
        //     _hook_handle, _event_id, _window_handle, _object_id, _child_id, _thread_id, _timestamp
        // );
        // println!("{:?}窗口名称: {}",_window_handle, get_window_title(hwnd));
        // 接受到创建窗体的事件
        if !WINDOW_CACHE.contains_key(&hwnd) {
            WINDOW_CACHE.insert(hwnd, (STATE::Processing, STATE::UnProcessed));
            std::thread::spawn(move || unsafe {
                let title = get_window_title(hwnd);
                if CONFIG_WINDOW_TITLES.contains(&title) {
                    if anti_window(hwnd, true) {
                        println!("************************已经设置窗口防截屏:{}", title);
                        set_tip(format!("已经设置窗口防截屏:{}", title));
                    } else {
                        println!("设置窗口防截屏失败:{}", title);
                        set_tip(format!("设置窗口防截屏失败:{}", title));
                    }
                }
                let mut val = WINDOW_CACHE.get_mut(&hwnd).unwrap();
                val.0 = STATE::Processed;
            });
        }
    } else if _event_id == EVENT_OBJECT_NAMECHANGE
        && _object_id == OBJID_WINDOW.0
        && _child_id as u32 == CHILDID_SELF
    {
        if !is_root_window_by_cache(hwnd) {
            return;
        }
        // println!(
        //     "对象名称改变事件: _hook_handle:{:?} _event_id:{} _window_handle:{:?} _object_id:{} _child_id:{} _thread_id:{} _timestamp:{}",
        //     _hook_handle, _event_id, _window_handle, _object_id, _child_id, _thread_id, _timestamp
        // );
        // println!("{:?}窗口名称: {}",_window_handle, get_window_title(hwnd));
        // 接受到名称改变的事件
        if !WINDOW_CACHE.contains_key(&hwnd) {
            WINDOW_CACHE.insert(hwnd, (STATE::UnProcessed, STATE::Processing));
            std::thread::spawn(move || unsafe {
                let title = get_window_title(hwnd);
                let mut val = WINDOW_CACHE.get_mut(&hwnd).unwrap();
                if CONFIG_WINDOW_TITLES.contains(&title) {
                    if anti_window(hwnd, true) {
                        println!("************************已经设置窗口防截屏:{}", title);
                        set_tip(format!("已经设置窗口防截屏:{}", title));
                    } else {
                        println!("设置窗口防截屏失败:{}", title);
                    }
                    val.1 = STATE::Completed;
                } else {
                    val.1 = STATE::Processed;
                }
            });
        } else {
            let val = WINDOW_CACHE.get_mut(&hwnd).unwrap();
            if val.1 == STATE::UnProcessed || val.1 == STATE::Processed {
                std::thread::spawn(move || unsafe {
                    let title = get_window_title(hwnd);
                    let mut val = WINDOW_CACHE.get_mut(&hwnd).unwrap();
                    if CONFIG_WINDOW_TITLES.contains(&title) {
                        if anti_window(hwnd, true) {
                            println!("************************已经设置窗口防截屏:{}", title);
                            set_tip(format!("已经设置窗口防截屏:{}", title));
                        } else {
                            println!("设置窗口防截屏失败:{}", title);
                            set_tip(format!("设置窗口防截屏失败:{}", title));
                        }
                        val.1 = STATE::Completed;
                    } else {
                        val.1 = STATE::Processed;
                    }
                });
            }
        }
    }
    // CHILDID_SELF;
}

fn get_window_title(hwnd: u64) -> String {
    unsafe {
        let title: &mut [u16] = &mut [0; 255];
        let size = GetWindowTextW(HWND(hwnd as *mut c_void), title);
        let title = &title[0..size as usize];
        String::from_utf16_lossy(title)
    }
}

fn is_root_window(hwnd: u64) -> bool {
    unsafe {
        if GetParent(HWND(hwnd as *mut c_void)).is_err() {
            // 没有父窗口，可能是顶级窗口，进一步检查样式
            let style = GetWindowLongPtrW(HWND(hwnd as *mut c_void), GWL_STYLE);
            let style = WINDOW_STYLE(style as u32);
            // 如果没有WS_CHILD标志，则是顶级窗口
            return (style & WS_CHILD).0 == 0;
        }
        return false;
    }
}

fn is_root_window_by_cache(hwnd: u64) -> bool {
    unsafe {
        let root_window = ROOT_WINDOW_CACHE.get(&hwnd);
        if let Some(root_window) = root_window {
            root_window.to_owned()
        } else {
            let root_window = is_root_window(hwnd);
            ROOT_WINDOW_CACHE.insert(hwnd, root_window);
            root_window
        }
    }
}

unsafe extern "system" fn enum_window_callback(
    hwnd: HWND,
    _lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    // println!("hwnd:{:?} title:{} lparam: {}", hwnd,get_window_title(hwnd.0 as u64),lparam.0);
    let hwnd = hwnd.0 as u64;
    WINDOW_CACHE.insert(hwnd, (STATE::Processing, STATE::UnProcessed));
    std::thread::spawn(move || unsafe {
        let title = get_window_title(hwnd);
        if CONFIG_WINDOW_TITLES.contains(&title) {
            if anti_window(hwnd, true) {
                println!("************************已经设置窗口防截屏:{}", title);
                set_tip(format!("已经设置窗口防截屏:{}", title));
            } else {
                println!("设置窗口防截屏失败:{}", title);
                set_tip(format!("设置窗口防截屏失败:{}", title));
            }
        }
        let mut val = WINDOW_CACHE.get_mut(&hwnd).unwrap();
        val.0 = STATE::Processed;
    });
    BOOL(1)
}
fn do_allwindow() {
    unsafe {
        WINDOW_CACHE.clear();
        let lparam = windows::Win32::Foundation::LPARAM(8888);
        EnumWindows(Some(enum_window_callback), lparam).unwrap();
    }
}
fn set_tip(txt: String) {
    let mut tip = unsafe { LABELS_CACHE.get_mut("tips").unwrap() };
    let tip_lab = tip.label();
    let lines: Vec<&str> = tip_lab.split("\n").collect();
    let last_line = lines.last().unwrap();
    let last_line = if last_line.is_empty() { "" } else { last_line };
    let tips = format!("{}\n{}", last_line, txt);
    tip.set_label(tips.as_str());
    tip.redraw_label();
    tip.parent().unwrap().redraw();
}
fn right_panel(parent: &mut group::Flex) {
    // frame::Frame::default();
    let mut sqq = group::Flex::default().row();
    {
        frame::Frame::default()
            .with_label("窗口拾取器:")
            .with_align(enums::Align::Inside | enums::Align::Right);

        let mut img = frame::Frame::default().with_size(42, 42);
        // img.set_frame(FrameType::EngravedBox);

        let image = PngImage::from_data(&helper::load_icon_to_png("IDI_1").unwrap()).unwrap();
        println!("image: {} {}", image.width(), image.height());
        // image.scale(42, 42, true, false);

        img.set_image(Some(image));
        img.handle({
            let mut last_hwnd = 0u64;
            move |me, event| match event {
                Event::Push => {
                    last_hwnd = 0;
                    let image =
                        PngImage::from_data(&helper::load_icon_to_png("IDI_2").unwrap()).unwrap();
                    // image.scale(42, 42, true, true);
                    me.set_image(Some(image));
                    me.redraw();
                    unsafe {
                        let h_module = GetModuleHandleW(None).unwrap();
                        let hcursor = LoadCursorW(h_module, w!("IDC_C_CURSOR")).unwrap();
                        // println!("hcursor: {:?}", hcursor);
                        SetCursor(hcursor);
                    }
                    true
                }
                Event::Released => {
                    if last_hwnd != 0 {
                        // highlight_border(HWND(last_hwnd as _),false);
                        invert_window(HWND(last_hwnd as _), false);
                    }
                    let image =
                        PngImage::from_data(&helper::load_icon_to_png("IDI_1").unwrap()).unwrap();
                    // image.scale(42, 42, true, true);
                    me.set_image(Some(image));
                    me.redraw();
                    true
                }
                Event::Drag => {
                    unsafe {
                        let mut point2 = POINT::default();
                        GetPhysicalCursorPos(&mut point2).unwrap();
                        // println!("GetPhysicalCursorPos: {:?}", point2);

                        let hwnd = WindowFromPhysicalPoint(point2);
                        // println!("hwnd: {:?}", hwnd);
                        if !hwnd.is_invalid() {
                            let window_info = helper::get_window_info(hwnd.0 as _);
                            // println!("hwnd: {:?} window_info: {:?}", hwnd, window_info);
                            // let hdc = GetWindowDC(hwnd);
                            // let oldRop2 = SetROP2(hdc, windows::Win32::Graphics::Gdi::R2_NOTXORPEN); // 返回0失败 R2_NOT R2_MASKPEN R2_NOTXORPEN
                            // let pen = CreatePen(PS_INSIDEFRAME, 3, COLORREF(0x000000FF));// red 0x000000FF
                            // let h_old_pen = SelectObject(hdc, pen);
                            // let h_old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
                            // let width = window_info.rect.right - window_info.rect.left;
                            // let height = window_info.rect.bottom - window_info.rect.top;
                            // Rectangle(hdc, 0, 0, width, height).as_bool();

                            // SetROP2(hdc, windows::Win32::Graphics::Gdi::R2_MODE(oldRop2));

                            // SelectObject(hdc, h_old_brush);
                            // SelectObject(hdc, h_old_pen);
                            // ReleaseDC(hwnd, hdc);
                            // DeleteObject(pen).as_bool();
                            if last_hwnd != hwnd.0 as _ {
                                if last_hwnd != 0 {
                                    // highlight_border(HWND(last_hwnd as _),false);
                                    invert_window(HWND(last_hwnd as _), false);
                                }
                                // highlight_border(hwnd,true);
                                invert_window(hwnd, false);
                            }

                            let group = me.parent().unwrap().parent().unwrap();
                            let mut win_title: Input = group
                                .child(1)
                                .unwrap()
                                .as_group()
                                .unwrap()
                                .child(1)
                                .unwrap()
                                .into_widget();
                            win_title.set_value(&window_info.text);

                            let mut win_hwnd: Input = group
                                .child(2)
                                .unwrap()
                                .as_group()
                                .unwrap()
                                .child(1)
                                .unwrap()
                                .into_widget();
                            win_hwnd.set_value(&window_info.hwnd.to_string());

                            let mut win_class_name: Input = group
                                .child(3)
                                .unwrap()
                                .as_group()
                                .unwrap()
                                .child(1)
                                .unwrap()
                                .into_widget();
                            win_class_name.set_value(&window_info.class_name);

                            let mut win_style: Input = group
                                .child(4)
                                .unwrap()
                                .as_group()
                                .unwrap()
                                .child(1)
                                .unwrap()
                                .into_widget();
                            win_style.set_value(&window_info.style.to_string());

                            let mut win_exstyle: Input = group
                                .child(5)
                                .unwrap()
                                .as_group()
                                .unwrap()
                                .child(1)
                                .unwrap()
                                .into_widget();
                            win_exstyle.set_value(&window_info.ex_style.to_string());

                            let mut win_parent_title: Input = group
                                .child(6)
                                .unwrap()
                                .as_group()
                                .unwrap()
                                .child(1)
                                .unwrap()
                                .into_widget();
                            win_parent_title.set_value(&window_info.parent_text);

                            let mut win_parent_hwnd: Input = group
                                .child(7)
                                .unwrap()
                                .as_group()
                                .unwrap()
                                .child(1)
                                .unwrap()
                                .into_widget();
                            win_parent_hwnd.set_value(&window_info.parent_hwnd.to_string());

                            let mut win_parent_class_name: Input = group
                                .child(8)
                                .unwrap()
                                .as_group()
                                .unwrap()
                                .child(1)
                                .unwrap()
                                .into_widget();
                            win_parent_class_name.set_value(&window_info.parent_class_name);

                            last_hwnd = hwnd.0 as _;
                        }
                    }
                    true
                }
                _ => false,
            }
        });
        sqq.fixed(&img, 43);
        sqq.end();
    }

    let mut urow = group::Flex::default().row();
    {
        frame::Frame::default()
            .with_label("窗口标题:")
            .with_align(enums::Align::Inside | enums::Align::Right);
        let mut inp_win_title = input::Input::default();
        inp_win_title.set_readonly(true);
        urow.fixed(&inp_win_title, 180);
        urow.end();
    }

    let mut prow = group::Flex::default().row();
    {
        frame::Frame::default()
            .with_label("窗口句柄:")
            .with_align(enums::Align::Inside | enums::Align::Right);
        let mut inp_win_hwnd = input::Input::default();
        inp_win_hwnd.set_readonly(true);
        prow.fixed(&inp_win_hwnd, 180);
        prow.end();
    }
    let mut row3 = group::Flex::default().row();
    {
        frame::Frame::default()
            .with_label("窗口类名:")
            .with_align(enums::Align::Inside | enums::Align::Right);
        let mut inp_win_class_name = input::Input::default();
        inp_win_class_name.set_readonly(true);

        row3.fixed(&inp_win_class_name, 180);
        row3.end();
    }
    let mut row4 = group::Flex::default().row();
    {
        frame::Frame::default()
            .with_label("窗口样式:")
            .with_align(enums::Align::Inside | enums::Align::Right);
        let mut inp_win_style = input::Input::default();
        inp_win_style.set_readonly(true);

        row4.fixed(&inp_win_style, 180);
        row4.end();
    }
    let mut row5 = group::Flex::default().row();
    {
        frame::Frame::default()
            .with_label("扩展样式:")
            .with_align(enums::Align::Inside | enums::Align::Right);
        let mut inp_win_exstyle = input::Input::default();
        inp_win_exstyle.set_readonly(true);

        row5.fixed(&inp_win_exstyle, 180);
        row5.end();
    }
    let mut row6 = group::Flex::default().row();
    {
        frame::Frame::default()
            .with_label("父窗口标题:")
            .with_align(enums::Align::Inside | enums::Align::Right);
        let mut inp_parent_win_title = input::Input::default();
        inp_parent_win_title.set_readonly(true);

        row6.fixed(&inp_parent_win_title, 180);
        row6.end();
    }
    let mut row7 = group::Flex::default().row();
    {
        frame::Frame::default()
            .with_label("父窗口句柄:")
            .with_align(enums::Align::Inside | enums::Align::Right);
        let mut inp_parent_win_hwnd = input::Input::default();
        inp_parent_win_hwnd.set_readonly(true);

        row7.fixed(&inp_parent_win_hwnd, 180);
        row7.end();
    }
    let mut row8 = group::Flex::default().row();
    {
        frame::Frame::default()
            .with_label("父窗口类名:")
            .with_align(enums::Align::Inside | enums::Align::Right);

        let mut inp_parent_win_class_name = input::Input::default();
        inp_parent_win_class_name.set_readonly(true);

        row8.fixed(&inp_parent_win_class_name, 180);
        row8.end();
    }
    let mut row9 = group::Flex::default().row();
    {
        let mut tip = frame::Frame::default()
            .with_label("https://github.com/pkptzx/AntiWindowSnap")
            .with_align(enums::Align::Inside | enums::Align::Left);
        tip.set_label_color(Color::Blue);
        row9.fixed(&tip, 180);
        unsafe {
            LABELS_CACHE.insert("tips".to_string(), tip);
        }
        row9.end();
    }

    let pad = frame::Frame::default();

    frame::Frame::default();

    parent.fixed(&sqq, 43);
    parent.fixed(&urow, 30);
    parent.fixed(&prow, 30);
    parent.fixed(&row3, 30);
    parent.fixed(&row4, 30);
    parent.fixed(&row5, 30);
    parent.fixed(&row6, 30);
    parent.fixed(&row7, 30);
    parent.fixed(&row8, 30);
    parent.fixed(&row9, 60);

    parent.fixed(&pad, 1); //空
                           // parent.fixed(&brow, 30); //按钮
                           // parent.fixed(&b, 30); //底部
}

fn create_button(caption: &str) -> button::Button {
    let mut btn = button::Button::default().with_label(caption);
    btn.set_color(enums::Color::from_rgb(225, 225, 225));
    btn
}

//from https://github.com/zodiacon/WinSpy/blob/master/WinSpy/WindowHelper.cpp#L101
pub fn highlight_border(hwnd: HWND, highlight: bool) {
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };

    unsafe {
        GetWindowRect(hwnd, &mut rc).unwrap();
        // rc.OffsetRect(-rc.left, -rc.top); // 偏移
        OffsetRect(&mut rc, -rc.left, -rc.top).unwrap();
        // rc.InflateRect(2, 2); //增大 CRect 的宽度和高度。
        InflateRect(&mut rc, 2, 2).unwrap();
    }
    let rgn1 = unsafe { CreateRectRgnIndirect(&rc) };
    unsafe {
        // rc.DeflateRect(5, 5); //减小 CRect 的宽度和高度。
        InflateRect(&mut rc, -5, -5).unwrap();
    }
    let rgn2 = unsafe { CreateRectRgnIndirect(&rc) };

    let rgn = unsafe { CreateRectRgn(0, 0, 1, 1) };
    unsafe { CombineRgn(rgn, rgn1, rgn2, RGN_DIFF) }; // RGN_DIFF = 2 RGN_OR(2) RGN_DIFF(4)

    if !highlight {
        unsafe {
            RedrawWindow(
                hwnd,
                None,
                rgn,
                RDW_INTERNALPAINT | RDW_INVALIDATE | RDW_UPDATENOW | RDW_FRAME,
            )
            .as_bool()
        };
        return;
    }

    let dc = unsafe { GetDC(hwnd) };
    let brush = unsafe { CreateSolidBrush(rgb(255, 0, 0)) };
    let _result = unsafe { FillRgn(dc, rgn, brush).as_bool() };
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    let color: u32 = (b as u32) << 16 | (g as u32) << 8 | r as u32;
    COLORREF(color)
}

// from https://github.com/strobejb/winspy/blob/master/src/FindTool.c#L91
fn invert_window(hwnd: HWND, f_show_hidden: bool) {
    let mut hwnd = hwnd;
    let mut rect = RECT::default();
    let mut rect2 = RECT::default();
    let mut rectc = RECT::default();

    let mut border = 3; //INVERT_BORDER

    if hwnd.is_invalid() {
        return;
    }

    //window rectangle (screen coords)
    unsafe {
        GetWindowRect(hwnd, &mut rect).unwrap();

        //client rectangle (screen coords)
        GetClientRect(hwnd, &mut rectc).unwrap();
        let mut point: POINT = POINT {
            x: rectc.left,
            y: rectc.top,
        };
        ClientToScreen(hwnd, &mut point).as_bool();
        rectc.left = point.x;
        rectc.top = point.y;

        let mut point: POINT = POINT {
            x: rectc.right,
            y: rectc.bottom,
        };
        ClientToScreen(hwnd, &mut point).as_bool();
        rectc.right = point.x;
        rectc.bottom = point.y;
        //MapWindowPoints(hwnd, 0, (POINT *)&rectc, 2);

        let x1 = rect.left;
        let y1 = rect.top;
        OffsetRect(&mut rect, -x1, -y1).as_bool();
        OffsetRect(&mut rectc, -x1, -y1).as_bool();

        if rect.bottom - border * 2 < 0 {
            border = 1;
        }

        if rect.right - border * 2 < 0 {
            border = 1;
        }

        if f_show_hidden == true {
            hwnd.0 = 0 as _;
        }

        let hdc = GetWindowDC(hwnd);

        if hdc.is_invalid() {
            return;
        }

        //top edge
        //border = rectc.top-rect.top;
        SetRect(&mut rect2, 0, 0, rect.right, border).as_bool();
        if f_show_hidden == true {
            OffsetRect(&mut rect2, x1, y1).as_bool();
        }
        InvertRect(hdc, &rect2).as_bool();

        //left edge
        //border = rectc.left-rect.left;
        SetRect(&mut rect2, 0, border, border, rect.bottom).as_bool();
        if f_show_hidden == true {
            OffsetRect(&mut rect2, x1, y1).as_bool();
        }
        InvertRect(hdc, &rect2).as_bool();

        //right edge
        //border = rect.right-rectc.right;
        SetRect(
            &mut rect2,
            border,
            rect.bottom - border,
            rect.right,
            rect.bottom,
        )
        .as_bool();
        if f_show_hidden == true {
            OffsetRect(&mut rect2, x1, y1).as_bool();
        }
        InvertRect(hdc, &rect2).as_bool();

        //bottom edge
        //border = rect.bottom-rectc.bottom;
        SetRect(
            &mut rect2,
            rect.right - border,
            border,
            rect.right,
            rect.bottom - border,
        )
        .as_bool();
        if f_show_hidden == true {
            OffsetRect(&mut rect2, x1, y1).as_bool();
        }
        InvertRect(hdc, &rect2).as_bool();

        ReleaseDC(hwnd, hdc);
    };
}
