use std::{
    cell::RefCell,
    io::{BufWriter, Write},
    os::unix::prelude::AsRawFd,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use libdecor::Context;
use wayland_client::{
    protocol::{
        wl_compositor,
        wl_keyboard::{self},
        wl_pointer::{self, ButtonState},
        wl_seat::{self, Capability},
        wl_shm, wl_surface,
    },
    Display, GlobalManager, Main,
};
use wayland_cursor::CursorTheme;

const CHK: i32 = 16;
const DEFAULT_WIDTH: i32 = 30 * CHK;
const DEFAULT_HEIGHT: i32 = 20 * CHK;

fn redraw(
    shm: &wl_shm::WlShm,
    surface: &wl_surface::WlSurface,
    width: i32,
    height: i32,
    window_state: Option<libdecor::WindowState>,
) {
    // create a tempfile to write the contents of the window on
    let mut tmp = tempfile::tempfile().expect("Unable to create a tempfile.");

    // write the contents to it
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

    compositor.quick_assign(|_, _request, _| todo!());

    let content_surface = compositor.create_surface();

    content_surface.quick_assign(|_, request, _| match request {
        wl_surface::Event::Enter { .. } => {}
        wl_surface::Event::Leave { .. } => {}
        _ => unreachable!(),
    });

    let shm = Rc::new(globals.instantiate_exact::<wl_shm::WlShm>(1).unwrap());

    shm.quick_assign(|_, request, _| match request {
        wl_shm::Event::Format { .. } => {}
        _ => unreachable!(),
    });

    let exit = Rc::new(AtomicBool::new(false));

    let (mut floating_width, mut floating_height) = (DEFAULT_WIDTH, DEFAULT_HEIGHT);

    let context = Context::new(display, |request| match request {
        libdecor::Request::Error(error) => {
            eprintln!("libdecor error: {}", error);
            std::process::exit(1);
        }
        _ => unreachable!(),
    });

    let frame = Rc::new(
        context
            .decorate(content_surface.detach(), {
                let exit = exit.clone();
                let shm = shm.clone();
                let content_surface = content_surface.detach();
                move |frame, request| match request {
                    libdecor::FrameRequest::Configure(configuration) => {
                        let (width, height) = if let Some(size) = configuration.content_size(&frame)
                        {
                            size
                        } else {
                            (floating_width, floating_height)
                        };

                        let window_state = configuration.window_state();

                        let state = libdecor::State::new(width, height);
                        frame.commit(&state, Some(&configuration));

                        if frame.is_floating() {
                            floating_width = width;
                            floating_height = height;
                        }

                        redraw(&shm, &content_surface, width, height, window_state);
                    }
                    libdecor::FrameRequest::Close => {
                        exit.store(true, Ordering::SeqCst);
                    }
                    libdecor::FrameRequest::Commit => {
                        content_surface.commit();
                    }
                    libdecor::FrameRequest::DismissPopup { .. } => eprintln!("DismissPopup called"),
                    _ => unreachable!(),
                }
            })
            .expect("Failed to create frame"),
    );

    let pointer: Rc<RefCell<Option<Main<wl_pointer::WlPointer>>>> = Rc::new(RefCell::new(None));
    let keyboard: Rc<RefCell<Option<Main<wl_keyboard::WlKeyboard>>>> = Rc::new(RefCell::new(None));
    let seat_name: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let seat = globals.instantiate_exact::<wl_seat::WlSeat>(5).unwrap();
    seat.quick_assign({
        let pointer = pointer.clone();
        let keyboard = keyboard.clone();
        let seat_name = seat_name.clone();
        move |seat, request, _| match request {
            wl_seat::Event::Capabilities { capabilities } => {
                if capabilities.contains(Capability::Keyboard) {
                    let mut keyboard = keyboard.borrow_mut();
                    (*keyboard) = Some(seat.get_keyboard());
                }
                if capabilities.contains(Capability::Pointer) {
                    let mut pointer = pointer.borrow_mut();
                    (*pointer) = Some(seat.get_pointer());
                }
            }
            wl_seat::Event::Name { name } => {
                let mut seat_name = seat_name.borrow_mut();
                (*seat_name) = Some(name);
            }
            _ => unreachable!(),
        }
    });

    event_queue
        .sync_roundtrip(&mut (), |_, _, _| unreachable!())
        .unwrap();

    let mut cursor_theme = CursorTheme::load(24, &shm);
    let cursor = cursor_theme
        .get_cursor("left_ptr")
        .expect("Cursor not provided by theme");
    let cursor_buffer = &cursor[0];
    let cursor_surface = compositor.create_surface();
    let cursor_buffer_dimension = cursor_buffer.dimensions();
    cursor_surface.quick_assign(|_, request, _| match request {
        wl_surface::Event::Enter { .. } => {}
        wl_surface::Event::Leave { .. } => {}
        _ => unreachable!(),
    });

    cursor_surface.attach(Some(cursor_buffer), 0, 0);
    cursor_surface.set_buffer_scale(1);
    cursor_surface.damage_buffer(
        0,
        0,
        cursor_buffer_dimension.0 as i32,
        cursor_buffer_dimension.1 as i32,
    );
    cursor_surface.commit();

    let has_pointer_focus = AtomicBool::new(false);

    if let Some(pointer) = &*pointer.borrow() {
        pointer.quick_assign({
            let content_surface = content_surface.detach();
            let frame = frame.clone();
            move |pointer, request, _| match request {
                wl_pointer::Event::Enter {
                    surface, serial, ..
                } => {
                    if surface.as_ref() != content_surface.as_ref() {
                        return;
                    }

                    has_pointer_focus.store(true, Ordering::SeqCst);
                    pointer.set_cursor(serial, Some(&cursor_surface), 0, 0);
                }
                wl_pointer::Event::Leave { surface, .. } => {
                    if surface.as_ref() != content_surface.as_ref() {
                        return;
                    }

                    has_pointer_focus.store(false, Ordering::SeqCst);
                }
                wl_pointer::Event::Motion { .. } => {}
                wl_pointer::Event::Button {
                    button,
                    state,
                    serial,
                    ..
                } => {
                    if !has_pointer_focus.load(Ordering::SeqCst) {
                        return;
                    }

                    if button == 0x110 && state == ButtonState::Pressed {
                        frame._move(&seat, serial)
                    }
                }
                wl_pointer::Event::Axis { .. } => {}
                wl_pointer::Event::Frame => {}
                wl_pointer::Event::AxisSource { .. } => {}
                wl_pointer::Event::AxisStop { .. } => {}
                wl_pointer::Event::AxisDiscrete { .. } => {}
                _ => unreachable!(),
            }
        });
    }

    if let Some(keyboard) = &*keyboard.borrow() {
        keyboard.quick_assign(|_, request, _| match request {
            wl_keyboard::Event::Keymap { .. } => {}
            wl_keyboard::Event::Enter { .. } => {}
            wl_keyboard::Event::Leave { .. } => {}
            wl_keyboard::Event::Key { .. } => {}
            wl_keyboard::Event::Modifiers { .. } => {}
            wl_keyboard::Event::RepeatInfo { .. } => {}
            _ => unreachable!(),
        })
    }

    frame.set_app_id("libdecor-rs-demo");
    frame.set_title("libdecor-rs demo");
    frame.map();

    while context.dispatch(Some(Duration::from_millis(16))) {
        if exit.load(Ordering::SeqCst) {
            break;
        }

        event_queue
            .dispatch(&mut (), |_, _, _| unreachable!())
            .unwrap();
    }
}
