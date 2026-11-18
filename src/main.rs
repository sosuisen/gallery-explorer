// A release build opens no console window. A debug build keeps it to show startup errors.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod domain;
mod explorer;
mod infra;
mod ui;

use windows_reactor::App;

fn main() {
    App::run_component::<ui::ExplorerWindow>(()).expect("the window could not be created");
}
