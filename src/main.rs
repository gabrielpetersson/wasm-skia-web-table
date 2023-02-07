use skia_safe::{
    gpu::gl::FramebufferInfo, 
    gpu::BackendRenderTarget, 
    gpu::DirectContext, 
    Color, 
    Paint,
    PaintStyle, 
    Surface,
    Rect,
    Point,
    Color4f,
    ColorSpace,
    Picture,
    PictureRecorder,
    ISize,
    Image,
    Typeface,
    Data,
    Font,
    TextBlob,
    DeferredDisplayListRecorder,
    Canvas
};
use std::{boxed::Box, collections::HashMap};
use once_cell::sync::Lazy;


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
    pub fn emscripten_get_now() -> ::std::os::raw::c_double;
}

const CELL_WIDTH: f32 = 320.0;
const CELL_HEIGHT: f32 = 64.0;
const ROWS_PER_TILE: i32 = 10;
const TILE_HEIGHT: f32 = CELL_HEIGHT * ROWS_PER_TILE as f32;

fn now() -> f64 {
    unsafe { emscripten_get_now() }
}

static FONT: Lazy<Font> = Lazy::new(|| {
    let inter_bytes = include_bytes!("./Inter.ttf");
    let inter_data = unsafe { Data::new_bytes(inter_bytes) };
    let typeface = Typeface::from_data(inter_data, None).unwrap();
    let font = Font::new(typeface.clone(), Some(24.0));
    font
});



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
    tile_cache: HashMap<i32, Image>
}

impl State {
    fn new(gpu_state: GpuState, surface: Surface) -> Self {
        State { gpu_state, surface, recorder: PictureRecorder::new(), tile_cache: HashMap::new() }
    }

    fn set_surface(&mut self, surface: Surface) {
        self.surface = surface;
    }

