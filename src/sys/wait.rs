use libc::{self, c_int};
use {Errno, Result};
use unistd::Pid;

use sys::signal::Signal;

libc_bitflags!(
    /// Defines optional flags for the `waitpid` function.
    pub flags WaitPidFlag: c_int {
        /// Returns immediately if no child has exited
        WNOHANG,
        /// Returns if a child has been stopped, but isn't being traced by ptrace.
        WUNTRACED,
        #[cfg(any(target_os = "linux",
                  target_os = "android"))]
        /// Waits for children that have terminated.
        WEXITED,
        #[cfg(any(target_os = "linux",
                  target_os = "android"))]
        /// Waits for previously stopped children that have been resumed with `SIGCONT`.
        WCONTINUED,
        #[cfg(any(target_os = "linux",
                  target_os = "android"))]
        /// Leave the child in a waitable state; a later wait call can be used to again retrieve
        /// the child status information.
        WNOWAIT,
        #[cfg(any(target_os = "linux",
                  target_os = "android"))]
        /// Don't wait on children of other threads in this group
        __WNOTHREAD,
        #[cfg(any(target_os = "linux",
                  target_os = "android"))]
        /// Wait for all children, regardless of type (clone or non-clone)
        __WALL,
        #[cfg(any(target_os = "linux",
                  target_os = "android"))]
        /// Wait for "clone" children only. If omitted then wait for "non-clone" children only.
        /// (A "clone" child is one which delivers no signal, or a signal other than `SIGCHLD` to
        /// its parent upon termination.) This option is ignored if `__WALL` is also specified.
        __WCLONE,

    }
);

#[cfg(any(target_os = "linux",
          target_os = "android"))]
const WSTOPPED: WaitPidFlag = WUNTRACED;

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
/// Contains the status returned by the `wait` and `waitpid` functions.
pub enum WaitStatus {
    /// Signifies that the process has exited, providing the PID and associated exit status.
    Exited(Pid, i8),
    /// Signifies that the process was killed by a signal, providing the PID and associated signal.
    Signaled(Pid, Signal, bool),
    /// Signifies that the process was stopped by a signal, providing the PID and associated signal.
    Stopped(Pid, Signal),
    #[cfg(any(target_os = "linux", target_os = "android"))]
    PtraceEvent(Pid, Signal, c_int),
    /// Signifies that the process received a `SIGCONT` signal, and thus continued.
    Continued(Pid),
    /// if `WNOHANG` was set, this value is returned when no children have changed state.
    StillAlive
}

#[cfg(any(target_os = "linux",
          target_os = "android"))]
mod status {
    use sys::signal::Signal;
    use libc::c_int;

    pub fn exited(status: i32) -> bool {
        (status & 0x7F) == 0
    }

    pub fn exit_status(status: i32) -> i8 {
        ((status & 0xFF00) >> 8) as i8
    }

    pub fn signaled(status: i32) -> bool {
        ((((status & 0x7f) + 1) as i8) >> 1) > 0
    }

    pub fn term_signal(status: i32) -> Signal {
        Signal::from_c_int(status & 0x7f).unwrap()
    }

    pub fn dumped_core(status: i32) -> bool {
        (status & 0x80) != 0
    }

    pub fn stopped(status: i32) -> bool {
        (status & 0xff) == 0x7f
    }

    pub fn stop_signal(status: i32) -> Signal {
        Signal::from_c_int((status & 0xFF00) >> 8).unwrap()
    }

    pub fn stop_additional(status: i32) -> c_int {
        (status >> 16) as c_int
    }

    pub fn continued(status: i32) -> bool {
        status == 0xFFFF
    }
}

#[cfg(any(target_os = "macos",
          target_os = "ios"))]
mod status {
    use sys::signal::{Signal,SIGCONT};

    const WCOREFLAG: i32 = 0x80;
    const WSTOPPED: i32 = 0x7f;

    fn wstatus(status: i32) -> i32 {
        status & 0x7F
    }

    pub fn exit_status(status: i32) -> i8 {
        ((status >> 8) & 0xFF) as i8
    }

    pub fn stop_signal(status: i32) -> Signal {
        Signal::from_c_int(status >> 8).unwrap()
    }

    pub fn continued(status: i32) -> bool {
        wstatus(status) == WSTOPPED && stop_signal(status) == SIGCONT
    }

    pub fn stopped(status: i32) -> bool {
        wstatus(status) == WSTOPPED && stop_signal(status) != SIGCONT
    }

    pub fn exited(status: i32) -> bool {
        wstatus(status) == 0
    }

    pub fn signaled(status: i32) -> bool {
        wstatus(status) != WSTOPPED && wstatus(status) != 0
    }

    pub fn term_signal(status: i32) -> Signal {
        Signal::from_c_int(wstatus(status)).unwrap()
    }

    pub fn dumped_core(status: i32) -> bool {
        (status & WCOREFLAG) != 0
    }
}

#[cfg(any(target_os = "freebsd",
          target_os = "openbsd",
          target_os = "dragonfly",
          target_os = "netbsd"))]
mod status {
    use sys::signal::Signal;

    const WCOREFLAG: i32 = 0x80;
    const WSTOPPED: i32 = 0x7f;

