use std::{
    cell::RefCell,
    fs::File,
    io::{BufWriter, Read, Write},
    os::unix::prelude::{AsRawFd, FromRawFd},
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration,
};

use libdecor::{Capabilities, Context, FrameRef, State, WindowState};
use wayland_client::{
    protocol::{wl_compositor, wl_keyboard, wl_pointer, wl_seat, wl_shm, wl_surface},
    Display, GlobalManager, Main,
};
use wayland_cursor::CursorTheme;
use wayland_protocols::xdg_shell::client::{xdg_popup, xdg_positioner, xdg_surface, xdg_wm_base};
use xkbcommon::xkb;

const CHK: i32 = 16;
const DEFAULT_WIDTH: i32 = 30 * CHK;
const DEFAULT_HEIGHT: i32 = 20 * CHK;
const POPUP_WIDTH: i32 = 100;
const POPUP_HEIGHT: i32 = 300;

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;

static TITLES: &[&str] = &[
    "Hello!",
    "Hallå!",
    "Привет!",
    "Γειά σου!",
    "שלום!",
    "你好！",
    "สวัสดี!",
    "こんにちは！",
    "👻❤️🤖➕🍰",
];

fn redraw(
    shm: &wl_shm::WlShm,
    surface: &wl_surface::WlSurface,
    width: i32,
    height: i32,
    window_state: Option<libdecor::WindowState>,
) {
    let mut tmp = tempfile::tempfile().expect("Unable to create a tempfile.");

    {
        let is_active = window_state
            .map(|s| s.contains(libdecor::WindowState::ACTIVE))
            .unwrap_or(false);

        let colors: (u32, u32) = if is_active {
            (0xffbcbcbc, 0xff8e8e8e)
        } else {
            (0xff8e8e8e, 0xff484848)
        };

        let mut buf = BufWriter::new(&mut tmp);
        for y in 0..height {
            for x in 0..width {
                let color = if (x & CHK) ^ (y & CHK) > 0 {
                    colors.0
                } else {
                    colors.1
                };
                buf.write_all(&color.to_ne_bytes()).unwrap();
            }
        }
        buf.flush().unwrap();
    }

    let pool = shm.create_pool(tmp.as_raw_fd(), (width * height * 4) as i32);
    let buffer = pool.create_buffer(
        0,
        width as i32,
        height as i32,
        (width * 4) as i32,
        wl_shm::Format::Argb8888,
    );

    buffer.quick_assign(|_, request, _| match request {
        wayland_client::protocol::wl_buffer::Event::Release => {}
        _ => unreachable!(),
    });

    surface.attach(Some(&buffer), 0, 0);
    surface.set_buffer_scale(1);
    surface.damage_buffer(0, 0, width, height);
    surface.commit();
}

fn popup_configure(
    shm: &wl_shm::WlShm,
    surface: &wl_surface::WlSurface,
    xdg_surface: &xdg_surface::XdgSurface,
    width: i32,
    height: i32,
    serial: u32,
) {
    let mut tmp = tempfile::tempfile().expect("Unable to create a tempfile.");

    {
        let color: u32 = 0xff4455ff;

        let mut buf = BufWriter::new(&mut tmp);
        for _ in 0..(width * height) {
            buf.write_all(&color.to_ne_bytes()).unwrap();
        }
        buf.flush().unwrap();
    }

    let pool = shm.create_pool(tmp.as_raw_fd(), (width * height * 4) as i32);
    let buffer = pool.create_buffer(
        0,
        width as i32,
        height as i32,
        (width * 4) as i32,
        wl_shm::Format::Argb8888,
    );

    buffer.quick_assign(|_, _, _| {});

    surface.attach(Some(&buffer), 0, 0);
    surface.set_buffer_scale(1);
    surface.damage_buffer(0, 0, width, height);
    xdg_surface.ack_configure(serial);
    surface.commit();
}

struct DemoState {
    window: Window,
    popup: Option<Popup>,
    seat_name: Option<String>,
    exit: AtomicBool,
    title_index: AtomicUsize,
}

