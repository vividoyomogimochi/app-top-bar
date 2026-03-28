use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use crate::config::ConfigState;

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Wrapper for a Windows Job Object handle that is safe to send across threads.
/// Windows HANDLEs are process-global and inherently thread-safe.
#[cfg(windows)]
struct JobHandle(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for JobHandle {}
#[cfg(windows)]
unsafe impl Sync for JobHandle {}

/// Shared state between the watchdog thread and the main thread.
struct WatchdogState {
    cancelled: AtomicBool,
    child: Mutex<Option<ManagedChild>>,
}

// Safety: ManagedChild contains a Child (which is Send) and a JobHandle (marked Send above).
// The Mutex ensures synchronized access.
unsafe impl Send for WatchdogState {}
unsafe impl Sync for WatchdogState {}

pub struct ServerProcess(Mutex<Option<Arc<WatchdogState>>>);

impl ServerProcess {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }
}

/// A child process with Job Object handle for process-tree cleanup on Windows.
struct ManagedChild {
    child: Child,
    #[cfg(windows)]
    _job: JobHandle,
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::CloseHandle;
            if !self._job.0.is_invalid() {
                unsafe {
                    let _ = CloseHandle(self._job.0);
                }
            }
        }
    }
}

/// Spawn a server process, returning a ManagedChild with Job Object on Windows.
fn spawn_process(command: &str) -> Result<ManagedChild, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Diagnostics::ToolHelp::*;
        use windows::Win32::System::JobObjects::*;
        use windows::Win32::System::Threading::*;

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const CREATE_SUSPENDED: u32 = 0x00000004;

        let child = Command::new(command)
            .creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED)
            .spawn()
            .map_err(|e| format!("Failed to start server: {}", e))?;

        let job = unsafe {
            let job = CreateJobObjectW(None, None)
                .map_err(|e| format!("CreateJobObject failed: {}", e))?;

            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(|e| {
                let _ = CloseHandle(job);
                format!("SetInformationJobObject failed: {}", e)
            })?;

            let proc_handle =
                OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, child.id()).map_err(
                    |e| {
                        let _ = CloseHandle(job);
                        format!("OpenProcess failed: {}", e)
                    },
                )?;
            let result = AssignProcessToJobObject(job, proc_handle);
            let _ = CloseHandle(proc_handle);
            result.map_err(|e| {
                let _ = CloseHandle(job);
                format!("AssignProcessToJobObject failed: {}", e)
            })?;

            // Resume suspended threads.
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
                .map_err(|e| format!("CreateToolhelp32Snapshot failed: {}", e))?;
            let mut entry = THREADENTRY32 {
                dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
                ..Default::default()
            };
            if Thread32First(snapshot, &mut entry).is_ok() {
                loop {
                    if entry.th32OwnerProcessID == child.id() {
                        if let Ok(h) =
                            OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID)
                        {
                            ResumeThread(h);
                            let _ = CloseHandle(h);
                        }
                    }
                    if Thread32Next(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);

            job
        };

        Ok(ManagedChild {
            child,
            _job: JobHandle(job),
        })
    }

    #[cfg(not(windows))]
    {
        let child = Command::new(command)
            .spawn()
            .map_err(|e| format!("Failed to start server: {}", e))?;
        Ok(ManagedChild { child })
    }
}

/// Stop the current watchdog (if any), killing its process.
fn stop_watchdog(server: &ServerProcess) {
    let mut slot = server.0.lock().unwrap();
    if let Some(state) = slot.take() {
        state.cancelled.store(true, Ordering::SeqCst);
        // Kill the child so the watchdog's poll loop exits promptly.
        let _ = state.child.lock().unwrap().take();
    }
}

/// Launch a watchdog thread that spawns the server and auto-restarts on crash.
fn start_watchdog(app: &tauri::AppHandle, command: String) {
    let state = Arc::new(WatchdogState {
        cancelled: AtomicBool::new(false),
        child: Mutex::new(None),
    });

    {
        let server = app.state::<ServerProcess>();
        let mut slot = server.0.lock().unwrap();
        *slot = Some(Arc::clone(&state));
    }

    let handle = app.clone();
    thread::spawn(move || {
        let mut retries = 0u32;

        loop {
            if state.cancelled.load(Ordering::SeqCst) {
                return;
            }

            // Spawn the process.
            let managed = match spawn_process(&command) {
                Ok(m) => m,
                Err(e) => {
                    log::error!("Server spawn failed: {}", e);
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        notify_failure(&handle, &e);
                        return;
                    }
                    thread::sleep(RETRY_DELAY);
                    continue;
                }
            };

            // Store the child (check cancelled while holding the lock).
            {
                let mut slot = state.child.lock().unwrap();
                if state.cancelled.load(Ordering::SeqCst) {
                    drop(managed);
                    return;
                }
                *slot = Some(managed);
            }

            // Poll until the process exits or we're cancelled.
            let exited_naturally = loop {
                thread::sleep(POLL_INTERVAL);

                if state.cancelled.load(Ordering::SeqCst) {
                    let _ = state.child.lock().unwrap().take();
                    return;
                }

                let mut slot = state.child.lock().unwrap();
                match slot.as_mut() {
                    Some(mc) => match mc.child.try_wait() {
                        Ok(Some(_status)) => {
                            *slot = None;
                            break true;
                        }
                        Ok(None) => {} // still running
                        Err(_) => {
                            *slot = None;
                            break true;
                        }
                    },
                    None => return, // child was taken externally
                }
            };

            if !exited_naturally {
                return;
            }

            // Process exited on its own — retry.
            retries += 1;
            if retries >= MAX_RETRIES {
                notify_failure(
                    &handle,
                    &format!(
                        "Server process exited {} times. Giving up.",
                        MAX_RETRIES
                    ),
                );
                return;
            }

            log::warn!(
                "Server exited unexpectedly, restarting ({}/{})",
                retries,
                MAX_RETRIES
            );
            thread::sleep(RETRY_DELAY);
        }
    });
}

fn notify_failure(app: &tauri::AppHandle, detail: &str) {
    let msg = format!(
        "The server process failed to stay running after {} attempts.\n\n{}",
        MAX_RETRIES, detail
    );
    app.dialog()
        .message(msg)
        .title("LED AppBar — Server Error")
        .blocking_show();
}

// --- Public API ---

/// Start the server if a command is configured. Called at app startup.
pub fn start_if_configured(app: &tauri::AppHandle) {
    let command = {
        let state = app.state::<ConfigState>();
        let cfg = state.0.lock().unwrap();
        cfg.server_command.clone()
    };
    if let Some(cmd) = command {
        if !cmd.is_empty() {
            start_watchdog(app, cmd);
        }
    }
}

/// Stop the running server process. Called at app shutdown.
pub fn stop(app: &tauri::AppHandle) {
    let server = app.state::<ServerProcess>();
    stop_watchdog(&*server);
}

/// Apply a new server command: stop old, start new if non-empty.
pub fn apply(app: &tauri::AppHandle, command: Option<String>) {
    let server = app.state::<ServerProcess>();
    stop_watchdog(&*server);
    if let Some(cmd) = command {
        if !cmd.is_empty() {
            start_watchdog(app, cmd);
        }
    }
}
