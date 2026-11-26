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
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut buf = [0u16; 32768];
    let len = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    let dir = if len == 0 || len >= buf.len() {
        PathBuf::from(r"C:\Windows\System32")
    } else {
        PathBuf::from(String::from_utf16_lossy(&buf[..len]))
    };
    dir.join(exe)
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
    pub parent: u32,
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
                out.push(ProcInfo { pid: e.th32ProcessID, parent: e.th32ParentProcessID, name: String::from_utf16_lossy(&e.szExeFile[..len]) });
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
            let configured = SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if configured == 0 {
                CloseHandle(h);
                return None;
            }
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
    pub account: String,
    pub pid: u32,
    pub kind: u32,
    pub accepted: u32,
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

fn inspect_service(svc: &Sc, name: &str) -> Option<ServiceInfo> {
    use windows_sys::Win32::System::Services::*;
    unsafe {
        let mut status: SERVICE_STATUS_PROCESS = std::mem::zeroed();
        let mut needed = 0u32;
        if QueryServiceStatusEx(svc.0 as _, SC_STATUS_PROCESS_INFO, (&mut status as *mut SERVICE_STATUS_PROCESS).cast(),
            std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32, &mut needed) == 0 { return None; }
        let mut buffer = [0usize; 1024];
        if QueryServiceConfigW(svc.0 as _, buffer.as_mut_ptr().cast(), std::mem::size_of_val(&buffer) as u32, &mut needed) == 0 { return None; }
        let cfg = &*buffer.as_ptr().cast::<QUERY_SERVICE_CONFIGW>();
        Some(ServiceInfo {
            name: name.into(), display: from_wide_ptr(cfg.lpDisplayName), state: status.dwCurrentState,
            start_type: cfg.dwStartType, binary: from_wide_ptr(cfg.lpBinaryPathName), account: from_wide_ptr(cfg.lpServiceStartName),
            pid: status.dwProcessId, kind: status.dwServiceType, accepted: status.dwControlsAccepted,
        })
    }
}

pub fn service(name: &str) -> Option<ServiceInfo> {
    let mgr = scm(SC_MANAGER_CONNECT)?;
    let svc = open_service(&mgr, name, SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG)?;
    inspect_service(&svc, name)
}

pub fn stop_verified_service(expected: &ServiceInfo) -> Result<(), String> {
    use windows_sys::Win32::System::Services::*;
    let mgr = scm(SC_MANAGER_CONNECT).ok_or("Windows не разрешила открыть диспетчер служб.")?;
    let svc = open_service(&mgr, &expected.name, SERVICE_STOP | SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG)
        .ok_or("Windows не разрешила остановить службу.")?;
    let actual = inspect_service(&svc, &expected.name).ok_or("Служба изменилась. Выполните проверку повторно.")?;
    if actual.pid != expected.pid || actual.binary != expected.binary || actual.account != expected.account ||
        actual.kind != SERVICE_WIN32_OWN_PROCESS || actual.state != SERVICE_RUNNING || actual.accepted & SERVICE_ACCEPT_STOP == 0 {
        return Err("Служба изменилась. Выполните проверку повторно.".into());
    }
    let mut status = unsafe { std::mem::zeroed::<SERVICE_STATUS>() };
    if unsafe { ControlService(svc.0 as _, SERVICE_CONTROL_STOP, &mut status) } == 0 {
        return Err(format!("Windows не остановила службу: {}", std::io::Error::last_os_error()));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    while std::time::Instant::now() < deadline {
        if inspect_service(&svc, &expected.name).is_some_and(|s| s.state == SERVICE_STOPPED) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err("Служба ещё останавливается. Подождите и повторите включение.".into())
}

pub fn uses_windivert(pid: u32) -> Option<bool> {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid);
        if snap == INVALID_HANDLE_VALUE { return None; }
        let mut module = std::mem::zeroed::<MODULEENTRY32W>();
        module.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
        if Module32FirstW(snap, &mut module) == 0 {
            CloseHandle(snap);
            return None;
        }
        let found = loop {
            if from_wide_ptr(module.szModule.as_ptr()).eq_ignore_ascii_case("WinDivert.dll") { break true; }
            if Module32NextW(snap, &mut module) == 0 { break false; }
        };
        CloseHandle(snap);
        Some(found)
    }
}

pub fn verified_signature(path: &Path) -> bool {
    use windows_sys::Win32::Security::WinTrust::*;
    let name = wide(path.as_os_str());
    let file = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32, pcwszFilePath: name.as_ptr(),
        ..Default::default()
    };
    let mut data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &file as *const _ as *mut _ },
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    unsafe { WinVerifyTrustEx(null_mut(), &mut action, &mut data) == 0 }
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
                account: String::new(),
                pid: it.ServiceStatusProcess.dwProcessId,
                kind: it.ServiceStatusProcess.dwServiceType,
                accepted: it.ServiceStatusProcess.dwControlsAccepted,
            });
        }
    }
    out
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
    pub ipv4_index: u32,
    pub name: String,
    pub description: String,
    pub up: bool,
    pub if_type: u32,
    pub mtu: u32,
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
                ipv4_index: unsafe { a.Anonymous1.Anonymous.IfIndex },
                name: unsafe { from_wide_ptr(a.FriendlyName) },
                description: unsafe { from_wide_ptr(a.Description) },
                up: a.OperStatus == 1,
                if_type: a.IfType,
                mtu: a.Mtu,
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

