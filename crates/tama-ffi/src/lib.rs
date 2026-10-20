//! C ABI over tama-core for native hosts (the Apple Watch app), mirroring crates/tama-web.
//! The contract is include/tama.h.
// Safety contract for every function (see the header): `game` is a live handle from `tama_new` /
// `tama_load`, strings are NUL-terminated UTF-8, and `(ptr, len)` pairs describe valid memory.
#![allow(clippy::missing_safety_doc)]

use std::ffi::{c_char, CStr, CString};
use std::ptr;
use tama_core::data::Data;
use tama_core::Game;

pub struct TamaGame(Game);

unsafe fn str_arg<'a>(s: *const c_char) -> &'a str {
    if s.is_null() {
        return "";
    }
    CStr::from_ptr(s).to_str().unwrap_or("")
}

fn c_string(s: String) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

unsafe fn set_err(err: *mut *mut c_char, msg: String) {
    if !err.is_null() {
        *err = c_string(msg);
    }
}

fn boxed(game: Game) -> *mut TamaGame {
    Box::into_raw(Box::new(TamaGame(game)))
}

/// Hands a Vec to C as (pointer, length); freed by `tama_*_free` with the same length.
unsafe fn give<T>(v: Vec<T>, len: *mut usize) -> *mut T {
    *len = v.len();
    Box::into_raw(v.into_boxed_slice()) as *mut T
}

unsafe fn take<T>(p: *mut T, len: usize) {
    if !p.is_null() {
        drop(Box::from_raw(ptr::slice_from_raw_parts_mut(p, len)));
    }
}

#[no_mangle]
pub unsafe extern "C" fn tama_new(data: *const c_char, seed: u32, err: *mut *mut c_char) -> *mut TamaGame {
    match Data::from_json(str_arg(data)) {
        Ok(data) => {
            let mut game = Game::new(data, seed);
            game.begin_intro();
            boxed(game)
        }
        Err(e) => {
            set_err(err, e.to_string());
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tama_load(data: *const c_char, save: *const c_char, err: *mut *mut c_char) -> *mut TamaGame {
    let loaded = Data::from_json(str_arg(data))
        .map_err(|e| e.to_string())
        .and_then(|data| Game::load(data, str_arg(save)).map_err(|e| e.to_string()));
    match loaded {
        Ok(game) => boxed(game),
        Err(e) => {
            set_err(err, e);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tama_free(game: *mut TamaGame) {
    if !game.is_null() {
        drop(Box::from_raw(game));
    }
}

#[no_mangle]
pub unsafe extern "C" fn tama_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

#[no_mangle]
pub unsafe extern "C" fn tama_u16_free(p: *mut u16, len: usize) {
    take(p, len);
}

#[no_mangle]
pub unsafe extern "C" fn tama_u32_free(p: *mut u32, len: usize) {
    take(p, len);
}

#[no_mangle]
pub unsafe extern "C" fn tama_save(game: *const TamaGame) -> *mut c_char {
    c_string((*game).0.save())
}

#[no_mangle]
pub unsafe extern "C" fn tama_frame(game: *mut TamaGame, now: u64, held: u8) {
    (*game).0.frame(now, held);
}

#[no_mangle]
pub unsafe extern "C" fn tama_poll_call(game: *mut TamaGame, now: u64) -> i32 {
    (*game).0.poll_call(now).map_or(-1, i32::from)
}

#[no_mangle]
pub unsafe extern "C" fn tama_advance_to(game: *mut TamaGame, now: u64) {
    (*game).0.advance_to(now);
}

#[no_mangle]
pub unsafe extern "C" fn tama_framebuffer(game: *const TamaGame, len: *mut usize) -> *const u8 {
    let fb = (*game).0.framebuffer();
    if !len.is_null() {
        *len = fb.len();
    }
    fb.as_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn tama_missing_sprites(game: *const TamaGame, len: *mut usize) -> *mut u16 {
    give((*game).0.missing_sprites(), len)
}

#[no_mangle]
pub unsafe extern "C" fn tama_add_sprite(game: *mut TamaGame, id: u16, bytes: *const u8, len: usize) -> bool {
    if bytes.is_null() {
        return false;
    }
    (*game).0.add_sprite(id, std::slice::from_raw_parts(bytes, len))
}

#[no_mangle]
pub unsafe extern "C" fn tama_take_sounds(game: *mut TamaGame, len: *mut usize) -> *mut u32 {
    let sounds = (*game).0.take_sounds().into_iter().map(|(id, vol)| (id as u32) << 8 | vol as u32).collect();
    give(sounds, len)
}

#[no_mangle]
pub unsafe extern "C" fn tama_take_connect(game: *mut TamaGame) -> *mut c_char {
    c_string((*game).0.take_connect().unwrap_or_default())
}

#[no_mangle]
pub unsafe extern "C" fn tama_text_entry(game: *const TamaGame) -> bool {
    (*game).0.text_entry()
}

#[no_mangle]
pub unsafe extern "C" fn tama_type_key(game: *mut TamaGame, key: *const c_char) -> bool {
    (*game).0.type_key(str_arg(key))
}

#[no_mangle]
pub unsafe extern "C" fn tama_touch(game: *const TamaGame) -> bool {
    (*game).0.touch()
}

#[no_mangle]
pub unsafe extern "C" fn tama_notify(game: *const TamaGame) -> bool {
    (*game).0.notify()
}

#[no_mangle]
pub unsafe extern "C" fn tama_set_notify(game: *mut TamaGame, on: bool) {
    (*game).0.set_notify(on);
}

#[no_mangle]
pub unsafe extern "C" fn tama_set_build(game: *mut TamaGame, build: *const c_char) {
    (*game).0.set_build(str_arg(build));
}

#[no_mangle]
pub unsafe extern "C" fn tama_begin_delete(game: *mut TamaGame) {
    (*game).0.begin_delete();
}
