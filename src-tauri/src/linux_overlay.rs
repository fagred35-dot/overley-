//! Linux overlay surface rendered with GTK/Cairo.
//! X11 uses override_redirect and an empty input region; Wayland uses layer-shell.

use crate::config::{NativePrimitive, OverlayConfig, PresetLayer};
use cairo::{Antialias, Context, FillRule, LineCap, LineJoin, Operator};
use gdkx11::{X11Display, X11Window};
use glib::prelude::*;
use glib::translate::ToGlibPtr;
use gtk::prelude::*;
use std::{cell::RefCell, ffi::CString, rc::Rc};
use tauri::AppHandle;

thread_local! {
    static NATIVE_OVERLAY: RefCell<Option<NativeOverlay>> = RefCell::new(None);
}

struct NativeOverlay {
    window: gtk::Window,
    config: Rc<RefCell<OverlayConfig>>,
    layers: Rc<RefCell<Vec<PresetLayer>>>,
}

#[link(name = "gtk-layer-shell")]
extern "C" {
    fn gtk_layer_is_supported() -> glib_sys::gboolean;
    fn gtk_layer_init_for_window(window: *mut gtk_sys::GtkWindow);
    fn gtk_layer_set_layer(window: *mut gtk_sys::GtkWindow, layer: i32);
    fn gtk_layer_set_anchor(
        window: *mut gtk_sys::GtkWindow,
        edge: i32,
        anchor_to_edge: glib_sys::gboolean,
    );
    fn gtk_layer_set_exclusive_zone(window: *mut gtk_sys::GtkWindow, zone: i32);
    fn gtk_layer_set_keyboard_interactivity(
        window: *mut gtk_sys::GtkWindow,
        interactivity: glib_sys::gboolean,
    );
    fn gtk_layer_set_namespace(window: *mut gtk_sys::GtkWindow, namespace: *const i8);
    fn gtk_layer_set_monitor(
        window: *mut gtk_sys::GtkWindow,
        monitor: *mut gdk_sys::GdkMonitor,
    );
}