    fn wstatus(status: i32) -> i32 {
        status & 0x7F
    }

    pub fn stopped(status: i32) -> bool {
        wstatus(status) == WSTOPPED
    }

    pub fn stop_signal(status: i32) -> Signal {
        Signal::from_c_int(status >> 8).unwrap()
    }

    pub fn signaled(status: i32) -> bool {
        wstatus(status) != WSTOPPED && wstatus(status) != 0 && status != 0x13
    }

    pub fn term_signal(status: i32) -> Signal {
        Signal::from_c_int(wstatus(status)).unwrap()
    }

    pub fn exited(status: i32) -> bool {
        wstatus(status) == 0
    }

    pub fn exit_status(status: i32) -> i8 {
        (status >> 8) as i8
    }

    pub fn continued(status: i32) -> bool {
        status == 0x13
    }

    pub fn dumped_core(status: i32) -> bool {
        (status & WCOREFLAG) != 0
    }
}

fn decode(pid : Pid, status: i32) -> WaitStatus {
    if status::exited(status) {
        WaitStatus::Exited(pid, status::exit_status(status))
    } else if status::signaled(status) {
        WaitStatus::Signaled(pid, status::term_signal(status), status::dumped_core(status))
    } else if status::stopped(status) {
        cfg_if! {
            if #[cfg(any(target_os = "linux", target_os = "android"))] {
                fn decode_stopped(pid: Pid, status: i32) -> WaitStatus {
                    let status_additional = status::stop_additional(status);
                    if status_additional == 0 {
                        WaitStatus::Stopped(pid, status::stop_signal(status))
                    } else {
                        WaitStatus::PtraceEvent(pid, status::stop_signal(status), status::stop_additional(status))
                    }
                }
            } else {
                fn decode_stopped(pid: Pid, status: i32) -> WaitStatus {
                    WaitStatus::Stopped(pid, status::stop_signal(status))
                }
            }
        }
        decode_stopped(pid, status)
    } else {
        assert!(status::continued(status));
        WaitStatus::Continued(pid)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
/// Designates whether the supplied `Pid` value is a process ID, process group ID,
/// specifies any child of the current process's group ID, or any child of the current process.
pub enum PidGroup {
    ProcessID(u32),
    ProcessGroupID(u32),
    AnyGroupChild,
    AnyChild,
}

impl From<Pid> for PidGroup {
    fn from(pid: Pid) -> PidGroup {
        if pid > Pid::from_raw(0) {
            PidGroup::ProcessID(i32::from(pid) as u32)
        } else if pid < Pid::from_raw(-1) {
            PidGroup::ProcessGroupID(-i32::from(pid) as u32)
        } else if pid == Pid::from_raw(0) {
            PidGroup::AnyGroupChild
        } else {
            PidGroup::AnyChild
        }
    }
}

impl From<i32> for PidGroup {
    fn from(pid: i32) -> PidGroup {
        if pid > 0 {
            PidGroup::ProcessID(pid as u32)
        } else if pid < -1 {
            PidGroup::ProcessGroupID(-pid as u32)
        } else if pid == 0 {
            PidGroup::AnyGroupChild
        } else {
            PidGroup::AnyChild
        }
    }
}

impl Into<i32> for PidGroup {
    fn into(self) -> i32 {
        match self {
            PidGroup::ProcessID(pid)      => pid as i32,
            PidGroup::ProcessGroupID(pid) => -(pid as i32),
            PidGroup::AnyGroupChild       => 0,
            PidGroup::AnyChild            => -1
        }
    }
}

/// Waits for and returns events that are received from the given supplied process or process group
/// ID, and associated options.
///
/// # Usage Notes
///
/// - If the value of PID is greater than `0`, it will wait on the child with that PID.
/// - If the value of the PID is less than `-1`, it will wait on any child that
///   belongs to the process group with the absolute value of the supplied PID.
/// - If the value of the PID is `0`, it will wait on any child process that has the same
/// group ID as the current process.
/// - If the value of the PID is `-1`, it will wait on any child process of the current process.
/// - If the value of the PID is `None`, the value of PID is set to `-1`.
///
/// # Possible Error Values
///
/// If this function returns an error, the error value will be one of the following:
///
/// - **ECHILD**: The process does not exist or is not a child of the current process.
///   - This may also happen if a child process has the `SIGCHLD` signal masked or set to
///     `SIG_IGN`.
/// - **EINTR**: `WNOHANG` was not set and either an unblocked signal or a `SIGCHLD` was caught.
/// - **EINVAL**: The supplied options were invalid.
pub fn waitpid<O>(pid: PidGroup, options: O) -> Result<WaitStatus>
    where O: Into<Option<WaitPidFlag>>
{
    use self::WaitStatus::*;

    let mut status = 0;
    let options = options.into().map_or(0, |o| o.bits());

    let res = unsafe { libc::waitpid(pid.into(), &mut status as *mut c_int, options) };

    Errno::result(res).map(|res| match res {
        0   => StillAlive,
        res => decode(Pid::from_raw(res), status),
    })
}

/// Waits on any child of the current process.
pub fn wait() -> Result<WaitStatus> {
    waitpid(PidGroup::AnyChild, None)
}
