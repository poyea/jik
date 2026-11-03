// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

//! A small modal window built from standard Windows controls.
//! The app places the controls and handles the events. `winkit` knows nothing about the content.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, CreateFontIndirectW, DeleteObject, HBRUSH, HFONT, HGDIOBJ,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    BST_CHECKED, BST_UNCHECKED, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
};
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, SystemParametersInfoForDpi};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_GROUPBOX, BS_PUSHBUTTON,
    CB_ADDSTRING, CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL, CBS_DROPDOWNLIST, CW_USEDEFAULT,
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL,
    ES_AUTOVSCROLL, ES_MULTILINE, ES_NUMBER, ES_WANTRETURN, GWLP_USERDATA, GetMessageW,
    GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, HMENU, IDC_ARROW, IsDialogMessageW,
    LB_ADDSTRING, LB_DELETESTRING, LB_GETCURSEL, LB_INSERTSTRING, LB_RESETCONTENT, LB_SETCURSEL,
    LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, LoadCursorW, MB_ICONWARNING, MB_OK, MSG, MessageBoxW,
    NONCLIENTMETRICSW, PostQuitMessage, RegisterClassExW, SPI_GETNONCLIENTMETRICS, SPI_GETWORKAREA,
    SW_SHOWNORMAL, SWP_NOSIZE, SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SendMessageW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
    SystemParametersInfoW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND,
    WM_NCDESTROY, WM_SETFONT, WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_EX_APPWINDOW, WS_EX_CLIENTEDGE,
    WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
    WS_VSCROLL,
};
use windows::core::{HSTRING, PCWSTR, w};

const CLASS_NAME: PCWSTR = w!("WinkitDialog");
const FIRST_ID: usize = 100;
const ID_OK: usize = 1;
const ID_CANCEL: usize = 2;
const SS_CENTERIMAGE: u32 = 0x200;
const BN_CLICKED: u32 = 0;
const SELCHANGE: u32 = 1;
const EN_CHANGE: u32 = 0x300;
/// The extra height of a closed drop-down list that holds its open list.
const DROP_DOWN_LIST_HEIGHT: i32 = 200;

