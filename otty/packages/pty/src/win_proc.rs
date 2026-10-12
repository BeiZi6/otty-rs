//! Foreground-process discovery on Windows.
//!
//! ConPTY has no controlling tty, so the Unix "same tty as the shell" filter
//! does not exist here. Direct children of the shell are the usual case; a
//! `.cmd` shim (npm's `claude.cmd` → `node.exe`) lives one level down, so this
//! walks descendants and returns their image name plus command line.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

const TH32CS_SNAPPROCESS: u32 = 0x2;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const PROCESS_COMMAND_LINE_INFORMATION: u32 = 60;
const INVALID_HANDLE_VALUE: isize = -1;

#[repr(C)]
struct ProcessEntry32W {
    dw_size: u32,
    cnt_usage: u32,
    th32_process_id: u32,
    th32_default_heap_id: usize,
    th32_module_id: u32,
    cnt_threads: u32,
    th32_parent_process_id: u32,
    pc_pri_class_base: i32,
    dw_flags: u32,
    sz_exe_file: [u16; 260],
}

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> isize;
    fn Process32FirstW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
    fn CloseHandle(handle: isize) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
}

#[link(name = "ntdll")]
extern "system" {
    fn NtQueryInformationProcess(
        process: isize,
        class: u32,
        info: *mut u8,
        len: u32,
        ret_len: *mut u32,
    ) -> i32;
}

#[link(name = "iphlpapi")]
extern "system" {
    fn GetExtendedTcpTable(table: *mut u8, size: *mut u32, order: i32, family: u32, class: u32, reserved: u32) -> u32;
}

const TCP_TABLE_OWNER_PID_LISTENER: u32 = 3;
/// `(address family, row size, offset of the port, offset of the owning pid)`
/// for `MIB_TCPROW_OWNER_PID` and `MIB_TCP6ROW_OWNER_PID`.
const TCP_TABLES: [(u32, usize, usize, usize); 2] = [(2, 24, 8, 20), (23, 56, 20, 52)];

struct Proc {
    pid: u32,
    parent: u32,
    name: String,
}

pub fn foreground_processes(pid: u32) -> Vec<String> {
    let snapshot = unsafe { snapshot_processes() };
    if snapshot.is_empty() {
        return Vec::new();
    }
    let children = descendants(pid, &snapshot);
    let mut out = Vec::new();
    for child in children {
        if is_noise(&child.name) {
            continue;
        }
        match command_line(child.pid) {
            Some(cmd) if !cmd.is_empty() => out.push(cmd),
            _ => out.push(child.name.clone()),
        }
    }
    out
}

/// The TCP ports `root` and its descendants are listening on, ascending.
pub fn listening_ports(root: u32) -> Vec<u16> {
    let snapshot = unsafe { snapshot_processes() };
    let mut pids: HashSet<u32> = descendants(root, &snapshot).iter().map(|p| p.pid).collect();
    pids.insert(root);
    let mut ports = Vec::new();
    for (family, row, port_at, pid_at) in TCP_TABLES {
        let table = listener_table(family);
        let count = table.get(..4).map_or(0, |n| u32::from_ne_bytes([n[0], n[1], n[2], n[3]]) as usize);
        // The table is a count, then that many rows.
        for entry in table.get(4..).unwrap_or_default().chunks_exact(row).take(count) {
            let pid = u32::from_ne_bytes([entry[pid_at], entry[pid_at + 1], entry[pid_at + 2], entry[pid_at + 3]]);
            if pids.contains(&pid) {
                // The port is in network byte order.
                ports.push(u16::from_be_bytes([entry[port_at], entry[port_at + 1]]));
            }
        }
    }
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// The raw `TCP_TABLE_OWNER_PID_LISTENER` table for one address family, or
/// nothing when the system will not give it.
fn listener_table(family: u32) -> Vec<u8> {
    let mut size = 0u32;
    unsafe {
        GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, family, TCP_TABLE_OWNER_PID_LISTENER, 0);
        // Room for the sockets opened between the two calls.
        size += 1024;
        let mut table = vec![0u8; size as usize];
        match GetExtendedTcpTable(table.as_mut_ptr(), &mut size, 0, family, TCP_TABLE_OWNER_PID_LISTENER, 0) {
            0 => table,
            _ => Vec::new(),
        }
    }
}

