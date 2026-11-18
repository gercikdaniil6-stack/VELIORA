//! Start with Windows via a Task Scheduler logon task that runs with the
//! highest available rights — so VELIORA (and Zapret) start without a UAC prompt.
use crate::win;

const TASK: &str = "VELIORA App Autostart";

fn xml(exe: &str) -> String {
    let user = format!("{}\\{}", std::env::var("USERDOMAIN").unwrap_or_default(), std::env::var("USERNAME").unwrap_or_default());
    let user = user.trim_start_matches('\\').replace('&', "&amp;").replace('<', "&lt;");
    let exe = exe.replace('&', "&amp;").replace('<', "&lt;");
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>VELIORA App: Telegram proxy and Zapret at sign-in</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId></LogonTrigger></Triggers>
  <Principals><Principal id="Author"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
  </Settings>
  <Actions Context="Author"><Exec><Command>{exe}</Command><Arguments>--autostart</Arguments></Exec></Actions>
</Task>"#
    )
}

pub fn enable() -> Result<(), String> {
    if !win::is_admin() {
        return Err("Для запуска вместе с Windows нужны права администратора.".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = std::env::temp_dir().join(format!("veliora-task-{}.xml", win::random_hex(4)));
    let mut data: Vec<u8> = vec![0xFF, 0xFE];
    for u in xml(&exe.to_string_lossy()).encode_utf16() {
        data.extend_from_slice(&u.to_le_bytes());
    }
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    let p = path.to_string_lossy().to_string();
    let (code, out) = win::run_tool("schtasks.exe", &["/Create", "/F", "/TN", TASK, "/XML", &p]);
    let _ = std::fs::remove_file(&path);
    if code == 0 {
        Ok(())
    } else {
        Err(format!("Планировщик заданий не создал задачу (код {code}): {}", out.trim()))
    }
}

pub fn disable() -> Result<(), String> {
    if !is_enabled() {
        return Ok(());
    }
    let (code, out) = win::run_tool("schtasks.exe", &["/Delete", "/F", "/TN", TASK]);
    if code == 0 {
        Ok(())
    } else {
        Err(format!("Не удалось удалить задачу автозапуска: {}", out.trim()))
    }
}

pub fn is_enabled() -> bool {
    win::run_tool("schtasks.exe", &["/Query", "/TN", TASK]).0 == 0
}
