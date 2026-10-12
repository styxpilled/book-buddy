//! Simulated input for the dev window, behind the `sim` feature. A named pipe
//! takes one `drive.ps1` step per line and answers each with one line. The
//! events go through GPUI's own dispatch and the pictures come from
//! `render_to_image`, so the window can stay behind the user's windows.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::FromRawHandle;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use futures::StreamExt;
use futures::channel::mpsc;
use gpui::{
    AnyWindowHandle, App, AsyncApp, ExternalPaths, FileDropEvent, KeyDownEvent, KeyUpEvent,
    Keystroke, Modifiers, ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PlatformInput, Point, ScrollDelta, ScrollWheelEvent, TouchPhase, Window, point,
    px,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, HWND_BOTTOM, SPI_GETWHEELSCROLLLINES, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetWindowPos, SystemParametersInfoW,
};
use windows::core::w;

/// Time for a frame to be drawn after a step, before the next one.
const SETTLE: Duration = Duration::from_millis(250);

type Request = (String, std::sync::mpsc::Sender<String>);

/// Puts the window behind the others and starts to take steps.
pub fn serve(handle: AnyWindowHandle, cx: &mut App) {
    handle
        .update(cx, |_, window, _| {
            if let Ok(hwnd) = hwnd(window) {
                // SAFETY: the window is open and owned by this thread.
                unsafe {
                    SetWindowPos(
                        hwnd,
                        Some(HWND_BOTTOM),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    )
                }
                .ok();
            }
        })
        .ok();

    let (sender, mut requests) = mpsc::unbounded::<Request>();
    std::thread::spawn(move || {
        if let Err(error) = listen(sender) {
            log::error!("sim pipe: {error:#}");
        }
    });
    cx.spawn(async move |cx| {
        let mut sim = Sim::default();
        while let Some((line, reply)) = requests.next().await {
            // Not trimmed: `keys: ` types a space.
            let answer = sim
                .step(&line, handle, cx)
                .await
                .unwrap_or_else(|error| format!("error: {error:#}"));
            reply.send(answer).ok();
        }
    })
    .detach();
}

/// One client at a time. Each line goes to the window, and its answer goes
/// back before the next line is read.
fn listen(sender: mpsc::UnboundedSender<Request>) -> Result<()> {
    loop {
        // SAFETY: plain arguments, and the handle is checked below.
        let pipe = unsafe {
            CreateNamedPipeW(
                w!(r"\\.\pipe\book-buddy-dev-sim"),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                4096,
                4096,
                0,
                None,
            )
        };
        if pipe.is_invalid() {
            bail!("CreateNamedPipeW: {}", windows::core::Error::from_thread());
        }
        // A client that connects before this call is not an error.
        // SAFETY: the pipe handle is valid.
        unsafe { ConnectNamedPipe(pipe, None) }.ok();
        // SAFETY: the handle is valid, and the file owns and closes it.
        let file = unsafe { File::from_raw_handle(pipe.0) };
        let mut writer = file.try_clone()?;
        for line in BufReader::new(file).lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let (reply, answer) = std::sync::mpsc::channel();
            sender.unbounded_send((line, reply))?;
            let answer = answer.recv()?;
            if writeln!(writer, "{answer}")
                .and_then(|_| writer.flush())
                .is_err()
            {
                break;
            }
        }
    }
}

#[derive(Default)]
struct Sim {
    /// Where the pointer is, in window pixels from the top left.
    pointer: (i32, i32),
    pressed: Option<MouseButton>,
    modifiers: Modifiers,
    /// Keys from `kdown` that have no `kup` yet, by virtual-key code.
    held: HashSet<u8>,
}

