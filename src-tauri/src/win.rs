//! Thin, safe wrappers over the Win32 APIs VELIORA needs: hidden child
//! processes, job objects, process listing, the service control manager,
//! elevation and shell execution.
#![allow(clippy::missing_safety_doc)]

use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

unsafe fn from_wide_ptr(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    while *p.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

/// A `Command` that never opens a console window.
pub fn hidden(program: impl AsRef<OsStr>) -> Command {
    let mut c = Command::new(program);
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

pub fn system32(exe: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("System32").join(exe)
}

/// Runs a system tool hidden and returns (exit code, combined output).
pub fn run_tool(exe: &str, args: &[&str]) -> (i32, String) {
    match hidden(system32(exe)).args(args).output() {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            (o.status.code().unwrap_or(-1), s)
        }
        Err(e) => (-1, e.to_string()),
    }
}

// ── elevation ─────────────────────────────────────────────────────────────

pub fn is_admin() -> bool {
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

/// Shell-executes `file` with an optional verb ("runas" asks for elevation).
/// Returns the Win32 error code on failure (1223 = the user declined UAC).
pub fn shell_execute(verb: Option<&str>, file: &str, params: Option<&str>, dir: Option<&Path>) -> Result<(), u32> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    const SW_SHOWNORMAL: i32 = 1;
    let verb_w = verb.map(wide);
    let file_w = wide(file);
    let params_w = params.map(wide);
    let dir_w = dir.map(|d| wide(d.as_os_str()));
    let r = unsafe {
        ShellExecuteW(
            null_mut(),
            verb_w.as_ref().map_or(null(), |v| v.as_ptr()),
            file_w.as_ptr(),
            params_w.as_ref().map_or(null(), |v| v.as_ptr()),
            dir_w.as_ref().map_or(null(), |v| v.as_ptr()),
            SW_SHOWNORMAL,
        )
    };
    if (r as isize) > 32 {
        Ok(())
    } else {
        Err(unsafe { GetLastError() })
    }
}

/// Starts a new elevated copy of this executable with the given arguments.
pub fn relaunch_elevated(args: &[String]) -> Result<(), u32> {
    let exe = std::env::current_exe().map_err(|_| 2u32)?;
    let params = args
        .iter()
        .map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() })
        .collect::<Vec<_>>()
        .join(" ");
    shell_execute(Some("runas"), &exe.to_string_lossy(), Some(&params), exe.parent())
}

// ── processes ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
}

pub fn processes() -> Vec<ProcInfo> {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    let mut out = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut e) != 0 {
            loop {
                let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                out.push(ProcInfo { pid: e.th32ProcessID, name: String::from_utf16_lossy(&e.szExeFile[..len]) });
                if Process32NextW(snap, &mut e) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    out
}

pub fn find_processes(name: &str) -> Vec<ProcInfo> {
    processes().into_iter().filter(|p| p.name.eq_ignore_ascii_case(name)).collect()
}

/// Full image path of a process (empty when it can't be queried).
pub fn process_path(pid: u32) -> Option<PathBuf> {
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(h);
        (ok != 0).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..size as usize])))
    }
}

/// Waits for another process to exit (used when VELIORA relaunches itself).
pub fn wait_pid(pid: u32, timeout_ms: u32) {
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};
    const SYNCHRONIZE: u32 = 0x0010_0000;
    unsafe {
        let h = OpenProcess(SYNCHRONIZE, 0, pid);
        if !h.is_null() {
            WaitForSingleObject(h, timeout_ms);
            CloseHandle(h);
        }
    }
}

pub fn kill_pid(pid: u32) -> bool {
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    unsafe {
        let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if h.is_null() {
            return false;
        }
        let ok = TerminateProcess(h, 1) != 0;
        CloseHandle(h);
        ok
    }
}

// ── job objects: children die with the job (and with VELIORA) ─────────────

pub struct Job(HANDLE);
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    pub fn new() -> Option<Job> {
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        unsafe {
            let h = CreateJobObjectW(null(), null());
            if h.is_null() {
                return None;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            Some(Job(h))
        }
    }

    pub fn assign(&self, child: &std::process::Child) -> bool {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as HANDLE) != 0 }
    }

    pub fn terminate(&self) {
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;
        unsafe {
            TerminateJobObject(self.0, 1);
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

// ── services ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub name: String,
    pub display: String,
    /// 1 stopped, 2 start pending, 3 stop pending, 4 running …
    pub state: u32,
    /// 2 auto, 3 manual, 4 disabled (0/1 for drivers)
    pub start_type: u32,
    pub binary: String,
}

impl ServiceInfo {
    pub fn running(&self) -> bool {
        self.state == 4 || self.state == 2
    }
}

const SC_MANAGER_CONNECT: u32 = 0x0001;
const SC_MANAGER_ENUMERATE_SERVICE: u32 = 0x0004;
const SERVICE_QUERY_CONFIG: u32 = 0x0001;
const SERVICE_QUERY_STATUS: u32 = 0x0004;
const SERVICE_STOP: u32 = 0x0020;
const DELETE: u32 = 0x0001_0000;

struct Sc(*mut c_void);
impl Drop for Sc {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::System::Services::CloseServiceHandle(self.0 as _);
        }
    }
}

