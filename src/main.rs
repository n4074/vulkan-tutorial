use anyhow::{Context, Result};

//use std::iter::Iter;

use libc::c_char;
use std::ffi::{CStr, CString};

use winit::{
    dpi::{LogicalSize, Pixel},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

use ash::vk::{self, ExtensionProperties};
//use ash::vk::{ApplicationInfo, StructureType};

struct VulkanApp {
    name: String,
    config: Config,
    window: Option<Window>,
    instance: Option<ash::Instance>,
    entry: Option<ash::Entry>,
}

#[derive(Default)]
struct Config {
    window_size: LogicalSize<u32>,
    resizable: bool,
}

impl VulkanApp {
    pub fn new(name: &str, width: u32, height: u32) -> Result<Self> {
        //let (event_loop, window) = Self::init_window()?;
        //let (event, instance) = Self::create_instance(&window)?;
        let app = VulkanApp {
            name: name.to_string(),
            config: Config {
                window_size: (width, height).into(),
                ..Default::default()
            },
            window: None,
            instance: None,
            entry: None,
        };
        Ok(app)
    }

    pub fn run(mut self) -> Result<()> {
        let event_loop = self.init_window()?;
        self.init_vulkan()?;
        self.main_loop(event_loop)?;
        Ok(())
    }

    pub fn main_loop(&self, event_loop: EventLoop<()>) -> Result<()> {
        let id = self.window.as_ref().context("window uninitialised")?.id();
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    window_id,
                } if window_id == id => *control_flow = ControlFlow::Exit,
                _ => (),
            }
        });
    }

    fn init_vulkan(&mut self) -> Result<()> {
        self.create_instance()?;
        Ok(())
    }

    fn create_instance(&mut self) -> Result<()> {
        let app_name = CString::new(self.name.as_bytes())?;
        let engine_name = CString::new("No engine")?;

        let app_info = vk::ApplicationInfo::builder()
            .application_name(&app_name)
            .application_version(vk::make_api_version(1, 0, 0, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(1, 0, 0, 0))
            .api_version(vk::make_api_version(1, 0, 0, 0))
            .build();

        let entry = unsafe { ash::Entry::new()? };
        let supported_extensions = entry.enumerate_instance_extension_properties()?;

        let required_extensions: Vec<*const c_char> =
            ash_window::enumerate_required_extensions(self.window.as_ref().unwrap())?
                .iter()
                .map(|s| s.as_ptr())
                .collect();

        unsafe {
            Self::check_extension(&required_extensions, &supported_extensions)?;
        }

        let instance_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(required_extensions.as_slice())
            .build();

        let instance = unsafe { entry.create_instance(&instance_info, None)? };

        self.entry = Some(entry);
        self.instance = Some(instance);

        Ok(())
    }

    unsafe fn check_extension(
        required: &Vec<*const c_char>,
        supported: &Vec<ExtensionProperties>,
    ) -> Result<()> {
        for &req in required {
            let in_supported = supported.iter().any(|ext| unsafe {
                // really is unsafe
                CStr::from_ptr(ext.extension_name.as_ptr()) == CStr::from_ptr(req)
            });

            if !in_supported {
                anyhow::bail!(
                    "Required extension is unsupported: {}",
                    CStr::from_ptr(req).to_str()?
                );
            }
        }

        Ok(())
    }

    fn init_window(&mut self) -> Result<EventLoop<()>> {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title(self.name.clone())
            .with_inner_size(LogicalSize::<u32>::from(self.config.window_size))
            .with_resizable(self.config.resizable)
            .build(&event_loop)
            .context("Failed to create event loop")?;

        self.window = Some(window);
        Ok(event_loop)
    }
}

impl Drop for VulkanApp {
    fn drop(&mut self) {
        unsafe {
            self.instance.take().unwrap().destroy_instance(None);
        }
    }
}

fn main() -> Result<()> {
    VulkanApp::new("Vulkan", 800, 600)?.run()
}