impl Sim {
    async fn step(
        &mut self,
        step: &str,
        handle: AnyWindowHandle,
        cx: &mut AsyncApp,
    ) -> Result<String> {
        let (kind, value) = step.split_once(':').unwrap_or((step, ""));
        let answer = match kind {
            "keys" => {
                // As SendKeys does: the modifiers go down, the key goes down
                // and up, and the modifiers come up. Each event has its own
                // update, so the action of a key down runs before its key up.
                let held = self.modifiers;
                let changed = |modifiers| {
                    PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                        modifiers,
                        capslock: Default::default(),
                    })
                };
                for mut keystroke in send_keys(value)? {
                    keystroke.modifiers.shift |= held.shift;
                    keystroke.modifiers.control |= held.control;
                    keystroke.modifiers.alt |= held.alt;
                    let modifiers = keystroke.modifiers;
                    if modifiers != held {
                        dispatch(handle, cx, changed(modifiers))?;
                    }
                    let down = keystroke.clone();
                    handle.update(cx, |_, window, cx| {
                        window.dispatch_keystroke(down, cx);
                    })?;
                    dispatch(handle, cx, PlatformInput::KeyUp(KeyUpEvent { keystroke }))?;
                    if modifiers != held {
                        dispatch(handle, cx, changed(held))?;
                    }
                }
                "ok".to_string()
            }
            "kdown" | "kup" => {
                let vk: u8 = value.parse().context("a virtual key is a number")?;
                self.key(vk, kind == "kdown", handle, cx)?;
                // As in drive.ps1. A full settle after a key down is as long
                // as a hold, so a tap would become one.
                if kind == "kdown" {
                    cx.background_executor()
                        .timer(Duration::from_millis(30))
                        .await;
                    return Ok("ok".to_string());
                }
                "ok".to_string()
            }
            "click" | "dblclick" | "rclick" | "mclick" => {
                let at = coordinates::<2>(value)?;
                let button = match kind {
                    "rclick" => MouseButton::Right,
                    "mclick" => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                self.move_to((at[0], at[1]), handle, cx)?;
                let count = if kind == "dblclick" { 2 } else { 1 };
                for click_count in 1..=count {
                    self.button(button, true, click_count, handle, cx)?;
                    self.button(button, false, click_count, handle, cx)?;
                }
                "ok".to_string()
            }
            // `down:x,y,2` is the second press of a double click, held.
            "down" | "mdown" => {
                let (at, count) = match coordinates::<3>(value) {
                    Ok([x, y, count]) => ([x, y], count.max(1) as usize),
                    Err(_) => (coordinates::<2>(value)?, 1),
                };
                let button = if kind == "mdown" {
                    MouseButton::Middle
                } else {
                    MouseButton::Left
                };
                self.move_to((at[0], at[1]), handle, cx)?;
                self.button(button, true, count, handle, cx)?;
                "ok".to_string()
            }
            "up" | "mup" => {
                let button = if kind == "mup" {
                    MouseButton::Middle
                } else {
                    MouseButton::Left
                };
                self.button(button, false, 1, handle, cx)?;
                "ok".to_string()
            }
            "move" => {
                let at = coordinates::<2>(value)?;
                // In small steps, so a drag sees the moves between.
                let from = self.pointer;
                for i in 1..=20 {
                    let x = from.0 + (at[0] - from.0) * i / 20;
                    let y = from.1 + (at[1] - from.1) * i / 20;
                    self.move_to((x, y), handle, cx)?;
                    cx.background_executor()
                        .timer(Duration::from_millis(10))
                        .await;
                }
                "ok".to_string()
            }
            // `drop:x,y,path|path`: files dropped from Explorer at x, y.
            "drop" => {
                let mut parts = value.splitn(3, ',');
                let at = coordinates::<2>(&format!(
                    "{},{}",
                    parts.next().unwrap_or_default(),
                    parts.next().unwrap_or_default()
                ))?;
                let paths = ExternalPaths(
                    parts
                        .next()
                        .context("no paths")?
                        .split('|')
                        .map(PathBuf::from)
                        .collect(),
                );
                let position =
                    handle.update(cx, |_, window, _| to_logical(window, (at[0], at[1])))??;
                let entered = FileDropEvent::Entered { position, paths };
                dispatch(handle, cx, PlatformInput::FileDrop(entered))?;
                // A frame between the two, so the drop lands on what is drawn
                // under the pointer.
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let submit = FileDropEvent::Submit { position };
                dispatch(handle, cx, PlatformInput::FileDrop(submit))?;
                "ok".to_string()
            }
            "scroll" => {
                let [x, y, notches] = coordinates::<3>(value)?;
                self.move_to((x, y), handle, cx)?;
                let lines = notches as f32 * wheel_scroll_lines();
                let modifiers = self.modifiers;
                handle.update(cx, |_, window, cx| -> Result<()> {
                    window.dispatch_event(
                        PlatformInput::ScrollWheel(ScrollWheelEvent {
                            position: to_logical(window, (x, y))?,
                            delta: ScrollDelta::Lines(point(0., lines)),
                            modifiers,
                            touch_phase: TouchPhase::Moved,
                        }),
                        cx,
                    );
                    Ok(())
                })??;
                "ok".to_string()
            }
            "hover" => {
                let at = coordinates::<2>(value)?;
                self.move_to((at[0], at[1]), handle, cx)?;
                // GPUI keeps the cursor it picks to itself, and sets it only
                // while the real pointer is over the window.
                format!(
                    "hover {},{}: unknown, sim cannot read the cursor",
                    at[0], at[1]
                )
            }
            "shot" => {
                cx.background_executor().timer(SETTLE).await;
                let path = value.to_string();
                handle.update(cx, |_, window, _| shot(window, &path))??;
                return Ok(path);
            }
            "wait" => {
                let ms: u64 = value.parse().context("wait takes milliseconds")?;
                cx.background_executor()
                    .timer(Duration::from_millis(ms))
                    .await;
                return Ok("ok".to_string());
            }
            _ => bail!("unknown step {step}"),
        };
        cx.background_executor().timer(SETTLE).await;
        Ok(answer)
    }

    fn move_to(
        &mut self,
        at: (i32, i32),
        handle: AnyWindowHandle,
        cx: &mut AsyncApp,
    ) -> Result<()> {
        self.pointer = at;
        let (pressed_button, modifiers) = (self.pressed, self.modifiers);
        handle.update(cx, |_, window, cx| -> Result<()> {
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: to_logical(window, at)?,
                    pressed_button,
                    modifiers,
                }),
                cx,
            );
            Ok(())
        })?
    }

    fn button(
        &mut self,
        button: MouseButton,
        down: bool,
        click_count: usize,
        handle: AnyWindowHandle,
        cx: &mut AsyncApp,
    ) -> Result<()> {
        self.pressed = down.then_some(button);
        let (at, modifiers) = (self.pointer, self.modifiers);
        handle.update(cx, |_, window, cx| -> Result<()> {
            let position = to_logical(window, at)?;
            let event = if down {
                PlatformInput::MouseDown(MouseDownEvent {
                    button,
                    position,
                    modifiers,
                    click_count,
                    first_mouse: false,
                })
            } else {
                PlatformInput::MouseUp(MouseUpEvent {
                    button,
                    position,
                    modifiers,
                    click_count,
                })
            };
            window.dispatch_event(event, cx);
            Ok(())
        })?
    }

    fn key(
        &mut self,
        vk: u8,
        down: bool,
        handle: AnyWindowHandle,
        cx: &mut AsyncApp,
    ) -> Result<()> {
        let modifier = match vk {
            16 | 160 | 161 => Some(&mut self.modifiers.shift),
            17 | 162 | 163 => Some(&mut self.modifiers.control),
            18 | 164 | 165 => Some(&mut self.modifiers.alt),
            _ => None,
        };
        let event = if let Some(modifier) = modifier {
            *modifier = down;
            PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers: self.modifiers,
                capslock: Default::default(),
            })
        } else {
            let keystroke = Keystroke {
                modifiers: self.modifiers,
                key: key_name(vk)?,
                key_char: None,
            };
            if down {
                let is_held = !self.held.insert(vk);
                PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: keystroke.with_simulated_ime(),
                    is_held,
                    prefer_character_input: false,
                })
            } else {
                self.held.remove(&vk);
                PlatformInput::KeyUp(KeyUpEvent { keystroke })
            }
        };
        handle.update(cx, |_, window, cx| {
            window.dispatch_event(event, cx);
        })
    }
}

