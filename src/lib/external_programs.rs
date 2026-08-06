// SPDX-License-Identifier: BSD-3-Clause

#[allow(unused)]
pub static POLKIT_STDIN_AGENT: &str = match option_env!("POLKIT_STDIN_AGENT") {
    Some(x) => x,
    None => "polkit-stdin-agent",
};

#[allow(unused)]
pub static RUN0_CMD: &str = match option_env!("RUN0") {
    Some(x) => x,
    None => "run0",
};

#[allow(unused)]
pub static TRUE_CMD: &str = match option_env!("TRUE") {
    Some(x) => x,
    None => "true",
};

#[allow(unused)]
pub static RUN0_EDIT_DAEMON: &str = match option_env!("RUN0_EDIT_DAEMON") {
    Some(x) => x,
    None => "run0-edit-daemon",
};

#[allow(unused)]
pub static SYSTEMD_RUN_CMD: &str = match option_env!("SYSTEMD_RUN") {
    Some(x) => x,
    None => "systemd-run",
};

#[allow(unused)]
pub static AUDITCTL_CMD: &str = match option_env!("AUDITCTL") {
    Some(x) => x,
    None => "auditctl",
};

#[allow(unused)]
pub static AUDISP_SOCKET: &str = match option_env!("AUDISPD_SOCKET") {
    Some(x) => x,
    None => "/run/audit/audispd_events",
};