/// A position and size in pixels at 100% display scale. The dialog scales them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Control<'a> {
    Label(&'a str),
    Group(&'a str),
    Edit { multiline: bool, digits_only: bool },
    DropDown,
    List,
    Check(&'a str),
    Button { text: &'a str, default: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Static,
    Edit,
    DropDown,
    List,
    Check,
    Button,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A button or a check box was clicked.
    Clicked(ControlId),
    /// The user changed the text of an edit box or the selection of a list.
    /// Changes made through [`Dialog`] methods do not send this event.
    Changed(ControlId),
    /// The user pressed Enter outside a multiline edit box.
    Submit,
    /// The user pressed Esc or the close button.
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The handler returned [`Flow::Close`].
    Closed,
    /// The thread got WM_QUIT. The dialog posts WM_QUIT again for the caller.
    Quit,
}

/// The window procedure only puts events in the queue. [`Dialog::run`] gives them to the handler
/// outside the window procedure, so the handler can change the controls safely.
struct Shared {
    kinds: RefCell<Vec<Kind>>,
    events: RefCell<VecDeque<Event>>,
    muted: Cell<bool>,
}

pub struct Dialog {
    hwnd: HWND,
    font: HFONT,
    scale: f32,
    controls: Vec<HWND>,
    shared: *const Shared,
}

impl Dialog {
    /// `width` and `height` are the client size at 100% display scale.
    pub fn new(title: &str, width: i32, height: i32) -> Result<Self> {
        unsafe {
            let _ = InitCommonControlsEx(&INITCOMMONCONTROLSEX {
                dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_STANDARD_CLASSES,
            });
            let hinstance: HINSTANCE = GetModuleHandleW(None).context("GetModuleHandleW")?.into();
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance,
                hCursor: LoadCursorW(None, IDC_ARROW).context("LoadCursorW")?,
                hbrBackground: HBRUSH((COLOR_BTNFACE.0 + 1) as usize as *mut c_void),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            // The class stays registered after the first dialog, so a second registration fails.
            RegisterClassExW(&wc);

            let dpi = crate::primary_monitor_dpi();
            let scale = dpi as f32 / 96.0;
            let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
            let ex_style = WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT | WS_EX_APPWINDOW;
            let mut frame = RECT {
                left: 0,
                top: 0,
                right: scaled(width, scale),
                bottom: scaled(height, scale),
            };
            AdjustWindowRectExForDpi(&mut frame, style, false, ex_style, dpi)
                .context("AdjustWindowRectExForDpi")?;
            let (outer_w, outer_h) = (frame.right - frame.left, frame.bottom - frame.top);

            let hwnd = CreateWindowExW(
                ex_style,
                CLASS_NAME,
                &HSTRING::from(title),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                outer_w,
                outer_h,
                None,
                None,
                Some(hinstance),
                None,
            )
            .context("CreateWindowExW(dialog)")?;

            let shared = Box::into_raw(Box::new(Shared {
                kinds: RefCell::new(Vec::new()),
                events: RefCell::new(VecDeque::new()),
                muted: Cell::new(false),
            }));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, shared as isize);

            let mut work = RECT::default();
            if SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut work as *mut _ as *mut c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
            .is_ok()
            {
                let x = work.left + ((work.right - work.left - outer_w) / 2).max(0);
                let y = work.top + ((work.bottom - work.top - outer_h) / 2).max(0);
                let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
            }

            Ok(Self {
                hwnd,
                font: message_font(dpi),
                scale,
                controls: Vec::new(),
                shared,
            })
        }
    }

    fn shared(&self) -> &Shared {
        unsafe { &*self.shared }
    }

    pub fn add(&mut self, control: Control, bounds: Bounds) -> Result<ControlId> {
        let child = WS_CHILD | WS_VISIBLE;
        let (class, text, kind, style, ex_style) = match control {
            Control::Label(text) => (
                w!("STATIC"),
                text,
                Kind::Static,
                child | WINDOW_STYLE(SS_CENTERIMAGE),
                WINDOW_EX_STYLE(0),
            ),
            Control::Group(text) => (
                w!("BUTTON"),
                text,
                Kind::Static,
                child | WINDOW_STYLE(BS_GROUPBOX as u32),
                WINDOW_EX_STYLE(0),
            ),
            Control::Edit {
                multiline,
                digits_only,
            } => {
                let mut style = child | WS_TABSTOP;
                if multiline {
                    style |= WS_VSCROLL
                        | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32);
                } else {
                    style |= WINDOW_STYLE(ES_AUTOHSCROLL as u32);
                }
                if digits_only {
                    style |= WINDOW_STYLE(ES_NUMBER as u32);
                }
                (w!("EDIT"), "", Kind::Edit, style, WS_EX_CLIENTEDGE)
            }
            Control::DropDown => (
                w!("COMBOBOX"),
                "",
                Kind::DropDown,
                child | WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                WINDOW_EX_STYLE(0),
            ),
            Control::List => (
                w!("LISTBOX"),
                "",
                Kind::List,
                child
                    | WS_TABSTOP
                    | WS_VSCROLL
                    | WINDOW_STYLE((LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32),
                WS_EX_CLIENTEDGE,
            ),
            Control::Check(text) => (
                w!("BUTTON"),
                text,
                Kind::Check,
                child | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                WINDOW_EX_STYLE(0),
            ),
            Control::Button { text, default } => {
                let push = if default {
                    BS_DEFPUSHBUTTON
                } else {
                    BS_PUSHBUTTON
                };
                (
                    w!("BUTTON"),
                    text,
                    Kind::Button,
                    child | WS_TABSTOP | WINDOW_STYLE(push as u32),
                    WINDOW_EX_STYLE(0),
                )
            }
        };
        let s = |v: i32| scaled(v, self.scale);
        let height = match kind {
            Kind::DropDown => s(bounds.height) + s(DROP_DOWN_LIST_HEIGHT),
            _ => s(bounds.height),
        };
        let id = FIRST_ID + self.controls.len();
        self.shared().kinds.borrow_mut().push(kind);
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                class,
                &HSTRING::from(text),
                style,
                s(bounds.x),
                s(bounds.y),
                s(bounds.width),
                height,
                Some(self.hwnd),
                Some(HMENU(id as *mut c_void)),
                None,
                None,
            )
        };
        let hwnd = match hwnd {
            Ok(hwnd) => hwnd,
            Err(err) => {
                self.shared().kinds.borrow_mut().pop();
                return Err(err).context("CreateWindowExW(control)");
            }
        };
        unsafe {
            SendMessageW(
                hwnd,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        self.controls.push(hwnd);
        Ok(ControlId(id - FIRST_ID))
    }

    fn control(&self, id: ControlId) -> HWND {
        self.controls[id.0]
    }

    fn kind(&self, id: ControlId) -> Kind {
        self.shared().kinds.borrow()[id.0]
    }

    fn send(&self, id: ControlId, msg: u32, wparam: usize, lparam: isize) -> isize {
        unsafe {
            SendMessageW(
                self.control(id),
                msg,
                Some(WPARAM(wparam)),
                Some(LPARAM(lparam)),
            )
            .0
        }
    }

    /// Run `change` without [`Event::Changed`] events.
    fn quietly<T>(&self, change: impl FnOnce() -> T) -> T {
        self.shared().muted.set(true);
        let result = change();
        self.shared().muted.set(false);
        result
    }

    pub fn text(&self, id: ControlId) -> String {
        unsafe {
            let hwnd = self.control(id);
            let len = GetWindowTextLengthW(hwnd).max(0) as usize;
            let mut buf = vec![0u16; len + 1];
            let copied = GetWindowTextW(hwnd, &mut buf).max(0) as usize;
            String::from_utf16_lossy(&buf[..copied])
        }
    }

    pub fn set_text(&self, id: ControlId, text: &str) {
        self.quietly(|| unsafe {
            let _ = SetWindowTextW(self.control(id), &HSTRING::from(text));
        });
    }

    pub fn set_items<S: AsRef<str>>(&self, id: ControlId, items: &[S]) {
        let (reset, add) = match self.kind(id) {
            Kind::DropDown => (CB_RESETCONTENT, CB_ADDSTRING),
            Kind::List => (LB_RESETCONTENT, LB_ADDSTRING),
            _ => return,
        };
        self.quietly(|| {
            self.send(id, reset, 0, 0);
            for item in items {
                let text = HSTRING::from(item.as_ref());
                self.send(id, add, 0, text.as_ptr() as isize);
            }
        });
    }

    /// Replace one item of a list, and keep the selection.
    pub fn set_item(&self, id: ControlId, index: usize, text: &str) {
        if self.kind(id) != Kind::List {
            return;
        }
        let selected = self.selected(id);
        let text = HSTRING::from(text);
        self.quietly(|| {
            self.send(id, LB_DELETESTRING, index, 0);
            self.send(id, LB_INSERTSTRING, index, text.as_ptr() as isize);
        });
        self.set_selected(id, selected);
    }

    pub fn selected(&self, id: ControlId) -> Option<usize> {
        let get = match self.kind(id) {
            Kind::DropDown => CB_GETCURSEL,
            Kind::List => LB_GETCURSEL,
            _ => return None,
        };
        usize::try_from(self.send(id, get, 0, 0)).ok()
    }

    pub fn set_selected(&self, id: ControlId, index: Option<usize>) {
        let set = match self.kind(id) {
            Kind::DropDown => CB_SETCURSEL,
            Kind::List => LB_SETCURSEL,
            _ => return,
        };
        self.quietly(|| self.send(id, set, index.unwrap_or(usize::MAX), 0));
    }

    pub fn checked(&self, id: ControlId) -> bool {
        self.send(id, BM_GETCHECK, 0, 0) == BST_CHECKED.0 as isize
    }

    pub fn set_checked(&self, id: ControlId, checked: bool) {
        let state = if checked { BST_CHECKED } else { BST_UNCHECKED };
        self.quietly(|| self.send(id, BM_SETCHECK, state.0 as usize, 0));
    }

    pub fn set_enabled(&self, id: ControlId, enabled: bool) {
        unsafe {
            let _ = EnableWindow(self.control(id), enabled);
        }
    }

    pub fn focus(&self, id: ControlId) {
        unsafe {
            let _ = SetFocus(Some(self.control(id)));
        }
    }

    /// Show a message that the user must close before the dialog continues.
    pub fn warn(&self, title: &str, message: &str) {
        unsafe {
            MessageBoxW(
                Some(self.hwnd),
                &HSTRING::from(message),
                &HSTRING::from(title),
                MB_OK | MB_ICONWARNING,
            );
        }
    }

    /// Show the dialog and give each event to `handler` until it returns [`Flow::Close`].
    pub fn run(&mut self, mut handler: impl FnMut(&mut Dialog, Event) -> Flow) -> Result<Outcome> {
        unsafe {
            // The first ShowWindow of a process can use the show state from the shortcut or
            // the parent process (for example, hidden or minimized). The second call is exact.
            let _ = ShowWindow(self.hwnd, SW_SHOWNORMAL);
            let _ = ShowWindow(self.hwnd, SW_SHOWNORMAL);
            let _ = SetForegroundWindow(self.hwnd);
            if let Some(first) = (0..self.controls.len())
                .map(ControlId)
                .find(|&id| self.kind(id) != Kind::Static)
            {
                self.focus(first);
            }
            let mut msg = MSG::default();
            loop {
                loop {
                    let event = self.shared().events.borrow_mut().pop_front();
                    let Some(event) = event else {
                        break;
                    };
                    if handler(self, event) == Flow::Close {
                        return Ok(Outcome::Closed);
                    }
                }
                match GetMessageW(&mut msg, None, 0, 0).0 {
                    0 => {
                        PostQuitMessage(msg.wParam.0 as i32);
                        return Ok(Outcome::Quit);
                    }
                    -1 => {
                        return Err(windows::core::Error::from_thread()).context("GetMessageW");
                    }
                    _ => {}
                }
                if !IsDialogMessageW(self.hwnd, &msg).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}

impl Drop for Dialog {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
            let _ = DeleteObject(HGDIOBJ(self.font.0));
            drop(Box::from_raw(self.shared as *mut Shared));
        }
    }
}

fn scaled(value: i32, scale: f32) -> i32 {
    (value as f32 * scale).round() as i32
}

/// The font that Windows uses for message boxes, at the given DPI.
fn message_font(dpi: u32) -> HFONT {
    unsafe {
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        let read = SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            metrics.cbSize,
            Some(&mut metrics as *mut _ as *mut c_void),
            0,
            dpi,
        );
        if read.is_ok() {
            CreateFontIndirectW(&metrics.lfMessageFont)
        } else {
            HFONT::default()
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let shared = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Shared;
        if shared.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let shared = &*shared;
        match msg {
            WM_COMMAND => {
                let id = wparam.0 & 0xFFFF;
                let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
                let event = match id {
                    ID_OK => Some(Event::Submit),
                    ID_CANCEL => Some(Event::Close),
                    _ if id >= FIRST_ID => {
                        let index = id - FIRST_ID;
                        let kind = shared.kinds.borrow().get(index).copied();
                        let control = ControlId(index);
                        match (kind, code) {
                            (Some(Kind::Button | Kind::Check), BN_CLICKED) => {
                                Some(Event::Clicked(control))
                            }
                            (Some(Kind::Edit), EN_CHANGE)
                            | (Some(Kind::DropDown | Kind::List), SELCHANGE)
                                if !shared.muted.get() =>
                            {
                                Some(Event::Changed(control))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(event) = event {
                    shared.events.borrow_mut().push_back(event);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                shared.events.borrow_mut().push_back(Event::Close);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
