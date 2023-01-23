use skia_safe::{
    gpu::gl::FramebufferInfo, gpu::BackendRenderTarget, gpu::DirectContext, Color, Paint,
    PaintStyle, Surface,
    Rect,
    Point,
    Font,
    Typeface,
    FontStyle,
    TextBlob,
    Canvas,
    Data,
    FontMgr,
    font_style::{
        Weight,
        Width,
        Slant,
    }, typeface::SerializeBehavior, Color4f, ColorSpace, Picture, PictureRecorder, RCHandle, IRect, IPoint, ImageInfo, ISize
};
use std::{boxed::Box, ops::Range};
use futures::executor::block_on;

extern "C" {
    pub fn emscripten_GetProcAddress(
        name: *const ::std::os::raw::c_char,
    ) -> *const ::std::os::raw::c_void;
    pub fn emscripten_set_keydown_callback(
        target: *const ::std::os::raw::c_char,
        userData: *mut ::std::os::raw::c_void,
        useCapture: i32,
        callback: unsafe extern "C" fn(
            eventTypeId: *const ::std::os::raw::c_char,
            event: *mut ::std::os::raw::c_void,
            userData: *mut ::std::os::raw::c_void,
        ) -> i32,
    ) -> i32;
}

struct GpuState {
    context: DirectContext,
    framebuffer_info: FramebufferInfo,
}

/// This struct holds the state of the Rust application between JS calls.
///
/// It is created by [init] and passed to the other exported functions. Note that rust-skia data
/// structures are not thread safe, so a state must not be shared between different Web Workers.
pub struct State {
    gpu_state: GpuState,
    surface: Surface,
    recorder: PictureRecorder,
    picture: Option<Picture>,
}

impl State {
    fn new(gpu_state: GpuState, surface: Surface) -> Self {
        State { gpu_state, surface, recorder: PictureRecorder::new(), picture: None }
    }

    fn set_surface(&mut self, surface: Surface) {
        self.surface = surface;
    }

    fn set_picture(&mut self, picture: Picture) {
        self.picture = Some(picture);
    }
}

/// Load GL functions pointers from JavaScript so we can call OpenGL functions from Rust.
///
/// This only needs to be done once.
fn init_gl() {
    unsafe {
        gl::load_with(|addr| {
            let addr = std::ffi::CString::new(addr).unwrap();
            emscripten_GetProcAddress(addr.into_raw() as *const _) as *const _
        });
    }
}

/// Create the GPU state from the JavaScript WebGL context.
///
/// This needs to be done once per WebGL context.
fn create_gpu_state() -> GpuState {
    let context = skia_safe::gpu::DirectContext::new_gl(None, None).unwrap();
    let framebuffer_info = {
        let mut fboid: gl::types::GLint = 0;
        unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fboid) };

        FramebufferInfo {
            fboid: fboid.try_into().unwrap(),
            format: skia_safe::gpu::gl::Format::RGBA8.into(),
        }
    };

    GpuState {
        context,
        framebuffer_info,
    }
}

/// Create the Skia surface that will be used for rendering.
fn create_surface(gpu_state: &mut GpuState, width: i32, height: i32) -> Surface {
    let backend_render_target =
        BackendRenderTarget::new_gl((width, height), 1, 8, gpu_state.framebuffer_info);

    Surface::from_backend_render_target(
        &mut gpu_state.context,
        &backend_render_target,
        skia_safe::gpu::SurfaceOrigin::BottomLeft,
        skia_safe::ColorType::RGBA8888,
        None,
        None,
    )
    .unwrap()
}



/// Initialize the renderer.
///
/// This is called from JS after the WebGL context has been created.
#[no_mangle]
pub extern "C" fn init(width: i32, height: i32) -> Box<State> {
    let mut gpu_state = create_gpu_state();
    let surface = create_surface(&mut gpu_state, width, height);
    let state = State::new(gpu_state, surface);
    Box::new(state)
}

/// Resize the Skia surface
///
/// This is called from JS when the window is resized.
#[no_mangle]
pub extern "C" fn resize_surface(state: *mut State, width: i32, height: i32) {
    let state = unsafe { state.as_mut() }.expect("got an invalid state pointer");
    let surface = create_surface(&mut state.gpu_state, width, height);
    state.set_surface(surface);
}

fn render_circle(surface: &mut Surface, x: f32, y: f32, radius: f32) {
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Fill);
    paint.set_color(Color::BLACK);
    paint.set_anti_alias(true);
    surface.canvas().draw_circle((x, y), radius, &paint);
}

/// Draw a black circle at the specified coordinates.
#[no_mangle]
pub extern "C" fn draw_circle(state: *mut State, x: i32, y: i32) {
    let state = unsafe { state.as_mut() }.expect("got an invalid state pointer");
    //state.surface.canvas().clear(Color::WHITE);
    render_circle(&mut state.surface, x as f32, y as f32, 50.);
    state.surface.flush();
}

fn render_box(surface: &mut Surface, x: f32, y: f32, width: f32, height: f32) {
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Fill);
    paint.set_color(Color::BLACK);
    paint.set_anti_alias(true);
    surface.canvas().draw_rect(Rect {left: x, top: y, right: x + width, bottom: y+height},  &paint);
}

async fn hello_world() {
    println!("aye");
    
    let body = reqwest_wasm::get("https://www.rust-lang.org")
    .await.unwrap()
    .text()
    .await;
    println!("body = {:?}", body);
}

