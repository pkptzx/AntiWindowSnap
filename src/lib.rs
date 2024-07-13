use std::{
    ffi::c_void,
    mem::{size_of, transmute},
};

use windows::{
    core::s,
    Win32::{
        Foundation::{CloseHandle, GetLastError, BOOL, FARPROC, HANDLE, HMODULE, HWND},
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
                VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_DECOMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READWRITE
            },
            Threading::{
                CreateRemoteThread, GetExitCodeThread, IsWow64Process, OpenProcess,
                WaitForSingleObject, INFINITE, LPTHREAD_START_ROUTINE, PROCESS_ALL_ACCESS,
            },
        },
        UI::WindowsAndMessaging::{GetWindowThreadProcessId, WDA_EXCLUDEFROMCAPTURE, WDA_NONE},
    },
};

const CODE_GET_FUNC_ADDRESS_X86: [u8; 124] = [
    0x64u8, 0xA1, 0x30, 0x00, 0x00, 0x00, /* mov eax,dword ptr fs:[00000030h] */
    0x53, /* push ebx */
    0x56, /* push esi */
    0x57, /* push edi */
    0x8B, 0x40, 0x0C, /* mov eax,dword ptr [eax+0Ch] */
    0x8B, 0x40, 0x14, /* mov eax,dword ptr [eax+14h] */
    0x8B, 0x00, /* mov eax,dword ptr [eax] */
    0x8B, 0x00, /* mov eax,dword ptr [eax] */
    0x8B, 0x78, 0x10, /* mov edi,dword ptr [eax+10h] */
    0x8B, 0x47, 0x3C, /* mov eax,dword ptr [edi+3Ch] */
    0x8B, 0x44, 0x38, 0x78, /* mov eax,dword ptr [eax+edi+78h] */
    0x8B, 0x4C, 0x38, 0x20, /* mov ecx,dword ptr [eax+edi+20h] */
    0x8B, 0x74, 0x38, 0x24, /* mov esi,dword ptr [eax+edi+24h] */
    0x03, 0xCF, /* add ecx,edi */
    0x8B, 0x5C, 0x38, 0x1C, /* mov ebx,dword ptr [eax+edi+1Ch] */
    0x03, 0xF7, /* add esi,edi */
    0x03, 0xDF, /* add ebx,edi */
    0x33, 0xD2, /* xor edx,edx */
    /* label1: */
    0x8B, 0x01, /* mov eax,dword ptr [ecx] */
    0x81, 0x3C, 0x38, 0x47, 0x65, 0x74, 0x50, /* cmp dword ptr [eax+edi],50746547h */
    0x75, 0x14, /* jne <label2> */
    0x81, 0x7C, 0x38, 0x04, 0x72, 0x6F, 0x63, 0x41, /* cmp dword ptr [eax+edi+4],41636F72h */
    0x75, 0x0A, /* jne <label2> */
    0x81, 0x7C, 0x38, 0x08, 0x64, 0x64, 0x72, 0x65, /* cmp dword ptr [eax+edi+8],65726464h */
    0x74, 0x06, /* je <label3> */
    /* label2: */
    0x83, 0xC1, 0x04, /* add ecx,4 */
    0x42, /* inc edx */
    0xEB, 0xDB, /* jmp <label1> */
    /* label3: */
    0x0F, 0xB7, 0x04, 0x56, /* movzx eax,word ptr [esi+edx*2] */
    0x68, 0x00, 0x00, 0x00, 0x00, /* push offset string "GetModuleHandleA" */
    0x57, /* push edi */
    0x8B, 0x34, 0x83, /* mov esi,dword ptr [ebx+eax*4] */
    0x03, 0xF7, /* add esi,edi */
    0xFF, 0xD6, /* call esi */
    0x68, 0x00, 0x00, 0x00, 0x00, /* push offset string "SetWindowDisplayAffinity" */
    0x68, 0x00, 0x00, 0x00, 0x00, /* push offset string "user32" */
    0xFF, 0xD0, /* call eax */
    0x50, /* push eax */
    0xFF, 0xD6, /* call esi */
    0x5F, /* pop edi */
    0x5E, /* pop esi */
    0x5B, /* pop ebx */
    0xC2, 0x04, 0x00, /* ret 4 */
];