fn descendants(root: u32, snapshot: &[Proc]) -> Vec<&Proc> {
    let mut by_parent: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, p) in snapshot.iter().enumerate() {
        by_parent.entry(p.parent).or_default().push(i);
    }
    let mut seen = HashSet::from([root]);
    let mut queue = VecDeque::from([root]);
    let mut out = Vec::new();
    // ponytail: BFS of the process snapshot; cap so a fork bomb cannot hang the tab poll.
    while let Some(pid) = queue.pop_front() {
        if out.len() >= 256 {
            break;
        }
        let Some(kids) = by_parent.get(&pid) else { continue };
        for &i in kids {
            let child = &snapshot[i];
            if !seen.insert(child.pid) {
                continue;
            }
            out.push(child);
            queue.push_back(child.pid);
        }
    }
    out
}

fn is_noise(name: &str) -> bool {
    matches!(
        Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name)
            .to_ascii_lowercase()
            .as_str(),
        "conhost" | "openconsole" | "csrss" | "fontdrvhost" | "runtimebroker"
    )
}

unsafe fn snapshot_processes() -> Vec<Proc> {
    let handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if handle == 0 || handle == INVALID_HANDLE_VALUE {
        return Vec::new();
    }
    let mut entry = std::mem::zeroed::<ProcessEntry32W>();
    entry.dw_size = std::mem::size_of::<ProcessEntry32W>() as u32;
    let mut out = Vec::new();
    if Process32FirstW(handle, &mut entry) != 0 {
        loop {
            out.push(Proc {
                pid: entry.th32_process_id,
                parent: entry.th32_parent_process_id,
                name: utf16_z(&entry.sz_exe_file),
            });
            if Process32NextW(handle, &mut entry) == 0 {
                break;
            }
        }
    }
    CloseHandle(handle);
    out
}

fn utf16_z(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn command_line(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 || handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut needed = 0u32;
        NtQueryInformationProcess(
            handle,
            PROCESS_COMMAND_LINE_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
        if needed < std::mem::size_of::<UnicodeString>() as u32 {
            needed = 4096;
        }
        let mut buf = vec![0u8; needed as usize];
        let status = NtQueryInformationProcess(
            handle,
            PROCESS_COMMAND_LINE_INFORMATION,
            buf.as_mut_ptr(),
            needed,
            &mut needed,
        );
        CloseHandle(handle);
        if status != 0 {
            return None;
        }
        let us = &*(buf.as_ptr() as *const UnicodeString);
        if us.buffer.is_null() || us.length == 0 {
            return None;
        }
        let start = us.buffer as usize;
        let end = start.checked_add(us.length as usize)?;
        let buf_start = buf.as_ptr() as usize;
        let buf_end = buf_start + buf.len();
        if start < buf_start || end > buf_end {
            return None;
        }
        let n = (us.length / 2) as usize;
        let slice = std::slice::from_raw_parts(us.buffer, n);
        let text = String::from_utf16_lossy(slice).trim().to_owned();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{foreground_processes, is_noise, listening_ports, snapshot_processes};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    #[test]
    fn conhost_is_noise_and_claude_is_not() {
        assert!(is_noise("conhost.exe"));
        assert!(is_noise("OpenConsole.exe"));
        assert!(!is_noise("claude.exe"));
        assert!(!is_noise("node.exe"));
        assert!(!is_noise("pwsh.exe"));
    }

    #[test]
    fn a_listening_socket_is_a_port_of_the_process_that_opened_it() {
        let v4 = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let v6 = std::net::TcpListener::bind("[::1]:0").ok();
        let ports = listening_ports(std::process::id());
        for socket in std::iter::once(&v4).chain(&v6) {
            let port = socket.local_addr().unwrap().port();
            assert!(ports.contains(&port), "{port} is not in {ports:?}");
        }
        assert!(ports.windows(2).all(|pair| pair[0] < pair[1]), "sorted, no repeats: {ports:?}");
        // A pid no process has owns nothing.
        assert!(!listening_ports(u32::MAX).contains(&v4.local_addr().unwrap().port()));
    }

    #[test]
    fn snapshot_includes_this_process() {
        let pid = std::process::id();
        let snap = unsafe { snapshot_processes() };
        assert!(
            snap.iter().any(|p| p.pid == pid),
            "snapshot missed pid {pid}, len={}",
            snap.len()
        );
    }

    #[test]
    fn descendants_include_a_spawned_ping() {
        let mut child = Command::new("ping")
            .args(["-n", "20", "127.0.0.1"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("ping");
        let parent = std::process::id();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut procs = Vec::new();
        while Instant::now() < deadline {
            procs = foreground_processes(parent);
            if procs.iter().any(|p| p.to_ascii_lowercase().contains("ping")) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = child.kill();
        assert!(
            procs.iter().any(|p| p.to_ascii_lowercase().contains("ping")),
            "spawned ping was not a descendant of {parent}: {procs:?}"
        );
    }
}