struct Window {
    configured_size: (i32, i32),
    floating_size: (i32, i32),
    content_surface: Main<wl_surface::WlSurface>,
    window_state: Option<WindowState>,
    has_pointer_focus: AtomicBool,
    pointer_position: (i32, i32),
}

struct Popup {
    surface: wl_surface::WlSurface,
    xdg_surface: xdg_surface::XdgSurface,
    xdg_popup: xdg_popup::XdgPopup,
}

impl Popup {
    fn destroy(self) {
        self.xdg_popup.destroy();
        self.xdg_surface.destroy();
        self.surface.destroy();
    }
}

fn resize(
    frame: &FrameRef,
    shm: &wl_shm::WlShm,
    surface: &wl_surface::WlSurface,
    width: i32,
    height: i32,
    window_state: Option<libdecor::WindowState>,
) -> bool {
    if height <= 0 || width <= 0 {
        eprintln!("... ignoring resize to 0");
        return false;
    }

    if !frame.is_floating() {
        eprintln!("... ignoring in non-floating mode");
        return false;
    }

    let state = State::new(width, height);
    frame.commit(&state, None);

    redraw(shm, surface, width, height, window_state);

    true
}

fn main() {
    let display = Display::connect_to_env().unwrap();
    let mut event_queue = display.create_event_queue();
    let attached_display = (*display).clone().attach(event_queue.token());
    let globals = GlobalManager::new(&attached_display);
    event_queue
        .sync_roundtrip(&mut (), |_, _, _| unreachable!())
        .unwrap();

    let compositor = globals
        .instantiate_exact::<wl_compositor::WlCompositor>(4)
        .unwrap();

    let xdg_wm_base = globals
        .instantiate_exact::<xdg_wm_base::XdgWmBase>(1)
        .expect("Missing xdg_wm_base");

    let content_surface = compositor.create_surface();
    content_surface.quick_assign(|_, _, _| {});

    let mut demo_state = DemoState {
        window: Window {
            configured_size: (DEFAULT_WIDTH, DEFAULT_HEIGHT),
            floating_size: (DEFAULT_WIDTH, DEFAULT_HEIGHT),
            content_surface,
            window_state: None,
            has_pointer_focus: AtomicBool::new(false),
            pointer_position: (0, 0),
        },
        popup: None,
        seat_name: None,
        exit: AtomicBool::new(false),
        title_index: AtomicUsize::new(0),
    };

    let shm = Rc::new(globals.instantiate_exact::<wl_shm::WlShm>(1).unwrap());

    shm.quick_assign(|_, _, _| {});

    let context = Context::new(display, |request| match request {
        libdecor::Request::Error(error) => {
            eprintln!("libdecor error: {}", error);
            std::process::exit(1);
        }
        _ => unreachable!(),
    });

    let pointer: Rc<RefCell<Option<Main<wl_pointer::WlPointer>>>> = Rc::new(RefCell::new(None));
    let keyboard: Rc<RefCell<Option<Main<wl_keyboard::WlKeyboard>>>> = Rc::new(RefCell::new(None));
    let seat = globals.instantiate_exact::<wl_seat::WlSeat>(5).unwrap();
    seat.quick_assign({
        let pointer = pointer.clone();
        let keyboard = keyboard.clone();
        move |seat, request, mut data| match request {
            wl_seat::Event::Capabilities { capabilities } => {
                if capabilities.contains(wl_seat::Capability::Keyboard) {
                    let mut keyboard = keyboard.borrow_mut();
                    (*keyboard) = Some(seat.get_keyboard());
                }
                if capabilities.contains(wl_seat::Capability::Pointer) {
                    let mut pointer = pointer.borrow_mut();
                    (*pointer) = Some(seat.get_pointer());
                }
            }
            wl_seat::Event::Name { name } => {
                let state = data.get::<DemoState>().unwrap();
                state.seat_name = Some(name);
            }
            _ => unreachable!(),
        }
    });

    event_queue
        .sync_roundtrip(&mut demo_state, |_, _, _| unreachable!())
        .unwrap();

    let frame = Rc::new(
        context
            .decorate(demo_state.window.content_surface.detach(), {
                let shm = shm.clone();
                move |frame, request, mut ddata| {
                    let mut demo_state = ddata.get::<DemoState>().unwrap();
                    match request {
                        libdecor::FrameRequest::Configure(configuration) => {
                            let size = configuration
                                .content_size(frame)
                                .unwrap_or(demo_state.window.floating_size);
                            demo_state.window.configured_size = size;

                            if let Some(state) = configuration.window_state() {
                                demo_state.window.window_state = Some(state);
                            }

                            let state = libdecor::State::new(size.0, size.1);
                            frame.commit(&state, Some(configuration));

                            if frame.is_floating() {
                                demo_state.window.floating_size = size;
                            }

                            redraw(
                                &shm,
                                &demo_state.window.content_surface,
                                size.0,
                                size.1,
                                demo_state.window.window_state,
                            );
                        }
                        libdecor::FrameRequest::Close => {
                            demo_state.exit.store(true, Ordering::SeqCst);
                        }
                        libdecor::FrameRequest::Commit => {
                            demo_state.window.content_surface.commit();
                        }
                        libdecor::FrameRequest::DismissPopup { .. } => {
                            if let Some(popup) = demo_state.popup.take() {
                                frame.popup_ungrab(demo_state.seat_name.as_ref().unwrap());
                                popup.destroy();
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            })
            .expect("Failed to create frame"),
    );

    frame.dispatch(&mut demo_state, |f| {
        f.set_app_id("libdecor-rs-demo");
        f.set_title("libdecor-rs demo");
    });

    let mut cursor_theme = CursorTheme::load(24, &shm);
    let cursor = cursor_theme
        .get_cursor("left_ptr")
        .expect("Cursor not provided by theme");
    let cursor_buffer = cursor[0].clone();
    let cursor_surface = compositor.create_surface();
    let cursor_buffer_dimension = cursor_buffer.dimensions();
    cursor_surface.quick_assign(|_, _, _| {});

    if let Some(pointer) = &*pointer.borrow() {
        pointer.quick_assign({
            let shm = shm.clone();
            let frame = frame.clone();
            move |pointer, request, mut ddata| {
                let demo_state = ddata.get::<DemoState>().unwrap();

                match request {
                wl_pointer::Event::Enter {
                    surface,
                    serial,
                    surface_x,
                    surface_y,
                } => {
                    if surface.as_ref() != demo_state.window.content_surface.as_ref() {
                        return;
                    }

                    demo_state.window.has_pointer_focus.store(true, Ordering::SeqCst);
                    demo_state.window.pointer_position = (surface_x as i32, surface_y as i32);

                    pointer.set_cursor(serial, Some(&cursor_surface), 0, 0);
                    cursor_surface.attach(Some(&cursor_buffer), 0, 0);
                    cursor_surface.set_buffer_scale(1);
                    cursor_surface.damage_buffer(
                        0,
                        0,
                        cursor_buffer_dimension.0 as i32,
                        cursor_buffer_dimension.1 as i32,
                    );
                    cursor_surface.commit();
                }
                wl_pointer::Event::Leave { surface, .. } => {
                    if surface.as_ref() != demo_state.window.content_surface.as_ref() {
                        return;
                    }

                    demo_state.window.has_pointer_focus.store(false, Ordering::SeqCst);
                }
                wl_pointer::Event::Motion {
                    surface_x,
                    surface_y,
                    ..
                } => {
                    demo_state.window.pointer_position = (surface_x as i32, surface_y as i32);
                }
                wl_pointer::Event::Button {
                    button,
                    state,
                    serial,
                    ..
                } => {
                    if !demo_state.window.has_pointer_focus.load(Ordering::SeqCst) {
                        return;
                    }

                    if state != wl_pointer::ButtonState::Pressed {
                        return;
                    }

                    if let Some(popup) = demo_state.popup.take() {
                        frame.dispatch(demo_state, {
                            let seat_name = demo_state.seat_name.clone();
                            move |f| {
                            f.popup_ungrab(seat_name.as_ref().unwrap());
                        }});
                        popup.destroy();
                    }

                    match button {
                        BTN_LEFT => {
                            frame.dispatch(demo_state, |f| {
                                f._move(&seat, serial);
                            });
                        }
                        BTN_MIDDLE => {
                            frame.dispatch(demo_state, {
                                let pointer_position = demo_state.window.pointer_position;
                                let seat = seat.clone();
                                move |f| {
                                f.show_window_menu(
                                    &seat,
                                    serial,
                                    pointer_position.0,
                                    pointer_position.1,
                                );
                            }});
                        }
                        BTN_RIGHT => {
                            let popup_surface = compositor.create_surface();
                            popup_surface.quick_assign(|_, _, _| {});
                            let xdg_surface = xdg_wm_base.get_xdg_surface(&popup_surface);
                            xdg_surface.quick_assign({
                                let popup_surface = popup_surface.detach();
                                let shm = shm.clone();
                                move |xdg_surface, request, _| {
                                match request {
                                    wayland_protocols::xdg_shell::client::xdg_surface::Event::Configure { serial } => {
                                        popup_configure(&shm, &popup_surface, &xdg_surface, POPUP_WIDTH, POPUP_HEIGHT, serial);
                                    },
                                    _ => unreachable!()
                                }
                            }});
                            let parent = frame.dispatch(demo_state, |f| {
                                f.xdg_surface().expect("Frame without xdg_surface")
                            });
                            let positioner = xdg_wm_base.create_positioner();
                            positioner.set_size(POPUP_WIDTH, POPUP_HEIGHT);
                            let (x, y) = frame.dispatch(demo_state, {
                                let pointer_position = demo_state.window.pointer_position;
                                move |f| {
                                f.translate_coordinate(pointer_position.0, pointer_position.1)
                            }});
                            positioner.set_anchor_rect(x, y, 1, 1);
                            positioner.set_constraint_adjustment(
                                (xdg_positioner::ConstraintAdjustment::FlipY
                                    | xdg_positioner::ConstraintAdjustment::SlideX)
                                    .to_raw(),
                            );
                            positioner.set_anchor(xdg_positioner::Anchor::BottomRight);
                            positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
                            let xdg_popup = xdg_surface.get_popup(Some(&parent), &positioner);
                            xdg_popup.quick_assign({
                                let frame = frame.clone();
                                move |_, request, mut ddata| {
                                    let demo_state = ddata.get::<DemoState>().unwrap();
                                    match request {
                                        wayland_protocols::xdg_shell::client::xdg_popup::Event::Configure { .. } => {},
                                        wayland_protocols::xdg_shell::client::xdg_popup::Event::PopupDone => {
                                            if let Some(popup) = demo_state.popup.take() {
                                                frame.dispatch(demo_state, {
                                                    let seat_name = demo_state.seat_name.clone();
                                                    move  |f| {
                                                    f.popup_ungrab(seat_name.as_ref().unwrap());
                                                }});
                                                popup.destroy();
                                            }
                                        },
                                        wayland_protocols::xdg_shell::client::xdg_popup::Event::Repositioned { .. } => {},
                                        _ => unreachable!(),
                                    }
                            }});
                            positioner.destroy();
                            xdg_popup.grab(&seat, serial);
                            popup_surface.commit();
                            frame.dispatch(demo_state, {
                                let seat_name = demo_state.seat_name.clone();
                                move |f| {
                                f.popup_grab(seat_name.as_ref().unwrap());
                            }});
                            demo_state.popup = Some(Popup {
                                surface: popup_surface.detach(),
                                xdg_popup: xdg_popup.detach(),
                                xdg_surface: xdg_surface.detach(),
                            })
                        }
                        _ => {}
                    }
                }
                wl_pointer::Event::Axis { .. } => {}
                wl_pointer::Event::Frame => {}
                wl_pointer::Event::AxisSource { .. } => {}
                wl_pointer::Event::AxisStop { .. } => {}
                wl_pointer::Event::AxisDiscrete { .. } => {}
                _ => unreachable!(),
            }
        }});
    }

    let xkb_context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let xkb_state: RefCell<Option<xkb::State>> = RefCell::new(None);

    if let Some(keyboard) = &*keyboard.borrow() {
        keyboard.quick_assign({
            let frame = frame.clone();
            move |_, request, mut ddata| {
                let demo_state = ddata.get::<DemoState>().unwrap();
                match request {
                    wl_keyboard::Event::Keymap { format, fd, size } => match format {
                        wl_keyboard::KeymapFormat::XkbV1 => {
                            let mut fd = unsafe { File::from_raw_fd(fd) };
                            let mut buffer = vec![0; (size as usize) - 1];
                            fd.read_exact(&mut buffer).expect("Failed to read keymap");
                            let keymap = String::from_utf8(buffer).expect("Failed to read keymap");
                            let keymap = xkb::Keymap::new_from_string(
                                &xkb_context,
                                keymap,
                                xkb::FORMAT_TEXT_V1,
                                xkb::COMPILE_NO_FLAGS,
                            );

                            if let Some(keymap) = keymap {
                                let mut xkb_state = xkb_state.borrow_mut();
                                *xkb_state = Some(xkb::State::new(&keymap));
                            }
                        }
                        wl_keyboard::KeymapFormat::NoKeymap => {
                            panic!("NoKeymap not supported");
                        }
                        _ => unreachable!(),
                    },
                    wl_keyboard::Event::Enter { .. } => {}
                    wl_keyboard::Event::Leave { .. } => {}
                    wl_keyboard::Event::Key { key, state, .. } => {
                        if state != wl_keyboard::KeyState::Pressed {
                            return;
                        }

                        if let Some(xkb_state) = &*xkb_state.borrow() {
                            let key_sym = xkb_state.key_get_syms(key + 8);

                            match key_sym[0] {
                                xkb::KEY_Escape => {
                                    frame.dispatch(demo_state, |f| f.close());
                                }
                                xkb::KEY_1 => {
                                    let resize_enabled = frame.dispatch(demo_state, |f| {
                                        f.has_capability(Capabilities::RESIZE)
                                    });

                                    if resize_enabled {
                                        eprintln!("set fixed-size");
                                        frame.dispatch(demo_state, |f| {
                                            f.unset_capabilities(Capabilities::RESIZE);
                                        });
                                    } else {
                                        eprintln!("set resizeable");
                                        frame.dispatch(demo_state, |f| {
                                            f.set_capabilities(Capabilities::RESIZE);
                                        });
                                    }
                                }
                                xkb::KEY_2 => {
                                    eprintln!("maximize");
                                    frame.dispatch(demo_state, |f| {
                                        f.set_maximized();
                                    });
                                }
                                xkb::KEY_3 => {
                                    eprintln!("un-maximize");
                                    frame.dispatch(demo_state, |f| {
                                        f.unset_maximized();
                                    });
                                }
                                xkb::KEY_4 => {
                                    eprintln!("fullscreen");
                                    frame.dispatch(demo_state, |f| {
                                        f.set_fullscreen(None);
                                    });
                                }
                                xkb::KEY_5 => {
                                    eprintln!("un-fullscreen");
                                    frame.dispatch(demo_state, |f| {
                                        f.unset_fullscreen();
                                    });
                                }
                                xkb::KEY_minus | xkb::KEY_plus => {
                                    let dd = CHK / 2;
                                    let dd = if key_sym[0] == xkb::KEY_minus {
                                        -dd
                                    } else {
                                        dd
                                    };
                                    let (width, height) = {
                                        let (configured_width, configured_height) =
                                            demo_state.window.configured_size;
                                        (configured_width + dd, configured_height + dd)
                                    };
                                    eprintln!("resize to: {} x {}", width, height);
                                    let resized = frame.dispatch(demo_state, {
                                        let shm = shm.clone();
                                        let window_state = demo_state.window.window_state;
                                        let content_surface =
                                            demo_state.window.content_surface.clone();
                                        move |d| {
                                            resize(
                                                d,
                                                &shm,
                                                &content_surface,
                                                width,
                                                height,
                                                window_state,
                                            )
                                        }
                                    });
                                    if resized {
                                        demo_state.window.floating_size = (width, height);
                                        demo_state.window.configured_size = (width, height);
                                    }
                                }
                                xkb::KEY_t => {
                                    let current_title_index =
                                        demo_state.title_index.load(Ordering::SeqCst);
                                    eprintln!("Changing title to: {}", TITLES[current_title_index]);
                                    frame.dispatch(demo_state, |f| {
                                        f.set_title(TITLES[current_title_index]);
                                    });
                                    demo_state.title_index.store(
                                        (current_title_index + 1) % TITLES.len(),
                                        Ordering::SeqCst,
                                    );
                                }
                                xkb::KEY_v => {
                                    eprintln!("set VGA resolution: 640x480");
                                    let size = (640, 480);
                                    let resized = frame.dispatch(demo_state, {
                                        let shm = shm.clone();
                                        let window_state = demo_state.window.window_state;
                                        let content_surface =
                                            demo_state.window.content_surface.clone();
                                        move |d| {
                                            resize(
                                                d,
                                                &shm,
                                                &content_surface,
                                                size.0,
                                                size.1,
                                                window_state,
                                            )
                                        }
                                    });
                                    if resized {
                                        demo_state.window.floating_size = size;
                                        demo_state.window.configured_size = size;
                                    }
                                }
                                xkb::KEY_s => {
                                    eprintln!("set SVGA resolution: 800x600");
                                    let size = (800, 600);
                                    let resized = frame.dispatch(demo_state, {
                                        let shm = shm.clone();
                                        let window_state = demo_state.window.window_state;
                                        let content_surface =
                                            demo_state.window.content_surface.clone();
                                        move |d| {
                                            resize(
                                                d,
                                                &shm,
                                                &content_surface,
                                                size.0,
                                                size.1,
                                                window_state,
                                            )
                                        }
                                    });
                                    if resized {
                                        demo_state.window.floating_size = size;
                                        demo_state.window.configured_size = size;
                                    }
                                }
                                xkb::KEY_x => {
                                    eprintln!("set XVGA resolution: 1024x768");
                                    let size = (1024, 768);
                                    let resized = frame.dispatch(demo_state, {
                                        let shm = shm.clone();
                                        let window_state = demo_state.window.window_state;
                                        let content_surface =
                                            demo_state.window.content_surface.clone();
                                        move |d| {
                                            resize(
                                                d,
                                                &shm,
                                                &content_surface,
                                                size.0,
                                                size.1,
                                                window_state,
                                            )
                                        }
                                    });
                                    if resized {
                                        demo_state.window.floating_size = size;
                                        demo_state.window.configured_size = size;
                                    }
                                }
                                xkb::KEY_h => {
                                    frame.dispatch(demo_state, |f| {
                                        f.set_visibility(!f.is_visible());
                                        if f.is_visible() {
                                            eprintln!("decorations visible");
                                        } else {
                                            eprintln!("decorations hidden");
                                        }
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                    wl_keyboard::Event::Modifiers { .. } => {}
                    wl_keyboard::Event::RepeatInfo { .. } => {}
                    _ => unreachable!(),
                }
            }
        })
    }

    frame.dispatch(&mut demo_state, |f| f.map());

    while context.dispatch(&mut demo_state, Some(Duration::from_millis(16))) {
        if demo_state.exit.load(Ordering::SeqCst) {
            break;
        }

        event_queue
            .dispatch(&mut demo_state, |_, _, _| unreachable!())
            .unwrap();
    }
}