pub fn create(config: OverlayConfig, layers: Vec<PresetLayer>) -> Result<(), String> {
    if !gtk::is_initialized() {
        gtk::init().map_err(|error| format!("GTK не смог запуститься: {error}"))?;
    }

    let display = gdk::Display::default()
        .ok_or_else(|| "Не удалось определить графический дисплей Linux".to_owned())?;
    let backend_name = display.type_().name().to_ascii_lowercase();
    let is_wayland = backend_name.contains("wayland");
    let is_x11 = backend_name.contains("x11");
    if !is_wayland && !is_x11 {
        return Err(format!(
            "Неподдерживаемый графический backend Linux: {}",
            display.type_().name()
        ));
    }

    let monitor = display
        .primary_monitor()
        .ok_or_else(|| "Не удалось определить основной монитор".to_owned())?;
    let geometry = monitor.geometry();
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("NOVA Crosshair overlay");
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_accept_focus(false);
    window.set_focus_on_map(false);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);
    window.set_app_paintable(true);
    window.set_default_size(geometry.width(), geometry.height());
    window.style_context().add_class("nova-crosshair-overlay");

    if let Some(screen) = window.screen() {
        if let Some(visual) = screen.rgba_visual() {
            window.set_visual(Some(&visual));
        }
        let css = gtk::CssProvider::new();
        css.load_from_data(
            b"window.nova-crosshair-overlay { background-color: rgba(0,0,0,0); background-image: none; }",
        )
        .map_err(|error| format!("Не удалось настроить прозрачность GTK: {error}"))?;
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let should_show = config.enabled;
    let native_config = Rc::new(RefCell::new(config));
    let native_layers = Rc::new(RefCell::new(layers));
    let draw_config = Rc::clone(&native_config);
    let draw_layers = Rc::clone(&native_layers);
    window.connect_draw(move |widget, context| {
        let allocation = widget.allocation();
        if let (Ok(config), Ok(layers)) = (draw_config.try_borrow(), draw_layers.try_borrow()) {
            draw_reticle(
                context,
                allocation.width() as f64,
                allocation.height() as f64,
                &config,
                &layers,
            );
        }
        glib::Propagation::Proceed
    });

    let raw_window: *mut gtk_sys::GtkWindow = window.to_glib_none().0;
    if is_wayland {
        if unsafe { gtk_layer_is_supported() } == 0 {
            return Err(
                "Wayland-композитор не предоставляет layer-shell. Используйте X11 или layer-shell-композитор (например, Sway/Hyprland).".to_owned(),
            );
        }
        let namespace = CString::new("nova-crosshair").expect("static namespace has no NUL");
        unsafe {
            gtk_layer_init_for_window(raw_window);
            gtk_layer_set_layer(raw_window, 3); // GTK_LAYER_SHELL_LAYER_OVERLAY
            for edge in 0..4 {
                gtk_layer_set_anchor(raw_window, edge, 1);
            }
            gtk_layer_set_exclusive_zone(raw_window, 0);
            gtk_layer_set_keyboard_interactivity(raw_window, 0);
            gtk_layer_set_namespace(raw_window, namespace.as_ptr());
            gtk_layer_set_monitor(raw_window, monitor.to_glib_none().0);
        }
    } else {
        window.move_(geometry.x(), geometry.y());
    }

    // Realize before mapping so the X11 window manager sees override_redirect at map time.
    window.realize();
    let gdk_window = window
        .window()
        .ok_or_else(|| "GTK не создал поверхность оверлея".to_owned())?;
    gdk_window.set_pass_through(true);

    if is_x11 {
        let x11_window = gdk_window
            .downcast::<X11Window>()
            .map_err(|_| "GDK не предоставил X11-окно для override_redirect".to_owned())?;
        let x11_display = display
            .downcast::<X11Display>()
            .map_err(|_| "GDK не предоставил X11-дисплей".to_owned())?;
        let raw_display = unsafe { x11_display.xdisplay() };
        if raw_display.is_null() {
            return Err("Не удалось получить соединение с X11".to_owned());
        }
        unsafe {
            let mut attributes: x11::xlib::XSetWindowAttributes = std::mem::zeroed();
            attributes.override_redirect = 1;
            x11::xlib::XChangeWindowAttributes(
                raw_display,
                x11_window.xid(),
                x11::xlib::CWOverrideRedirect,
                &mut attributes,
            );
            x11::xlib::XFlush(raw_display);
        }
    }

    NATIVE_OVERLAY.with(|slot| {
        *slot.borrow_mut() = Some(NativeOverlay {
            window: window.clone(),
            config: native_config,
            layers: native_layers,
        });
    });
    if should_show {
        window.show_all();
        window.queue_draw();
    } else {
        window.hide();
    }
    Ok(())
}

pub fn set_state(
    app: AppHandle,
    config: OverlayConfig,
    layers: Vec<PresetLayer>,
) -> tauri::Result<()> {
    app.run_on_main_thread(move || {
        NATIVE_OVERLAY.with(|slot| {
            if let Some(overlay) = slot.borrow().as_ref() {
                if let Ok(mut current) = overlay.config.try_borrow_mut() {
                    *current = config;
                }
                if let Ok(mut current) = overlay.layers.try_borrow_mut() {
                    *current = layers;
                }
                overlay.window.queue_draw();
                let enabled = overlay
                    .config
                    .try_borrow()
                    .map(|config| config.enabled)
                    .unwrap_or(false);
                if enabled {
                    overlay.window.show_all();
                } else {
                    overlay.window.hide();
                }
            }
        });
    })
}

fn draw_reticle(
    context: &Context,
    width: f64,
    height: f64,
    config: &OverlayConfig,
    layers: &[PresetLayer],
) {
    let _ = context.save();
    context.set_operator(Operator::Clear);
    let _ = context.paint();
    context.set_operator(Operator::Over);
    context.set_antialias(Antialias::Best);
    context.set_line_cap(LineCap::Square);
    context.set_line_join(LineJoin::Round);
    context.translate(width / 2.0, height / 2.0);
    let scale = config.size / 100.0;
    context.scale(scale, scale);
    context.rotate(config.rotation.to_radians());
    context.translate(-50.0, -50.0);

    for layer in layers {
        let _ = context.save();
        if layer.gap_mask && config.gap > 0.0 {
            context.new_path();
            context.rectangle(0.0, 0.0, 100.0, 100.0);
            context.arc(50.0, 50.0, config.gap, 0.0, std::f64::consts::TAU);
            context.set_fill_rule(FillRule::EvenOdd);
            let _ = context.clip();
            context.set_fill_rule(FillRule::Winding);
        }
        for primitive in &layer.native {
            draw_primitive(context, primitive, config);
        }
        let _ = context.restore();
    }
    let _ = context.restore();
}

