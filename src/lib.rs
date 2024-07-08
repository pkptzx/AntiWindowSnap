use std::{
    ffi::c_void,
    mem::{size_of, transmute},
};

use windows::{
    core::s,
    Win32::{
        Foundation::{CloseHandle, GetLastError, FARPROC, HMODULE, HWND},
        System::{
            Diagnostics::{
                Debug::WriteProcessMemory,
                ToolHelp::{
                    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W,
                    TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
                },
            },
            LibraryLoader::GetProcAddress,
            Memory::{
                VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            },
            Threading::{
                CreateRemoteThread, OpenProcess, WaitForSingleObject, INFINITE,
                LPTHREAD_START_ROUTINE, PROCESS_ALL_ACCESS,
            },
        },
        UI::WindowsAndMessaging::{
            GetWindowThreadProcessId, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
        },
    },
};

pub fn anti_window(hwnd: u64, hide: bool) -> bool {
    unsafe {
        let mut shellcode: Vec<u8> = "48 89 4C 24 08 48 89 54 24 10 4C 89 44 24 18 4C 89 4C 24 20 48 83 EC 38 48 B9 ED FE AD DE ED FE 00 00 48 C7 C2 AD DE 00 00 48 B8 AD DE ED FE AD DE 00 00 FF D0 48 83 C4 38 48 8B 4C 24 08 48 8B 54 24 10 4C 8B 44 24 18 4C 8B 4C 24 20 C3".split_whitespace().map(|v|{
        u8::from_str_radix(v, 16).unwrap()
    }).collect();

        let mut pid = 0u32;
        let pid_ptr = Some(&mut pid as *mut u32);

        let _tid = GetWindowThreadProcessId(HWND(hwnd as *mut c_void), pid_ptr);

        let mod_info = get_mod_info(pid, "User32.dll").unwrap();

        let mod_handle = mod_info.hModule.0;
        let fn_set_window_display_affinity_address = GetProcAddress(
            HMODULE(mod_handle as *mut std::ffi::c_void),
            s!("SetWindowDisplayAffinity"),
        )
        .unwrap();

        let mask: u32 = if hide {
            WDA_EXCLUDEFROMCAPTURE.0
        } else {
            WDA_NONE.0
        }; // WDA_NONE WDA_EXCLUDEFROMCAPTURE
        let address: u64 = fn_set_window_display_affinity_address as u64;

        let hwnd_bytes = hwnd.to_ne_bytes();
        let mask_bytes = mask.to_ne_bytes();
        let address_bytes = address.to_ne_bytes();
        //26 37 43
        shellcode.splice(26..26 + hwnd_bytes.len(), hwnd_bytes.iter().cloned());
        shellcode.splice(37..37 + mask_bytes.len(), mask_bytes.iter().cloned());
        shellcode.splice(43..43 + address_bytes.len(), address_bytes.iter().cloned());
        // println!("pat_bytes: {:02X?}", shellcode);

        let process_handle = OpenProcess(PROCESS_ALL_ACCESS, false, pid).unwrap();
        let address = VirtualAllocEx(
            process_handle,
            None,
            shellcode.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        );
        // println!("address: {:?}", address);

        WriteProcessMemory(
            process_handle,
            address as *mut std::os::raw::c_void,
            shellcode.as_ptr() as *const std::os::raw::c_void,
            shellcode.len(),
            None,
        )
        .unwrap();
        let addr = transmute(address);
        let address = transmute::<FARPROC, LPTHREAD_START_ROUTINE>(Some(addr));
        let mut tid = 0u32;
        let tid_ptr = &mut tid as *mut u32;

        let t_handle =
            CreateRemoteThread(process_handle, None, 0, address, None, 0, Some(tid_ptr)).unwrap();
        let _last_error = GetLastError();
        // println!("last_error: {:?}", _last_error);

        // println!("t_handle: {:?}", t_handle);
        // println!("tid: {:?}", tid);
        WaitForSingleObject(t_handle, INFINITE);

        VirtualFreeEx(
            process_handle,
            addr as *mut std::os::raw::c_void,
            0,
            MEM_RELEASE,
        )
        .unwrap();
        CloseHandle(t_handle).unwrap();
        CloseHandle(process_handle).unwrap();
    }
    return false;
}
pub fn get_mod_info(
    pid: u32,
    mod_name: &str,
) -> Result<MODULEENTRY32W, Box<dyn std::error::Error + Send + Sync>> {
    unsafe {
        let snapshot_handle =
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)?;

        let mut mod_info = MODULEENTRY32W::default();
        mod_info.dwSize = size_of::<MODULEENTRY32W>() as u32;

        Module32FirstW(snapshot_handle, &mut mod_info)
            .inspect_err(|err| {
                println!("{:?}", err);
                CloseHandle(snapshot_handle).unwrap();
            })
            .unwrap();
        let mod_name_ = String::from_utf16(&mod_info.szModule)
            .unwrap()
            .trim_end_matches("\0")
            .to_string();

        if mod_name_.eq_ignore_ascii_case(mod_name) {
            CloseHandle(snapshot_handle)?;
            return Ok(mod_info.clone());
        }

        while Module32NextW(snapshot_handle, &mut mod_info).is_ok() {
            let mod_name_ = String::from_utf16(&mod_info.szModule)
                .unwrap()
                .trim_end_matches("\0")
                .to_string();

            if mod_name_.eq_ignore_ascii_case(mod_name) {
                CloseHandle(snapshot_handle)?;
                return Ok(mod_info.clone());
            }
        }

        CloseHandle(snapshot_handle)?;

        Err(format!("{} not found", mod_name).into())
    }
}