fn known_folder(id: &windows_sys::core::GUID) -> Option<PathBuf> {
    use windows_sys::Win32::{System::Com::CoTaskMemFree, UI::Shell::SHGetKnownFolderPath};
    unsafe {
        let mut path = null_mut();
        if SHGetKnownFolderPath(id, 0, null_mut(), &mut path) < 0 || path.is_null() {
            return None;
        }
        let dir = PathBuf::from(from_wide_ptr(path));
        CoTaskMemFree(path.cast());
        Some(dir)
    }
}

/// Resolve through Windows, ignoring caller-controlled environment variables.
pub fn program_data() -> PathBuf {
    use windows_sys::Win32::UI::Shell::FOLDERID_ProgramData;
    known_folder(&FOLDERID_ProgramData).unwrap_or_else(|| PathBuf::from(r"C:\ProgramData")).join("VELIORA")
}

pub fn trusted_install() -> bool {
    use windows_sys::Win32::UI::Shell::FOLDERID_ProgramFiles;
    let Some(base)=known_folder(&FOLDERID_ProgramFiles).and_then(|path| path.canonicalize().ok()) else{return false;};
    std::env::current_exe().ok().and_then(|path|path.canonicalize().ok())
      .is_some_and(|exe| exe.starts_with(base))
}

/// Create the root with its final permissions in one operation, including on first launch.
pub fn create_protected_dir(dir: &Path) -> std::io::Result<()> {
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;

    let descriptor = wide("O:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)");
    let path = wide(dir.as_os_str());
    unsafe {
        let mut sd = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor.as_ptr(), SDDL_REVISION_1, &mut sd, null_mut(),
        ) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let attrs = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };
        let created = CreateDirectoryW(path.as_ptr(), &attrs) != 0;
        let error = GetLastError();
        LocalFree(sd);
        if created || error == ERROR_ALREADY_EXISTS {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(error as i32))
        }
    }
}

fn protected_acl(path: &Path) -> bool {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        GetAce, IsWellKnownSid, WinBuiltinAdministratorsSid, WinLocalSystemSid, ACCESS_ALLOWED_ACE,
        DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSID,
    };

    unsafe {
        let mut owner = null_mut();
        let mut dacl = null_mut();
        let mut sd = null_mut();
        if GetNamedSecurityInfoW(wide(path.as_os_str()).as_ptr(), SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION, &mut owner, null_mut(),
            &mut dacl, null_mut(), &mut sd) != 0 {
            return false;
        }
        let trusted = |sid: PSID| !sid.is_null()
            && (IsWellKnownSid(sid, WinBuiltinAdministratorsSid) != 0
                || IsWellKnownSid(sid, WinLocalSystemSid) != 0);
        let valid = trusted(owner) && !dacl.is_null() && (0..u32::from((*dacl).AceCount)).all(|index| {
            let mut raw = null_mut();
            if GetAce(dacl, index, &mut raw) == 0 || raw.is_null() {
                return false;
            }
            let ace = &*raw.cast::<ACCESS_ALLOWED_ACE>();
            match ace.Header.AceType {
                0 => ace.Mask & !0x0012_00a9 == 0
                    || trusted(std::ptr::addr_of!(ace.SidStart).cast_mut().cast()),
                1 => true, // ACCESS_DENIED_ACE cannot grant write access.
                _ => false,
            }
        });
        LocalFree(sd);
        valid
    }
}

/// Validate the owner and write permissions before trusting an existing runtime.
/// An ordinary user could otherwise retain control as the owner after an ACL reset.
pub fn protected_tree(root: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(root)?;
    if meta.file_attributes() & 0x400 != 0 || !protected_acl(root) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Рабочая папка содержит ссылку, файл с недоверенным владельцем или разрешением на запись.",
        ));
    }
    if meta.is_dir() {
        for child in std::fs::read_dir(root)? {
            protected_tree(&child?.path())?;
        }
    }
    Ok(())
}

