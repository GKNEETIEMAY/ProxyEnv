use std::collections::HashMap;

use sysinfo::System;

#[derive(Debug, Clone)]
pub struct RunningProcess {
    pub pid: u32,
    pub name: String,
}

pub fn enumerate() -> HashMap<u32, RunningProcess> {
    let system = System::new_all();
    system
        .processes()
        .iter()
        .map(|(pid, process)| {
            let pid = pid.as_u32();
            (
                pid,
                RunningProcess {
                    pid,
                    name: process.name().to_string_lossy().to_ascii_lowercase(),
                },
            )
        })
        .collect()
}