fn scm(access: u32) -> Option<Sc> {
    let h = unsafe { windows_sys::Win32::System::Services::OpenSCManagerW(null(), null(), access) };
    (!h.is_null()).then(|| Sc(h as _))
}

fn open_service(mgr: &Sc, name: &str, access: u32) -> Option<Sc> {
    let w = wide(name);
    let h = unsafe { windows_sys::Win32::System::Services::OpenServiceW(mgr.0 as _, w.as_ptr(), access) };
    (!h.is_null()).then(|| Sc(h as _))
}

pub fn service(name: &str) -> Option<ServiceInfo> {
    use windows_sys::Win32::System::Services::{QueryServiceConfigW, QueryServiceStatus, QUERY_SERVICE_CONFIGW, SERVICE_STATUS};
    let mgr = scm(SC_MANAGER_CONNECT)?;
    let svc = open_service(&mgr, name, SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG)?;
    unsafe {
        let mut st: SERVICE_STATUS = std::mem::zeroed();
        QueryServiceStatus(svc.0 as _, &mut st);
        let mut buf = vec![0u8; 8192];
        let mut needed = 0u32;
        let (start_type, binary) =
            if QueryServiceConfigW(svc.0 as _, buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW, buf.len() as u32, &mut needed) != 0 {
                let cfg = &*(buf.as_ptr() as *const QUERY_SERVICE_CONFIGW);
                (cfg.dwStartType, from_wide_ptr(cfg.lpBinaryPathName))
            } else {
                (0, String::new())
            };
        Some(ServiceInfo { name: name.to_string(), display: String::new(), state: st.dwCurrentState, start_type, binary })
    }
}

pub fn services_all() -> Vec<ServiceInfo> {
    use windows_sys::Win32::System::Services::{EnumServicesStatusExW, ENUM_SERVICE_STATUS_PROCESSW, SC_ENUM_PROCESS_INFO};
    const SERVICE_TYPE_ALL: u32 = 0x3F; // drivers + win32
    const SERVICE_STATE_ALL: u32 = 3;
    let mut out = Vec::new();
    let Some(mgr) = scm(SC_MANAGER_CONNECT | SC_MANAGER_ENUMERATE_SERVICE) else { return out };
    let mut buf = vec![0u8; 256 * 1024];
    let mut needed = 0u32;
    let mut count = 0u32;
    let mut resume = 0u32;
    unsafe {
        let ok = EnumServicesStatusExW(
            mgr.0 as _,
            SC_ENUM_PROCESS_INFO,
            SERVICE_TYPE_ALL,
            SERVICE_STATE_ALL,
            buf.as_mut_ptr(),
            buf.len() as u32,
            &mut needed,
            &mut count,
            &mut resume,
            null(),
        );
        if ok == 0 && count == 0 {
            return out;
        }
        let items = std::slice::from_raw_parts(buf.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW, count as usize);
        for it in items {
            out.push(ServiceInfo {
                name: from_wide_ptr(it.lpServiceName),
                display: from_wide_ptr(it.lpDisplayName),
                state: it.ServiceStatusProcess.dwCurrentState,
                start_type: 0,
                binary: String::new(),
            });
        }
    }
    out
}

pub fn service_stop(name: &str) -> bool {
    use windows_sys::Win32::System::Services::{ControlService, SERVICE_CONTROL_STOP, SERVICE_STATUS};
    let Some(mgr) = scm(SC_MANAGER_CONNECT) else { return false };
    let Some(svc) = open_service(&mgr, name, SERVICE_STOP | SERVICE_QUERY_STATUS) else { return false };
    unsafe {
        let mut st: SERVICE_STATUS = std::mem::zeroed();
        ControlService(svc.0 as _, SERVICE_CONTROL_STOP, &mut st) != 0
    }
}

pub fn service_delete(name: &str) -> bool {
    use windows_sys::Win32::System::Services::DeleteService;
    let Some(mgr) = scm(SC_MANAGER_CONNECT) else { return false };
    let Some(svc) = open_service(&mgr, name, DELETE) else { return false };
    unsafe { DeleteService(svc.0 as _) != 0 }
}

