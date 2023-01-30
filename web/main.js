/**
 * Make a canvas element fit to the display window.
 */
let totDelta = 0;
// let toBottom = true;
const startFakeScrolling = () => {
  const SCROLL_BY = 300;
  const cb = () => {
    // if (totDelta > metrics.missingHeight / 2) {
    //   toBottom = false;
    // } else if (totDelta < 100) {
    //   toBottom = true;
    // }

    const delta = SCROLL_BY; //toBottom ? SCROLL_BY : -SCROLL_BY;
    totDelta += delta;

    // document
    //   .querySelector("canvas")
    window.dispatchEvent(
      new CustomEvent("mousewheel", { detail: { deltaY: delta } })
    );

    window.requestAnimationFrame(cb);
  };
  window.requestAnimationFrame(cb);
};

function resizeCanvasToDisplaySize(canvas) {
  const width = canvas.clientWidth * window.devicePixelRatio;
  const height = canvas.clientHeight * window.devicePixelRatio;
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
    return true;
  }
  return false;
}

let scrollY = 0;
let scrollX = 0;

// This loads and initialize our WASM module
createRustSkiaModule().then((RustSkia) => {
  // Create the WebGL context
  let context;
  const canvas = document.querySelector("#glcanvas");
  context = canvas.getContext("webgl2", {
    antialias: true,
    depth: true,
    stencil: true,
    alpha: true,
  });

  const debugInfo = context.getExtension("WEBGL_debug_renderer_info");
  const vendor = context.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL);
  const renderer = context.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL);
  console.log(vendor, renderer);

  // Register the context with emscripten
  handle = RustSkia.GL.registerContext(context, { majorVersion: 2 });
  RustSkia.GL.makeContextCurrent(handle);

  // Fit the canvas to the viewport
  resizeCanvasToDisplaySize(canvas);

  // Initialize Skia
  const state = RustSkia._init(canvas.width, canvas.height);

  // Draw a circle that follows the mouse pointer
  // window.addEventListener("mousemove", (event) => {
  // const canvasPos = canvas.getBoundingClientRect();
  // RustSkia._draw_circle(
  //   state,
  //   event.clientX - canvasPos.x,
  //   event.clientY - canvasPos.y
  // );
  // });

  // window.addEventListener("click", (event) => {
  //   const canvasPos = canvas.getBoundingClientRect();
  //   RustSkia._draw_box(
  //     state,
  //     event.clientX - canvasPos.x,
  //     event.clientY - canvasPos.y
  //   );
  // });

  let isRunning = false;

  window.addEventListener(
    "mousewheel",
    (event) => {
      event.preventDefault();
      const deltaY =
        event.deltaY != null ? event.deltaY : event.detail.deltaY ?? 0;
      scrollY += deltaY;
      // scrollX += event.deltaX;
      if (isRunning) {
        return;
      }

      isRunning = true;
      window.requestAnimationFrame(() => {
        RustSkia._on_translate(state, scrollY);
        isRunning = false;
      });
    },
    { passive: false }
  );

  // Make canvas size stick to the window size
  window.addEventListener("resize", () => {
    if (resizeCanvasToDisplaySize(canvas)) {
      RustSkia._resize_surface(state, canvas.width, canvas.height);
    }
  });

  RustSkia._on_animation_frame(state);
  // const cb = () => {
  //   RustSkia._on_animation_frame(state, scrollY);
  //   window.requestAnimationFrame(cb);
  // };
  // window.requestAnimationFrame(cb);
  // startFakeScrolling();
});
