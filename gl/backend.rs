use std::num::NonZero;

use glutin::{
    config::GlConfig as _,
    context::PossiblyCurrentContext,
    context::{self, ContextAttributesBuilder},
    display::{Display, DisplayApiPreference, GlDisplay as _},
    prelude::NotCurrentGlContext as _,
    surface::Surface,
    surface::{GlSurface as _, SurfaceAttributesBuilder, SwapInterval, WindowSurface},
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle as _};
use winit::dpi::PhysicalSize;

pub struct Backend {
    context: PossiblyCurrentContext,
    display: Display,
    surface: Surface<WindowSurface>,

    imgui: ImGui
}

pub struct ImGui {
    pub ctx: imgui::Context,
    pub platform: imgui_winit_support::WinitPlatform,
    pub renderer: imgui_opengl_renderer::Renderer,
}

impl cmn::common::Backend for Backend {
    type FrameCtx = ();

    fn initalize(platform: &cmn::common::Platform) -> anyhow::Result<Self> {
        let window = &platform.window;
        let hwnd = window.window_handle().map(|handle| handle.as_raw())?;
        let hdsp = window.display_handle().map(|handle| handle.as_raw())?;

        let template = glutin::config::ConfigTemplateBuilder::new()
            .compatible_with_native_window(hwnd)
            .prefer_hardware_accelerated(Some(true))
            .build();

        let display = unsafe { Display::new(hdsp, DisplayApiPreference::Wgl(Some(hwnd)))? };

        let gl_config = unsafe {
            display
                .find_configs(template)?
                .reduce(|accum, config| {
                    if config.supports_transparency().unwrap_or_default()
                        || config.num_samples() > accum.num_samples()
                    {
                        config
                    } else {
                        accum
                    }
                })
                .ok_or(anyhow::anyhow!("no suitable configurations found",))?
        };

        let surface = unsafe {
            let PhysicalSize { width, height } = dbg!(window.inner_size());
            display.create_window_surface(
                &gl_config,
                &SurfaceAttributesBuilder::<WindowSurface>::default().build(
                    hwnd,
                    NonZero::new(width).unwrap(),
                    NonZero::new(height).unwrap(),
                ),
            )?
        };

        let context = {
            let context_attributes = ContextAttributesBuilder::new()
                .with_profile(context::GlProfile::Core)
                .with_debug(cfg!(debug_assertions))
                .with_context_api(context::ContextApi::OpenGl(Some(context::Version::new(
                    4, 6,
                ))))
                .with_robustness(context::Robustness::RobustLoseContextOnReset)
                .build(Some(hwnd));

            unsafe {
                display
                    .create_context(&gl_config, &context_attributes)
                    .expect("failed to create context")
            }
            .make_current(&surface)
            .expect("can make surface current")
        };

        surface.set_swap_interval(&context, SwapInterval::DontWait)?;

        let loader = |symbol| {
            let symbol = std::ffi::CString::new(symbol).expect("no zero byte in symbol names");
            display.get_proc_address(symbol.as_c_str()).cast()
        };

        gb::load_with(&loader);

        let imgui = {
            let mut ctx = imgui::Context::create();

            let mut platform = imgui_winit_support::WinitPlatform::new(&mut ctx);
            platform.attach_window(
                ctx.io_mut(),
                &window,
                imgui_winit_support::HiDpiMode::Default,
            );

            let renderer = imgui_opengl_renderer::Renderer::new(&mut ctx, loader);

            ImGui {
                platform,
                renderer,
                ctx,
            }
        };

        Ok(Self {
            context,
            display,
            surface,
            imgui,
        })
    }

    fn pre(&mut self) -> Self::FrameCtx {}

    fn post(&mut self) {}
}
