use std::{ffi::c_void, fs::File, io::{BufRead, BufReader}};

use anti_window_snap::anti_window;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use windows::{
    core::w,
    Win32::{
        Foundation::{BOOL, HWND},
        UI::{
            Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
            WindowsAndMessaging::{
                DispatchMessageW, EnumWindows, GetMessageW, GetParent, GetWindowLongPtrW, GetWindowTextW, TranslateMessage, CHILDID_SELF, EVENT_OBJECT_CREATE, EVENT_OBJECT_NAMECHANGE, GWL_STYLE, MSG, OBJID_WINDOW, WINDOW_STYLE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNTHREAD, WS_CHILD
            },
        },
    },
};
#[derive(Debug, PartialEq, Eq)]
enum STATE {
    UnProcessed,
    Processing,
    Processed,
    Completed,
}

// 内容: key: 窗口句柄, value: (创建时是否处理, 名称改变时是否处理) 处理状态:None未处理,false在处理,true已处理
static mut WINDOW_CACHE: Lazy<DashMap<u64, (STATE, STATE)>> = Lazy::new(|| initialize_map());
static mut ROOT_WINDOW_CACHE: Lazy<DashMap<u64, bool>> = Lazy::new(|| initialize_root_window_map());
static mut CONFIG_WINDOW_TITLES: Vec<String> = Vec::new();

fn initialize_map() -> DashMap<u64, (STATE, STATE)> {
    DashMap::new()
}
fn initialize_root_window_map() -> DashMap<u64, bool> {
    DashMap::new()
}

fn main() {

    unsafe { windows::Win32::System::Console::SetConsoleTitleW(w!("防截屏工具")).unwrap(); };

    let path = std::env::current_dir().unwrap_or_default().join("config.txt");

    if path.exists() {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
    
        for line_result in reader.lines() {
            let line = line_result.unwrap();
            if !line.trim().is_empty() {          
                unsafe {
                    println!("已加载配置:{}", line);
                    CONFIG_WINDOW_TITLES.push(line);
                }
            };
            
        }        
    }else {
        unsafe {
            println!("您可以在当前目录下创建配置文件: config.txt");
            println!("然后按照一行一个窗口标题来添加配置项");
            CONFIG_WINDOW_TITLES.push("无标题 - 记事本".to_string());
        }
    }
    

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
    // Make sure the hook is installed; a real application would want to do more
    // elaborate error handling
    assert!(!hook.is_invalid(), "Failed to install hook");

    // Have the system spin up a message loop (and get a convenient way to exit
    // the application for free)
    // let _ = unsafe { MessageBoxW(None, w!("点击'确定'退出程序"), w!("运行中"), MB_OK) };

    let mut msg: MSG = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool()
        {
            TranslateMessage(&msg).as_bool();
            DispatchMessageW(&msg);
        }
    }


    // let mut buff = String::new();
    // std::io::stdin().read_line(&mut buff).unwrap();

    unsafe {
        let _ = UnhookWinEvent(hook);
    }
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
                    anti_window(hwnd,true);
                    println!("************************已经设置窗口防截屏:{}", title);
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
                    anti_window(hwnd,true);
                    println!("************************已经设置窗口防截屏:{}", title);
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
                        anti_window(hwnd,true);
                        println!("************************已经设置窗口防截屏:{}", title);
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
            anti_window(hwnd,true);
            println!("************************已经设置窗口防截屏:{}", title);
        }
        let mut val = WINDOW_CACHE.get_mut(&hwnd).unwrap();
        val.0 = STATE::Processed;
    });
    BOOL(1)
}
fn do_allwindow() {
    unsafe {
        let lparam = windows::Win32::Foundation::LPARAM(8888);
        EnumWindows(Some(enum_window_callback), lparam).unwrap();
    }
}
