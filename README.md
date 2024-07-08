<h1 align="center" style="border-bottom: none"> 
    AntiWindowSnap<a href="https://github.com/pkptzx/AntiWindowSnap/releases/latest">
      <img
        alt="Windows"
        src="https://img.shields.io/badge/-Windows-blue?style=flat-square&logo=windows&logoColor=white"
      />
    </a></br>
</h1>

<p align="center">
  <a href="https://github.com/pkptzx/AntiWindowSnap"><img src="https://img.shields.io/github/stars/pkptzx/AntiWindowSnap"></a> 
  <a href="https://github.com/pkptzx/AntiWindowSnap/releases/latest"><img src="https://img.shields.io/github/downloads/pkptzx/antiwindowsnap/total"></a> 
  <a href="https://github.com/pkptzx/AntiWindowSnap"><img src="https://img.shields.io/github/license/pkptzx/AntiWindowSnap"></a>
</p>

`AntiWindowSnap` Prevent screenshotting and screen recording for the window with the specified title.

## Usage
1. Create a config.txt file next to AntiWindowSnap.exe. 
2. In config.txt, list one window title per line. 
3. Double-click to run AntiWindowSnap.exe.

## Why AntiWindowSnap?
- No DLL injection, no administrator rights are required, and only code injection is used to implement anti-screenshot api calls
- It detects new windows and changes to window titles in real-time, and also prevents screenshotting of previously opened windows.

### Download Prebuilt Binaries 
[Release download](https://github.com/pkptzx/AntiWindowSnap/releases/latest)  



## Build from source
```shell
cargo build --release
```

# License
MIT