#[no_mangle]
pub extern "C" fn draw_box(state: *mut State, x: i32, y: i32) {
    let state = unsafe { state.as_mut() }.expect("got an invalid state pointer");
    //state.surface.canvas().clear(Color::WHITE);
    render_box(&mut state.surface, x as f32, y as f32, 50., 50.);
    state.surface.flush();
    
    block_on(hello_world());
}

fn render_character_f(canvas: &mut Canvas, paint: &Paint, x: f32, y: f32) {
    canvas.draw_line(Point {x, y }, Point {x: x + 8., y}, &paint);
    canvas.draw_line(Point {x, y }, Point {x, y: y + 12.}, &paint);
    canvas.draw_line(Point {x, y: y + 5. }, Point {x: x + 5., y: y + 5.}, &paint);
}

fn render_cell(canvas: &mut Canvas, bg_paint: &Paint, border_paint: &Paint, f_paint: &Paint, x: f32, y: f32, width: f32, height: f32, text: &str) {
    canvas.draw_rect(Rect {left: x, top: y, right: x + width, bottom: y+height},  &bg_paint);

    canvas.draw_line(Point {x: x+ width, y },Point {x: x+ width, y: y + height },  &border_paint);
    canvas.draw_line(Point {x, y: y + height },Point {x: x+ width, y: y + height },  &border_paint);

    for n in 0..7 {
        render_character_f(canvas, &f_paint, x + n as f32 * 12. + 10., y + 10.);
    }

    // let data = Data::new_bytes(text.as_bytes().to_vec());
    
    // for x in &font_mgr.family_names() {
    //     println!("family name {:?}", x);
    // }
    
    // println!("{:#?}",);



    // let mut text_paint = Paint::default();
    // let mut font = Font::default();
    // let typeface = Typeface::from_name("Arial", FontStyle::new(Weight::from(400), Width::from(16), Slant::Upright)).unwrap();
    // println!("typeface {:?} {:?}", typeface.family_name(), typeface.is_italic());

    // // panic!("hey");
    // font.set_typeface(typeface);
    // text_paint.set_color(Color::BLACK);
    
    
    // surface.canvas().draw_str(text, (10., 10. ), &font, &text_paint);
    // surface.canvas().draw_str(text, (500., 500. ), &font, &text_paint);
    // surface.canvas().draw_str(text, (-50., 50. ), &font, &text_paint);
    // // surface.canvas().draw_text_blob(blob, origin, paint)
        // //
    // let blob = TextBlob::from_str(text, &font).unwrap();
    // surface.canvas().draw_text_blob(blob, (x + 5., 80. ), &text_paint);

    
}

const WIDTH: f32 = 160.0;
const HEIGHT: f32 = 32.0;

#[no_mangle]
pub extern "C" fn on_animation_frame(state: *mut State, scroll: i32) {
    let state = unsafe { state.as_mut() }.expect("got an invalid state pointer");
    state.surface.canvas().clear(Color::WHITE);

    let mut bg_paint = Paint::default();
    bg_paint.set_style(PaintStyle::Fill);
    bg_paint.set_color(Color::GRAY);
    bg_paint.set_anti_alias(true);
    
    let mut border_paint = Paint::default();
    border_paint.set_style(PaintStyle::Fill);
    border_paint.set_color(Color::BLACK);
    border_paint.set_anti_alias(true);

    let mut f_paint = Paint::default();
    f_paint.set_style(PaintStyle::Fill);
    f_paint.set_color4f(Color4f { r: 255., g: 255., b: 255., a: 1.}, None);
    f_paint.set_anti_alias(true);

    // let mut recorder = PictureRecorder::new();
    let mut canvas_recorder = state.recorder.begin_recording(Rect { left:0., top: 0., right: 1000., bottom: 1000.}, None);

    for col in 0..10 {
        for row in 0..100 {
            let y = row as f32 * HEIGHT + scroll as f32;
            let x = col as f32 * WIDTH;
            print!("{} {} ", x, y);
            render_cell(&mut canvas_recorder, &bg_paint, &border_paint, &f_paint, x, y, WIDTH, HEIGHT, "hola");
        }
    }
    let picture = state.recorder.finish_recording_as_picture(None).unwrap();
    
    picture.playback(state.surface.canvas());
    state.surface.flush();

    state.set_picture(picture);
}

#[no_mangle]
pub extern "C" fn on_translate(state: *mut State, scroll: i32) {
    let state = unsafe { state.as_mut() }.expect("got an invalid state pointer");
    
    
    state.surface.canvas().translate(Point { x: 0., y: scroll as f32});
    match state.picture {
        Some(ref picture) => {
            picture.playback(state.surface.canvas());
        },
        None => panic!("no picture")
    }

    state.surface.flush();
    
    // blitting part of canvas to another place
    // let canvas = state.surface.canvas();

    // let dst_info = ImageInfo::new_n32_premul(ISize {width: 20, height: 20}, None);
    // let mut dst_pixels = vec![0; dst_info.compute_byte_size(20 * 4)];
    // let src_point = IPoint::new(10, 10);
    // let suc = canvas.read_pixels(&dst_info, &mut dst_pixels, 20 * 4, src_point);
    // if suc == false {
    //     panic!("read pixels failed");
    // }

    
    // let offset = IPoint::new(0, 40);
    // let suc = canvas.write_pixels(&dst_info, &dst_pixels, 20 * 4, offset);
    // if suc == false {
    //     panic!("write pixels failed");
    // }

    // println!("successed?");
    // state.surface.flush();
}


/// The main function is called by emscripten when the WASM object is created.
fn main() {
    init_gl();
}
