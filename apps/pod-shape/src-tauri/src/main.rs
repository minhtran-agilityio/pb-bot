// Prevents an extra console window on Windows in release. Kept for parity with pod-board.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pod_shape_lib::run()
}