/// Returns a unique overlapping TCP listener. Unknown or ambiguous ownership stays unknown.
pub fn tcp_listener_owner(address: std::net::SocketAddr) -> Option<u32> {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
        TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};

    fn rows<T: Copy>(family: u16) -> Option<Vec<T>> {
        let mut size = 0;
        for _ in 0..4 {
            let mut table = vec![0_u32; (size as usize).div_ceil(4)];
            let data = if table.is_empty() { std::ptr::null_mut() } else { table.as_mut_ptr().cast() };
            let status = unsafe {
                GetExtendedTcpTable(data, &mut size, 0, family.into(), TCP_TABLE_OWNER_PID_LISTENER, 0)
            };
            match status {
                122 if size <= 4 * 1024 * 1024 => continue,
                0 if !table.is_empty() => {
                    let count = table[0] as usize;
                    if count > (table.len() * 4 - 4) / std::mem::size_of::<T>() {
                        return None;
                    }
                    let base = unsafe { table.as_ptr().add(1).cast::<T>() };
                    return Some((0..count).map(|i| unsafe { base.add(i).read_unaligned() }).collect());
                }
                _ => return None,
            }
        }
        None
    }

    let mut owners = std::collections::BTreeSet::new();
    match address {
        SocketAddr::V4(address) => {
            for row in rows::<MIB_TCPROW_OWNER_PID>(AF_INET)? {
                let ip = Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes());
                if u16::from_be(row.dwLocalPort as u16) == address.port()
                    && (ip.is_unspecified() || address.ip().is_unspecified() || ip == *address.ip())
                {
                    owners.insert(row.dwOwningPid);
                }
            }
        }
        SocketAddr::V6(address) => {
            for row in rows::<MIB_TCP6ROW_OWNER_PID>(AF_INET6)? {
                let ip = Ipv6Addr::from(row.ucLocalAddr);
                if u16::from_be(row.dwLocalPort as u16) == address.port()
                    && (ip.is_unspecified() || address.ip().is_unspecified()
                        || (ip == *address.ip() && row.dwLocalScopeId == address.scope_id()))
                {
                    owners.insert(row.dwOwningPid);
                }
            }
        }
    }
    if owners.len() == 1 { owners.pop_first().filter(|pid| *pid != 0) } else { None }
}

#[cfg(test)]
mod listener_tests {
    #[test]
    fn reports_listener_pid_only_for_matching_address() {
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = socket.local_addr().unwrap();
        assert_eq!(super::tcp_listener_owner(address), Some(std::process::id()));
        let different = std::net::SocketAddr::from(([127, 0, 0, 2], address.port()));
        assert_eq!(super::tcp_listener_owner(different), None);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClosableWindow {
    pid: u32,
    started: u64,
    path: PathBuf,
    hwnd: usize,
    pub label: &'static str,
}

pub fn closable_window(pid: u32) -> Option<ClosableWindow> {
    use windows_sys::Win32::Foundation::{FILETIME, HWND, LPARAM, TRUE};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::Shell::FOLDERID_ProgramFiles;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
    };

    struct Search { pid: u32, hwnd: usize, count: u32 }
    unsafe extern "system" fn visit(hwnd: HWND, param: LPARAM) -> i32 {
        let search = unsafe { &mut *(param as *mut Search) };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == search.pid && IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER).is_null() {
                search.hwnd = hwnd as usize;
                search.count += 1;
            }
        }
        TRUE
    }

    // Keep the automatic action restricted to known desktop applications
    // installed at protected paths. Other listeners can be diagnosed manually.
    let program_files = known_folder(&FOLDERID_ProgramFiles)?;
    let path = process_path(pid)?.canonicalize().ok()?;
    let label = [
        (r"VideoLAN\VLC\vlc.exe", "VLC"),
        (r"OBS Studio\bin\64bit\obs64.exe", "OBS Studio"),
        (r"Microsoft VS Code\Code.exe", "Visual Studio Code"),
    ].into_iter().find_map(|(relative, name)|
        program_files.join(relative).canonicalize().ok().filter(|expected| *expected == path).map(|_| name))?;

    let started = unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() { return None; }
        let mut created: FILETIME = std::mem::zeroed();
        let mut exit = std::mem::zeroed();
        let mut kernel = std::mem::zeroed();
        let mut user = std::mem::zeroed();
        let read = GetProcessTimes(process, &mut created, &mut exit, &mut kernel, &mut user);
        CloseHandle(process);
        if read == 0 { return None; }
        (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime)
    };
    let mut search = Search { pid, hwnd: 0, count: 0 };
    unsafe { EnumWindows(Some(visit), &mut search as *mut Search as LPARAM); }
    (search.count == 1).then_some(ClosableWindow { pid, started, path, hwnd: search.hwnd, label })
}

pub fn request_listener_close(address: std::net::SocketAddr, target: &ClosableWindow) -> Result<(), &'static str> {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, PostMessageW, WM_CLOSE};
    if tcp_listener_owner(address) != Some(target.pid) || closable_window(target.pid).as_ref() != Some(target) {
        return Err("Владелец порта или окно изменились. Диагностика обновлена.");
    }
    let hwnd = target.hwnd as HWND;
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid != target.pid {
            return Err("Окно изменилось. Диагностика обновлена.");
        }
        if PostMessageW(hwnd, WM_CLOSE, 0, 0) == 0 {
            return Err("Windows не разрешила закрыть окно. Закройте программу вручную.");
        }
    }
    Ok(())
}
