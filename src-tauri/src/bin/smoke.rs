use std::{thread, time::Duration};

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

fn main() {
    println!("Focus a window with selected text. Copying in 3 seconds...");
    thread::sleep(Duration::from_secs(3));

    let mut clipboard = Clipboard::new().expect("clipboard open");
    let original = clipboard.get_text().unwrap_or_default();
    println!("original clipboard: {original:?}");
    clipboard.set_text("").expect("clear");

    let mut enigo = Enigo::new(&Settings::default()).expect("enigo");
    enigo.key(Key::Control, Direction::Press).unwrap();
    enigo.key(Key::Unicode('c'), Direction::Click).unwrap();
    enigo.key(Key::Control, Direction::Release).unwrap();

    let mut captured = String::new();
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(20));
        let value = clipboard.get_text().unwrap_or_default();
        if !value.trim().is_empty() {
            captured = value;
            break;
        }
    }
    println!("captured: {captured:?}");

    if captured.trim().is_empty() {
        println!("FAIL: nothing captured — Ctrl+C did not reach the target window");
        return;
    }

    clipboard.set_text(format!("[{captured}]")).expect("set");
    enigo.key(Key::Control, Direction::Press).unwrap();
    enigo.key(Key::Unicode('v'), Direction::Click).unwrap();
    enigo.key(Key::Control, Direction::Release).unwrap();
    thread::sleep(Duration::from_millis(300));
    clipboard.set_text(original).expect("restore");
    println!("OK: pasted the captured text wrapped in brackets, clipboard restored");
}