fn dispatch(handle: AnyWindowHandle, cx: &mut AsyncApp, event: PlatformInput) -> Result<()> {
    handle.update(cx, |_, window, cx| {
        window.dispatch_event(event, cx);
    })
}

fn coordinates<const N: usize>(value: &str) -> Result<[i32; N]> {
    let numbers = value
        .split(',')
        .map(|part| part.trim().parse::<i32>())
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("not numbers: {value}"))?;
    numbers
        .try_into()
        .map_err(|_| anyhow!("expected {N} numbers: {value}"))
}

fn hwnd(window: &Window) -> Result<HWND> {
    let handle = HasWindowHandle::window_handle(window).map_err(|error| anyhow!("{error}"))?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as _)),
        _ => bail!("not a Win32 window"),
    }
}

/// The size of the window rectangle, and where the client area starts inside
/// it, in device pixels. `drive.ps1` measures from the window rectangle, and so
/// does sim.
fn window_frame(window: &Window) -> Result<((u32, u32), (i32, i32))> {
    let hwnd = hwnd(window)?;
    let mut rect = RECT::default();
    let mut origin = POINT::default();
    // SAFETY: the window is open, and both pointers are to locals.
    unsafe {
        GetWindowRect(hwnd, &mut rect)?;
        ClientToScreen(hwnd, &mut origin).ok()?;
    }
    let size = (
        (rect.right - rect.left) as u32,
        (rect.bottom - rect.top) as u32,
    );
    Ok((size, (origin.x - rect.left, origin.y - rect.top)))
}