fn draw_primitive(context: &Context, primitive: &NativePrimitive, config: &OverlayConfig) {
    match primitive {
        NativePrimitive::Segments { segments } => {
            context.new_path();
            for &[x1, y1, x2, y2] in segments {
                context.move_to(x1, y1);
                context.line_to(x2, y2);
            }
            stroke_path(context, config, config.thickness);
        }
        NativePrimitive::Circle { cx, cy, radius } => {
            context.new_path();
            context.arc(*cx, *cy, *radius, 0.0, std::f64::consts::TAU);
            stroke_path(context, config, config.thickness);
        }
        NativePrimitive::Dot { cx, cy, radius } => draw_dot(context, config, *cx, *cy, *radius),
        NativePrimitive::Polyline { points, closed } => {
            if let Some(first) = points.first() {
                context.new_path();
                context.move_to(first[0], first[1]);
                for point in points.iter().skip(1) {
                    context.line_to(point[0], point[1]);
                }
                if *closed {
                    context.close_path();
                }
                stroke_path(context, config, config.thickness);
            }
        }
        NativePrimitive::Arc {
            cx,
            cy,
            radius,
            start_deg,
            end_deg,
        } => {
            context.new_path();
            context.arc(
                *cx,
                *cy,
                *radius,
                start_deg.to_radians(),
                end_deg.to_radians(),
            );
            stroke_path(context, config, config.thickness);
        }
    }
}

fn stroke_path(context: &Context, config: &OverlayConfig, width: f64) {
    if config.shadow_enabled && config.shadow_blur > 0.0 {
        let shadow = parse_hex(&config.shadow_color).unwrap_or([0.41, 0.96, 0.82]);
        context.set_source_rgba(shadow[0], shadow[1], shadow[2], config.opacity * 0.22);
        context.set_line_width(width + config.shadow_blur * 2.0 + 2.0);
        let _ = context.stroke_preserve();
    }
    if config.outline_enabled && config.outline_thickness > 0.0 {
        let outline = parse_hex(&config.outline_color).unwrap_or([0.02, 0.04, 0.06]);
        context.set_source_rgba(outline[0], outline[1], outline[2], config.opacity);
        context.set_line_width(width + config.outline_thickness * 2.0);
        let _ = context.stroke_preserve();
    }
    let color = parse_hex(&config.color).unwrap_or([0.41, 0.96, 0.82]);
    context.set_source_rgba(color[0], color[1], color[2], config.opacity);
    context.set_line_width(width);
    let _ = context.stroke();
}

fn draw_dot(context: &Context, config: &OverlayConfig, cx: f64, cy: f64, radius: f64) {
    if config.shadow_enabled && config.shadow_blur > 0.0 {
        let shadow = parse_hex(&config.shadow_color).unwrap_or([0.41, 0.96, 0.82]);
        context.new_path();
        context.arc(cx, cy, radius + config.shadow_blur, 0.0, std::f64::consts::TAU);
        context.set_source_rgba(shadow[0], shadow[1], shadow[2], config.opacity * 0.22);
        let _ = context.fill();
    }
    if config.outline_enabled && config.outline_thickness > 0.0 {
        let outline = parse_hex(&config.outline_color).unwrap_or([0.02, 0.04, 0.06]);
        context.new_path();
        context.arc(
            cx,
            cy,
            radius + config.outline_thickness / 2.0,
            0.0,
            std::f64::consts::TAU,
        );
        context.set_source_rgba(outline[0], outline[1], outline[2], config.opacity);
        let _ = context.fill();
    }
    let color = parse_hex(&config.color).unwrap_or([0.41, 0.96, 0.82]);
    context.new_path();
    context.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
    context.set_source_rgba(color[0], color[1], color[2], config.opacity);
    let _ = context.fill();
}

fn parse_hex(value: &str) -> Option<[f64; 3]> {
    if value.len() != 7 || !value.starts_with('#') {
        return None;
    }
    Some([
        u8::from_str_radix(&value[1..3], 16).ok()? as f64 / 255.0,
        u8::from_str_radix(&value[3..5], 16).ok()? as f64 / 255.0,
        u8::from_str_radix(&value[5..7], 16).ok()? as f64 / 255.0,
    ])
}