/// Normalises an SCM binary path ("\??\C:\x.sys", quoted, with args) to a file path.
pub fn service_file(binary: &str) -> PathBuf {
    let mut s = binary.trim().trim_start_matches(r"\??\").to_string();
    if let Some(rest) = s.strip_prefix('"') {
        s = rest.split('"').next().unwrap_or_default().to_string();
    }
    if s.to_ascii_lowercase().starts_with(r"\systemroot\") {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        s = format!("{root}{}", &s[11..]);
    }
    PathBuf::from(s)
}

// ── network adapters ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Adapter {
    pub name: String,
    pub description: String,
    pub up: bool,
    pub if_type: u32,
}

pub fn adapters() -> Vec<Adapter> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH};
    let mut out = Vec::new();
    let mut size: u32 = 32 * 1024;
    for _ in 0..3 {
        let mut buf = vec![0u8; size as usize];
        let ret = unsafe { GetAdaptersAddresses(0, 0x0002 | 0x0004 | 0x0008, null(), buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH, &mut size) };
        if ret == 111 {
            continue; // ERROR_BUFFER_OVERFLOW: retry with the size Windows asked for
        }
        if ret != 0 {
            return out;
        }
        let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !p.is_null() {
            let a = unsafe { &*p };
            out.push(Adapter {
                name: unsafe { from_wide_ptr(a.FriendlyName) },
                description: unsafe { from_wide_ptr(a.Description) },
                up: a.OperStatus == 1,
                if_type: a.IfType,
            });
            p = a.Next;
        }
        break;
    }
    out
}

// ── misc ──────────────────────────────────────────────────────────────────

pub fn local_date() -> (u16, u16, u16) {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        (t.wYear, t.wMonth, t.wDay)
    }
}

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    if getrandom::getrandom(&mut buf).is_err() {
        // Extremely unlikely; fall back to time-based entropy rather than panic.
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (t >> ((i % 16) * 8)) as u8 ^ (i as u8).wrapping_mul(151);
        }
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// %ProgramData%\VELIORA — the elevated-only working area for Zapret.
pub fn program_data() -> PathBuf {
    std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\ProgramData")).join("VELIORA")
}

/// Restricts a folder to SYSTEM + Administrators (full) and Users (read/execute),
/// so nothing unelevated can swap the binaries VELIORA runs with admin rights.
pub fn lock_down(dir: &Path) -> bool {
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{GetSecurityDescriptorDacl, ACL, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION};
    // Exactly this DACL on the root, not inherited from ProgramData:
    // SYSTEM and Administrators full control, Users read & execute; all inheritable.
    let sddl = wide("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)");
    let ok = unsafe {
        let mut sd: *mut c_void = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, null_mut()) == 0 {
            return false;
        }
        let mut present = 0;
        let mut defaulted = 0;
        let mut dacl: *mut ACL = null_mut();
        GetSecurityDescriptorDacl(sd, &mut present, &mut dacl, &mut defaulted);
        let path = wide(dir.as_os_str());
        let r = SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            dacl,
            null(),
        );
        windows_sys::Win32::Foundation::LocalFree(sd);
        r == 0
    };
    // Everything inside simply inherits the root's ACL.
    let children = format!("{}\\*", dir.to_string_lossy().trim_end_matches('\\'));
    let (code, _) = run_tool("icacls.exe", &[&children, "/reset", "/T", "/C", "/Q"]);
    ok && code == 0
}

#[cfg(test)]
mod tests {
    #[test]
    fn lock_down_sets_inherited_read_only_for_users() {
        let d = std::env::temp_dir().join("veliora-acl-unit");
        // A previous run leaves the folder read-only for us: hand it back to Temp's ACL first.
        let reset = |d: &std::path::Path| {
            super::run_tool("icacls.exe", &[&d.to_string_lossy(), "/reset", "/T", "/C", "/Q"]);
            let _ = std::fs::remove_dir_all(d);
        };
        reset(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("sub").join("a.txt"), "x").unwrap();
        assert!(super::lock_down(&d));
        let (_, acl) = super::run_tool("icacls.exe", &[&d.join("sub").join("a.txt").to_string_lossy()]);
        println!("{acl}");
        // Users read-only inherited; no explicit/extra user ACE
        assert!(acl.contains("S-1-5-32-545") || acl.contains("(I)(RX)"));
        let aces = acl.lines().filter(|l| l.contains(":(")).count();
        assert_eq!(aces, 3, "expected exactly SYSTEM, Administrators, Users: {acl}");
        // Unelevated (this test): reading works, writing is denied.
        assert_eq!(std::fs::read_to_string(d.join("sub").join("a.txt")).unwrap(), "x");
        assert!(std::fs::write(d.join("sub").join("a.txt"), "y").is_err());
        reset(&d);
    }
}