    fn set_tile(&mut self, index: i32, image: Image) { // -> &Image
        self.tile_cache.insert(index, image);
        // self.tile_cache.get(&index).unwrap()
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

fn create_cell_picture() -> Picture {
    let mut picture_recorder = PictureRecorder::new();
    let canvas = picture_recorder.begin_recording(Rect { left:0., top: 0., right: CELL_WIDTH, bottom: CELL_HEIGHT}, None);
    
    let mut bg_paint = Paint::default();
    bg_paint.set_style(PaintStyle::Fill);
    bg_paint.set_color(Color::GRAY);
    bg_paint.set_anti_alias(true);
    
    let mut border_paint = Paint::default();
    border_paint.set_style(PaintStyle::Fill);
    border_paint.set_color(Color::BLACK);
    border_paint.set_anti_alias(true);

    canvas.draw_rect(Rect {left: 0., top: 0., right: CELL_WIDTH, bottom: CELL_HEIGHT},  &bg_paint);
    canvas.draw_line(Point {x: CELL_WIDTH, y: 0. },Point {x: CELL_WIDTH, y: CELL_HEIGHT },  &border_paint);
    canvas.draw_line(Point {x: 0., y: CELL_HEIGHT },Point {x:CELL_WIDTH, y: CELL_HEIGHT },  &border_paint);

    let picture = picture_recorder.finish_recording_as_picture(None).unwrap();
    picture
}

#[no_mangle]
pub extern "C" fn init(width: i32, height: i32) -> Box<State> {
    let mut gpu_state = create_gpu_state();
    let surface = create_surface(&mut gpu_state, width, height);
    // let cell_picture = create_cell_picture();

    let state = State::new(gpu_state, surface);
    Box::new(state)
}

#[no_mangle]
pub extern "C" fn resize_surface(state: *mut State, width: i32, height: i32) {
    let state = unsafe { state.as_mut() }.expect("got an invalid state pointer");
    let surface = create_surface(&mut state.gpu_state, width, height);
    state.set_surface(surface);
}


fn paint_tile(tile_offset: i32, canvas: &mut Canvas, tile_width: f32) {
    // let start = now();
    
    let mut text_paint = Paint::default();
    text_paint.set_color4f(Color4f { r: 1., g: 1., b: 1., a: 1.}, None);
    // text_paint.set_anti_alias(true);
    
    // let mut bg_paint = Paint::default();
    // bg_paint.set_style(PaintStyle::Fill);
    // bg_paint.set_color4f(Color4f {r: 0.094, g: 0.12, b: 0.15, a: 1.}, None);
    // bg_paint.set_anti_alias(true);
    
    let mut border_paint = Paint::default();
    border_paint.set_style(PaintStyle::Fill);
    border_paint.set_color4f(Color4f {r: 0.19, g: 0.24, b: 0.29, a: 1.} , None);
    border_paint.set_anti_alias(true);
    
    // println!("tile setup - {}", now() - start);
    // let start = now();

    let start_row = tile_offset * ROWS_PER_TILE; 
    let end_row = ROWS_PER_TILE + start_row;
    
    // background
    // canvas.draw_rect(Rect {left: 0., top: 0., right: tile_width, bottom: TILE_HEIGHT},  &bg_paint);
    // println!("tile bg - {}", now() - start);
    // let start = now();

    // horizontal lines
    for row in start_row..end_row {
        let y = CELL_HEIGHT * (row - start_row + 1) as f32;
        canvas.draw_line(Point {x: 0., y },Point {x: tile_width, y },  &border_paint);
    }

    // println!("tile horizontal lines - {}", now() - start);
    // let start = now();

    // vertical lines
    for col in 0..7 {
        let vertical_line_x = CELL_WIDTH * (col + 1) as f32;
        canvas.draw_line(Point {x: vertical_line_x, y: 0. },Point {x: vertical_line_x, y: TILE_HEIGHT },  &border_paint);
    }

    // println!("tile vertical lines - {}", now() - start);
    // let start = now();

    // text    
    for row in start_row..end_row {
        let y = CELL_HEIGHT * (row - start_row) as f32;
        let row_string = row.to_string();
        for col in 0..7 {
            let x = CELL_WIDTH * col as f32;
            // let col_string = col.to_string();
            // let start = now();
            // slow
            // let s: String = rand::thread_rng()
            //     .sample_iter(&Alphanumeric)
            //     .take(7)
            //     .map(char::from)
            //     .collect();
            // let s = format!("{}-{}", row_string, col.to_string());

            // let text_blob = TextBlob::from_str(&row_string, &FONT).unwrap();
            // canvas.draw_text_blob(text_blob, (x + 20., y + 40.),  &text_paint);
            canvas.draw_str(&row_string, (x + 20., y + 40.),  &FONT, &text_paint);
            // println!("hh draw {}", now() - start);
        }        
    }
    
    // println!("tile text - {}", now() - start);
    // let start = now();
}

const USE_GPU: bool = false;
fn raster_tile(state: &mut State, tile_offset: i32) -> Image {  //, paint: &Paint, font: &Font
    // let vv = state.surface.recording_context().unwrap();
    // &surf.from_backend_render_target(ColorType::RGBA8888, &BackendFormat::new_gl(state.gpu_state.framebuffer_info.format, state.gpu_state.framebuffer_info.format))
    // let surf = SurfaceCharacterization::default();

    if USE_GPU == true {
        let mut surf = create_surface(&mut state.gpu_state, state.surface.width(), TILE_HEIGHT as i32);
        let characterization = surf.characterize().unwrap();
        let mut display_list_recorder = DeferredDisplayListRecorder::new(&characterization);
        let canvas = display_list_recorder.canvas();

        paint_tile(tile_offset, canvas, state.surface.width() as f32);

        let display_list = display_list_recorder.detach().unwrap();
        surf.draw_display_list(&display_list);
        // println!("tile draw display list - {}", now() - start);
        // let start = now();
        let image = surf.image_snapshot();

        // println!("IS GPU?? {}", image.is_texture_backed());
        image
        // println!("tile image - {}", now() - start);
    } else {
        let mut recorder = PictureRecorder::new();
        let canvas = recorder.begin_recording(Rect { left:0., top: 0., right: state.surface.width() as f32, bottom: TILE_HEIGHT}, None);    
        
        paint_tile(tile_offset, canvas, state.surface.width() as f32);
        let picture = recorder.finish_recording_as_picture(None).unwrap();
        let image = Image::from_picture(&picture, ISize { width: state.surface.width(), height: TILE_HEIGHT as i32}, None, None, skia_safe::image::BitDepth::U8, Some(ColorSpace::new_srgb())).unwrap();
        
        // println!("IS GPU?? {}", image.is_texture_backed());
        image
    }
}

#[no_mangle]
pub extern "C" fn on_animation_frame(state: *mut State) {
    let state = unsafe { state.as_mut() }.expect("got an invalid state pointer");
    state.surface.canvas().clear(Color4f {r: 41., g: 55., b: 66., a: 1.});

    let mut tile_border = Paint::default();
    tile_border.set_style(PaintStyle::Stroke);
    tile_border.set_color4f(Color4f {r: 1., g: 0., b: 0., a: 1.}, None);
    tile_border.set_stroke_width(2.);

    let surface_height = state.surface.height();
    let surface_width = state.surface.width();

    let tiles_on_screen = (surface_height as f32 / TILE_HEIGHT).ceil() as i32;
    for tile_offset in 0..tiles_on_screen {
        // println!("tile offset {}", tile_offset);
        let image = raster_tile(state, tile_offset);

        let y = TILE_HEIGHT * tile_offset as f32;
        state.surface.canvas().draw_image(&image, Point { x: 0., y}, None);

        
        state.surface.canvas().draw_rect(Rect {left: 0., top: y, right: surface_width as f32, bottom: y + TILE_HEIGHT  }, &tile_border);
        state.set_tile(tile_offset, image);

        // println!("tile draw {}", tile_offset);
    } 

    // let font = Font::new(TYPEFACE.clone(), Some(56.0));
    // let mut text_paint = Paint::default();
    // text_paint.set_color(Color::BLACK);
    // text_paint.set_anti_alias(true);
    // let text_blob = TextBlob::from_str("HEYYYYYYY", &font).unwrap();
    // state.surface.canvas().draw_text_blob(&text_blob, (10., 200.), &text_paint);

    state.surface.flush();
}

#[no_mangle]
pub extern "C" fn on_translate(state: *mut State, scroll: i32) {
    // let start = now();
    let state = unsafe { state.as_mut() }.expect("got an invalid state pointer");
    state.surface.canvas().clear(Color4f {r: 0.094, g: 0.12, b: 0.15, a: 1.});
    
    let mut tile_border = Paint::default();
    tile_border.set_style(PaintStyle::Stroke);
    tile_border.set_color4f(Color4f {r: 1., g: 0., b: 0., a: 1.}, None);
    tile_border.set_stroke_width(2.);
    
    let surface_height = state.surface.height();
    let surface_width = state.surface.width();
    
    let scroll_offset = scroll % TILE_HEIGHT as i32;
    let start_tile = (scroll as f32 / TILE_HEIGHT).floor() as i32;
    let tiles_on_screen = ((surface_height + scroll_offset) as f32 / TILE_HEIGHT).ceil() as i32;
    let end_tile = start_tile + tiles_on_screen;

    // println!("setup {}", now() - start);
    // let start = now();

    for tile_offset in start_tile..end_tile {
        // let image = raster_tile(state, tile_offset);
        let maybe_image = state.tile_cache.get(&tile_offset);
        
        let image = match maybe_image {
            Some(image) => image,
            None => {
                let start = now();
                let image = raster_tile(state, tile_offset);
                state.set_tile(tile_offset, image);
                println!("draw tile with offsest {} took {}ms.", tile_offset, now() - start);
                state.tile_cache.get(&tile_offset).unwrap()
            }
        };
        
        let y = TILE_HEIGHT * (tile_offset - start_tile) as f32 - scroll_offset as f32;
        state.surface.canvas().draw_image(&image, Point { x: 0., y}, None);
        state.surface.canvas().draw_rect(Rect {left: 0., top: y, right: surface_width as f32, bottom: y + TILE_HEIGHT  }, &tile_border);
    } 
    // println!("loop {}", now() - start);
    // let start = now();
    // let start = now();
    state.surface.flush();
    // println!("flush {}", now() - start);
}

/// The main function is called by emscripten when the WASM object is created.
fn main() {
    init_gl();
}

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




    // let data = Data::new_bytes(text.as_bytes().to_vec());
    
    // for x in &font_mgr.family_names() {
        // println!("family name {:?}", x);
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
    // surface.canvas().draw_text_blob(blob, (x + 5., 80. ), &text_paint




    // let mut recorder = PictureRecorder::new();
    // let mut canvas_recorder = state.recorder.begin_recording(Rect { left:0., top: 0., right: 1000., bottom: 1000.}, None);

    
    // let image = Image::new_raster_n32_premul(100, 100);
    // let mut image_canvas = Canvas::image(&image);
    // for col in 0..10 {
    //     for row in 0..100 {

            // let canvas: Canvas = {
            //     if row % 20 == 0 && row != 0 {
            //         let image = Image::new_raster_n32_premul(100, 100);
            //         let mut image_canvas = Canvas::new_from_image(&image);
            //         return &mut image_canvas
            //     }
            //     return &mut image_canvas;
            // };
    //         let y = row as f32 * CELL_HEIGHT + scroll as f32;
    //         let x = col as f32 * CELL_WIDTH;
    //         render_cell(&mut canvas_recorder, &f_paint, x, y);
    //     }
    // }
    // let picture = state.recorder.finish_recording_as_picture(None).unwrap();
    
    
    // picture.playback(state.surface.canvas());
    
    
    // let image = Image::from_picture(&picture, ISize { width: 500, height: 500}, None, Some(&Paint::default()), skia_safe::image::BitDepth::U8, Some(ColorSpace::new_srgb())).unwrap();
    // state.surface.canvas().draw_image(&image, Point { x: 200., y: 200.}, None);
    // println!("{} - {}", state.surface.height(), state.surface.width());


    // state.set_picture(picture);