const CODE_GET_FUNC_ADDRESS_X86_DATA_GMH: [u8; 17] = *b"GetModuleHandleA\0";
const CODE_GET_FUNC_ADDRESS_X86_DATA_USER32: [u8; 7] = *b"user32\0";
const CODE_GET_FUNC_ADDRESS_X86_DATA_SWDF: [u8; 25] = *b"SetWindowDisplayAffinity\0";

const CODE_GET_FUNC_ADDRESS_X86_SIZE: usize = CODE_GET_FUNC_ADDRESS_X86.len();
const CODE_GET_FUNC_ADDRESS_X86_DATA_GMH_SIZE: usize = CODE_GET_FUNC_ADDRESS_X86_DATA_GMH.len();
const CODE_GET_FUNC_ADDRESS_X86_DATA_USER32_SIZE: usize =
    CODE_GET_FUNC_ADDRESS_X86_DATA_USER32.len();
const CODE_GET_FUNC_ADDRESS_X86_DATA_SWDF_SIZE: usize = CODE_GET_FUNC_ADDRESS_X86_DATA_SWDF.len();

const CODE_GET_FUNC_ADDRESS_X86_SIZE_ALL: usize = CODE_GET_FUNC_ADDRESS_X86_SIZE
    + CODE_GET_FUNC_ADDRESS_X86_DATA_GMH_SIZE
    + CODE_GET_FUNC_ADDRESS_X86_DATA_USER32_SIZE
    + CODE_GET_FUNC_ADDRESS_X86_DATA_SWDF_SIZE;

const CODE_X86: [u8; 11] = [
    0x58u8, /* pop eax */
    0x59,   /* pop ecx */
    0x6A, 0x00, /* push <affinity> */
    0x51, /* push ecx */
    0x50, /* push eax */
    0xE9, 0x00, 0x00, 0x00, 0x00,
];

const CODE_X86_SIZE: usize = CODE_X86.len();

