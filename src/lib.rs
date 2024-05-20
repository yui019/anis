use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowAttributes,
};

pub mod context;

use crate::context::Context;

impl<'a> ApplicationHandler for Context<'a> {
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            // close on escape or when it's requested
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        ..
                    },
                ..
            } => event_loop.exit(),

            // handle window resizing
            WindowEvent::Resized(physical_size) => {
                self.resize(physical_size);
            }

            WindowEvent::RedrawRequested => {
                self.window.request_redraw();

                self.update();

                match self.render() {
                    Ok(_) => {}

                    // Reconfigure the surface if it's lost or outdated
                    Err(
                        wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated,
                    ) => self.resize(self.size),

                    // The system is out of memory, should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        log::error!("OutOfMemory");
                        event_loop.exit();
                    }

                    // This happens when the a frame takes too long to present
                    Err(wgpu::SurfaceError::Timeout) => {
                        log::warn!("Surface timeout")
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn run() {
    let event_loop = EventLoop::new().unwrap();

    // this is supposed to be done in ApplicationHandler::resumed because on
    // some platforms, like Android, you need to handle suspend and resume
    // events by recreating your entire graphics context. I only care about
    // desktop platforms so that doesn't matter to me.
    #[allow(deprecated)]
    let window = event_loop
        .create_window(WindowAttributes::default())
        .unwrap();

    let mut context = Context::new(&window);

    event_loop.run_app(&mut context).unwrap();
}
