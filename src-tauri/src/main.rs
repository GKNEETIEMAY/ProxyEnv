#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if proxyenv_lib::try_run_ssh_askpass() {
        return;
    }
    proxyenv_lib::run();
}