fn to_logical(window: &Window, (x, y): (i32, i32)) -> Result<Point<gpui::Pixels>> {
    let (_, (dx, dy)) = window_frame(window)?;
    let scale = window.scale_factor();
    Ok(point(
        px((x - dx) as f32 / scale),
        px((y - dy) as f32 / scale),
    ))
}

/// The last frame, drawn again off screen, in a picture the size of the window
/// rectangle, so it measures the way a `drive.ps1` picture does.
fn shot(window: &Window, path: &str) -> Result<()> {
    let frame = window.render_to_image()?;
    let ((width, height), (dx, dy)) = window_frame(window)?;
    let mut picture = image::RgbaImage::new(width, height);
    image::imageops::overlay(&mut picture, &frame, dx.into(), dy.into());
    picture
        .save(path)
        .with_context(|| format!("writing {path}"))
}

fn wheel_scroll_lines() -> f32 {
    let mut lines = 3u32;
    // SAFETY: the pointer is to a local of the size the call writes.
    unsafe {
        SystemParametersInfoW(
            SPI_GETWHEELSCROLLLINES,
            0,
            Some(&mut lines as *mut u32 as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .ok();
    lines as f32
}

fn key_name(vk: u8) -> Result<String> {
    Ok(match vk {
        8 => "backspace".into(),
        9 => "tab".into(),
        13 => "enter".into(),
        27 => "escape".into(),
        32 => "space".into(),
        33 => "pageup".into(),
        34 => "pagedown".into(),
        35 => "end".into(),
        36 => "home".into(),
        37 => "left".into(),
        38 => "up".into(),
        39 => "right".into(),
        40 => "down".into(),
        45 => "insert".into(),
        46 => "delete".into(),
        b'0'..=b'9' => (vk as char).to_string(),
        b'A'..=b'Z' => (vk as char).to_ascii_lowercase().to_string(),
        112..=123 => format!("f{}", vk - 111),
        _ => bail!("no key for virtual key {vk}"),
    })
}

/// The keystrokes of a SendKeys string: `+` shift, `^` control, `%` alt, each
/// for the next key or `(group)`, `~` enter, and `{NAME}` or `{NAME count}`.
fn send_keys(text: &str) -> Result<Vec<Keystroke>> {
    let mut keystrokes = Vec::new();
    let mut modifiers = Modifiers::default();
    let mut group: Option<Modifiers> = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let mut keys: Vec<Key> = Vec::new();
        match c {
            '+' => {
                modifiers.shift = true;
                continue;
            }
            '^' => {
                modifiers.control = true;
                continue;
            }
            '%' => {
                modifiers.alt = true;
                continue;
            }
            '(' => {
                group = Some(modifiers);
                continue;
            }
            ')' => {
                group = None;
                modifiers = Modifiers::default();
                continue;
            }
            '~' => keys.push(Key::named("enter")),
            '{' => {
                let mut name = String::new();
                // `{}}` and `{{}` name the braces themselves.
                if let Some(&first) = chars.peek() {
                    name.push(first);
                    chars.next();
                }
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    name.push(c);
                }
                let (name, count) = match name.rsplit_once(' ') {
                    Some((name, count)) => (name.to_string(), count.parse().context("a count")?),
                    None => (name, 1),
                };
                let key = brace_key(&name)?;
                keys.extend(std::iter::repeat_n(key, count));
            }
            c => keys.push(Key::char(c)),
        }
        for key in keys {
            let mut modifiers = modifiers;
            modifiers.shift |= key.shift;
            let key_char = key
                .key_char
                .filter(|_| !modifiers.control && !modifiers.alt);
            keystrokes.push(Keystroke {
                modifiers,
                key: key.key,
                key_char,
            });
        }
        modifiers = group.unwrap_or_default();
    }
    Ok(keystrokes)
}

#[derive(Clone)]
struct Key {
    key: String,
    key_char: Option<String>,
    shift: bool,
}

impl Key {
    fn named(key: &str) -> Self {
        Key {
            key: key.into(),
            key_char: None,
            shift: false,
        }
    }

    /// An upper case letter is shift and the lower case key.
    fn char(c: char) -> Self {
        let key = match c {
            ' ' => "space".into(),
            c => c.to_ascii_lowercase().to_string(),
        };
        Key {
            key,
            key_char: Some(c.to_string()),
            shift: c.is_ascii_uppercase(),
        }
    }
}

fn brace_key(name: &str) -> Result<Key> {
    let key = match name.to_ascii_uppercase().as_str() {
        "BACKSPACE" | "BS" | "BKSP" => "backspace",
        "DELETE" | "DEL" => "delete",
        "DOWN" => "down",
        "UP" => "up",
        "LEFT" => "left",
        "RIGHT" => "right",
        "END" => "end",
        "HOME" => "home",
        "ENTER" => "enter",
        "ESC" => "escape",
        "INSERT" | "INS" => "insert",
        "PGDN" => "pagedown",
        "PGUP" => "pageup",
        "TAB" => "tab",
        "SPACE" => return Ok(Key::char(' ')),
        f if f.len() > 1 && f.starts_with('F') && f[1..].parse::<u8>().is_ok() => {
            return Ok(Key::named(&f.to_ascii_lowercase()));
        }
        _ if name.chars().count() == 1 => return Ok(Key::char(name.chars().next().unwrap())),
        _ => bail!("no key named {{{name}}}"),
    };
    Ok(Key::named(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spelled(text: &str) -> Vec<String> {
        send_keys(text)
            .unwrap()
            .iter()
            .map(|k| format!("{}{:?}", k.unparse(), k.key_char))
            .collect()
    }

    #[test]
    fn send_keys_spelling() {
        assert_eq!(spelled("aB"), ["aSome(\"a\")", "shift-bSome(\"B\")"]);
        assert_eq!(
            spelled("{ESC}{DOWN 2}~"),
            ["escapeNone", "downNone", "downNone", "enterNone"]
        );
        assert_eq!(spelled("^a+{TAB}"), ["ctrl-aNone", "shift-tabNone"]);
        assert_eq!(
            spelled("^(ab)c"),
            ["ctrl-aNone", "ctrl-bNone", "cSome(\"c\")"]
        );
        assert_eq!(spelled("{+}{}}"), ["+Some(\"+\")", "}Some(\"}\")"]);
    }
}