pub fn anti_window(hwnd: u64, hide: bool) -> bool {
    let mask: u32 = if hide {
        WDA_EXCLUDEFROMCAPTURE.0
    } else {
        WDA_NONE.0
    }; // WDA_NONE WDA_EXCLUDEFROMCAPTURE
    unsafe {
        let mut pid = 0u32;
        let pid_ptr = Some(&mut pid as *mut u32);

        let _tid = GetWindowThreadProcessId(HWND(hwnd as *mut c_void), pid_ptr);

        let process_handle = OpenProcess(PROCESS_ALL_ACCESS, false, pid).unwrap();

        let is_x86_process = &mut BOOL::default();
        IsWow64Process(process_handle, is_x86_process).unwrap();
        let is_x86_process = is_x86_process.as_bool();

        if !is_x86_process {
            let mut shellcode_x64: Vec<u8> = "48 89 4C 24 08 48 89 54 24 10 4C 89 44 24 18 4C 89 4C 24 20 48 83 EC 38 48 B9 ED FE AD DE ED FE 00 00 48 C7 C2 AD DE 00 00 48 B8 AD DE ED FE AD DE 00 00 FF D0 48 83 C4 38 48 8B 4C 24 08 48 8B 54 24 10 4C 8B 44 24 18 4C 8B 4C 24 20 C3".split_whitespace().map(|v|{
                u8::from_str_radix(v, 16).unwrap()
            }).collect();

            let mod_info = get_mod_info(pid, "User32.dll").unwrap();

            let mod_handle = mod_info.hModule.0;
            let fn_set_window_display_affinity_address = GetProcAddress(
                HMODULE(mod_handle as *mut std::ffi::c_void),
                s!("SetWindowDisplayAffinity"),
            )
            .unwrap();

            let address: u64 = fn_set_window_display_affinity_address as u64;

            let hwnd_bytes = hwnd.to_ne_bytes();
            let mask_bytes = mask.to_ne_bytes();
            let address_bytes = address.to_ne_bytes();
            //26 37 43
            shellcode_x64.splice(26..26 + hwnd_bytes.len(), hwnd_bytes.iter().cloned());
            shellcode_x64.splice(37..37 + mask_bytes.len(), mask_bytes.iter().cloned());
            shellcode_x64.splice(43..43 + address_bytes.len(), address_bytes.iter().cloned());
            // println!("pat_bytes: {:02X?}", shellcode);

            let address = VirtualAllocEx(
                process_handle,
                None,
                shellcode_x64.len(),
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            );
            // println!("address: {:?}", address);

            WriteProcessMemory(
                process_handle,
                address as *mut c_void,
                shellcode_x64.as_ptr() as *const c_void,
                shellcode_x64.len(),
                None,
            )
            .unwrap();
            let addr = transmute(address);
            let address = transmute::<FARPROC, LPTHREAD_START_ROUTINE>(Some(addr));
            let mut tid = 0u32;
            let tid_ptr = &mut tid as *mut u32;

            let t_handle =
                CreateRemoteThread(process_handle, None, 0, address, None, 0, Some(tid_ptr))
                    .unwrap();
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
        } else {
            let func_addr = get_func_address_x86(process_handle);
            let code_address = VirtualAllocEx(
                process_handle,
                None,
                CODE_X86_SIZE,
                MEM_COMMIT,
                PAGE_EXECUTE_READWRITE,
            );
            let code = build_x86_code(
                func_addr as _,
                mask as u8,
                code_address as _,
            );
            let retval = write_and_execute_code_wait(
                process_handle,
                code_address,
                code.as_ptr() as *const std::os::raw::c_void,
                CODE_X86_SIZE,
                Some(hwnd as *mut c_void),
                INFINITE,
            );
            println!("成功?: {:?}", retval);
            VirtualFreeEx(process_handle, code_address, CODE_X86_SIZE, MEM_DECOMMIT).unwrap();
        }
        CloseHandle(process_handle).unwrap();
    }
    return true;
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

fn get_func_address_x86(h_process: HANDLE) -> u32 {
    unsafe {
        let mut retval = 0u32;
        let code_address = VirtualAllocEx(
            h_process,
            None,
            CODE_GET_FUNC_ADDRESS_X86_SIZE_ALL,
            MEM_COMMIT,
            PAGE_EXECUTE_READWRITE,
        );
        println!("code_address: {:?}", code_address);
        if code_address as u32 != 0 {
            let code = build_x86_get_func_code(code_address as _);
            let exit_code = &mut 0u32;
            // DWORD exit_code;
            if write_and_execute_code_wait_return(
                h_process,
                code_address,
                code.as_ptr() as *const std::os::raw::c_void,
                CODE_GET_FUNC_ADDRESS_X86_SIZE_ALL,
                exit_code,
                None,
                INFINITE,
            ) {
                retval = *exit_code;
                VirtualFreeEx(
                    h_process,
                    code_address,
                    CODE_GET_FUNC_ADDRESS_X86_SIZE_ALL,
                    MEM_DECOMMIT,
                )
                .unwrap();
            }
        }
        return retval;
    }
}
fn build_x86_code(func_addr: usize, affinity: u8, base_address: usize) -> [u8; 11] {
    // affinity 是 WWDA_NONE / DA_MONITOR 虽然定义是u32但实际上只是0,1,17.u8就够了
    let mut code = [0u8; 11];
    code.copy_from_slice(&CODE_X86);
    code[3] = affinity;
    code[7..11].copy_from_slice(&((func_addr - (base_address + 6) - 5) as u32).to_ne_bytes());
    code
}
fn build_x86_get_func_code(base_address: usize) -> [u8; CODE_GET_FUNC_ADDRESS_X86_SIZE_ALL] {
    let mut code = [0; CODE_GET_FUNC_ADDRESS_X86_SIZE_ALL];
    let mut start = 0usize;
    let mut end = CODE_GET_FUNC_ADDRESS_X86_SIZE;
    code[start..end].copy_from_slice(&CODE_GET_FUNC_ADDRESS_X86);
    start = end;
    end += CODE_GET_FUNC_ADDRESS_X86_DATA_GMH_SIZE;
    code[start..end].copy_from_slice(&CODE_GET_FUNC_ADDRESS_X86_DATA_GMH);
    start = end;
    end += CODE_GET_FUNC_ADDRESS_X86_DATA_USER32_SIZE;
    code[start..end].copy_from_slice(&CODE_GET_FUNC_ADDRESS_X86_DATA_USER32);
    start = end;
    end += CODE_GET_FUNC_ADDRESS_X86_DATA_SWDF_SIZE;
    code[start..end].copy_from_slice(&CODE_GET_FUNC_ADDRESS_X86_DATA_SWDF);

    code[91..95]
        .copy_from_slice(&((base_address + CODE_GET_FUNC_ADDRESS_X86_SIZE) as u32).to_ne_bytes());
    code[104..108].copy_from_slice(
        &((base_address
            + CODE_GET_FUNC_ADDRESS_X86_SIZE
            + CODE_GET_FUNC_ADDRESS_X86_DATA_GMH_SIZE
            + CODE_GET_FUNC_ADDRESS_X86_DATA_USER32_SIZE) as u32)
            .to_ne_bytes(),
    );
    code[109..113].copy_from_slice(
        &((base_address + CODE_GET_FUNC_ADDRESS_X86_SIZE + CODE_GET_FUNC_ADDRESS_X86_DATA_GMH_SIZE)
            as u32)
            .to_ne_bytes(),
    );
    println!("BuildGetFuncCode:\n{:02X?}", code);
    code
}

fn write_and_execute_code(
    h_process: HANDLE,
    code_address: *const c_void,
    code: *const core::ffi::c_void,
    code_size: usize,
    parameter: Option<*const c_void>,
) -> HANDLE {
    unsafe {
        WriteProcessMemory(h_process, code_address, code, code_size, None).unwrap();
        // let addr = transmute(code_address);
        let address = transmute::<*const c_void, LPTHREAD_START_ROUTINE>(code_address);
        return CreateRemoteThread(h_process, None, 0, address, parameter, 0, None).unwrap();
    }
}

fn write_and_execute_code_wait(
    h_process: HANDLE,
    code_address: *const c_void,
    code: *const c_void,
    code_size: usize,
    parameter: Option<*const c_void>,
    timeout: u32,
) -> bool {
    unsafe {
        let h_thread = write_and_execute_code(h_process, code_address, code, code_size, parameter);
        if !h_thread.is_invalid() {
            WaitForSingleObject(h_thread, timeout); //INFINITE
            CloseHandle(h_thread).unwrap();
            return true;
        }
        return false;
    }
}

fn write_and_execute_code_wait_return(
    h_process: HANDLE,
    code_address: *const c_void,
    code: *const c_void,
    code_size: usize,
    exit_code: *mut u32,
    parameter: Option<*const c_void>,
    timeout: u32,
) -> bool {
    unsafe {
        let h_thread = write_and_execute_code(h_process, code_address, code, code_size, parameter);
        if !h_thread.is_invalid() {
            WaitForSingleObject(h_thread, timeout);
            GetExitCodeThread(h_thread, exit_code).unwrap();
            CloseHandle(h_thread).unwrap();
            return true;
        }
        return false;
    }
}
