use anyhow::{Context, Result};

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

struct VulkanApp {
    window_title: String,
}

impl VulkanApp {
    pub fn new(title: &str) -> Self {
        VulkanApp {
            window_title: title.to_string(),
        }
    }
    pub fn run(self) {
        let event_loop = init_window(self.window_title);
        let window = init_window(event_loop);
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    window_id,
                } if window_id == window.id() => *control_flow = ControlFlow::Exit,
                _ => (),
            }
        });
    }

    fn initVulkan() {}

    fn init_window(self, event_loop: EventLoop<()>) -> Result<Window> {
        WindowBuilder::new()
            .with_title(self.window_title)
            .build(event_loop)
            .context("Failed to create event loop");
    }
}

fn main() {
    VulkanApp::new("inital").run()
}
