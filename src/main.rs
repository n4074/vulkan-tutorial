use anyhow::{Context, Result};

use lazy_static::lazy_static;
use libc::c_char;
use std::{
    borrow::Cow,
    ffi::{CStr, CString},
};

use log::debug;

use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

use ash::vk::{self, DebugUtilsMessengerCreateInfoEXTBuilder};
use ash::{extensions::ext::DebugUtils, vk::DebugUtilsMessengerCreateInfoEXT};
//use ash::vk::{ApplicationInfo, StructureType};

#[derive(Default)]
struct VulkanApp {
    name: String,
    config: Config,
    window: Option<Window>,
    instance: Option<ash::Instance>,
    entry: Option<ash::Entry>,
    enable_validation_layer: bool,
    debug_callback: Option<vk::DebugUtilsMessengerEXT>,
    debug_utils_loader: Option<DebugUtils>,
}

lazy_static! {
    static ref KHRONOS_VALIDATION: &'static CStr =
        CStr::from_bytes_with_nul("VK_LAYER_KHRONOS_validation\0".as_bytes()).unwrap();
}

#[derive(Default)]
struct Config {
    window_size: LogicalSize<u32>,
    resizable: bool,
}

// copied from ash/examples/src/lib.rs
unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number: i32 = callback_data.message_id_number as i32;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    println!(
        "{:?}:\n{:?} [{} ({})] : {}\n",
        message_severity,
        message_type,
        message_id_name,
        &message_id_number.to_string(),
        message,
    );

    vk::FALSE
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
            enable_validation_layer: true, // if cfg!(debug_assertions) { true } else { false },
            debug_callback: None,
            debug_utils_loader: None,
        };
        Ok(app)
    }

    pub fn run(mut self) -> Result<()> {
        let event_loop = self.init_window()?;
        self.init_vulkan()?;
        //self.main_loop(event_loop)?;
        debug!("cleanup");
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
            .api_version(vk::make_api_version(1, 0, 0, 0));

        let entry = unsafe { ash::Entry::new()? };

        //let extensions = ash_window::enumerate_required_extensions(self.window.as_ref().unwrap())?;
        let extensions = self.get_required_extension()?;

        let extension_ptrs: Vec<*const c_char> = extensions.iter().map(|s| s.as_ptr()).collect();

        self.check_extension_support(&entry, &extension_ptrs)?;

        let validation_layers = self.get_required_validation_layers()?;

        let validation_layer_ptrs = validation_layers.iter().map(|l| l.as_ptr()).collect();

        self.check_validation_layer_support(&entry, &validation_layer_ptrs)?;

        let mut instance_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_ptrs)
            .enabled_layer_names(&validation_layer_ptrs);

        let mut debug_create_info;

        if self.enable_validation_layer {
            debug_create_info = self.populate_debug_messenger_create_info()?;
            instance_info = instance_info.push_next(&mut debug_create_info);
        }

        let instance = unsafe { entry.create_instance(&instance_info, None)? };

        self.setup_debug_messenger(&entry, &instance)?;

        self.entry = Some(entry);
        self.instance = Some(instance);

        Ok(())
    }

    fn get_required_validation_layers(&self) -> Result<Vec<&'static CStr>> {
        let mut validation_layers: Vec<&'static CStr> = vec![&KHRONOS_VALIDATION];

        if self.enable_validation_layer {
            validation_layers.push(&KHRONOS_VALIDATION);
        }

        Ok(validation_layers)
    }

    fn get_required_extension(&self) -> Result<Vec<&'static CStr>> {
        let mut extensions =
            ash_window::enumerate_required_extensions(self.window.as_ref().unwrap())?;

        if self.enable_validation_layer {
            extensions.push(ash::extensions::ext::DebugUtils::name());
        }

        debug!("Required extensions: {:?}", extensions);

        Ok(extensions)
    }

    fn check_extension_support(
        &self,
        entry: &ash::Entry,
        required: &Vec<*const c_char>,
    ) -> Result<()> {
        let supported = entry.enumerate_instance_extension_properties()?;

        debug!("Supported extensions: {:?}", supported);

        for &req in required {
            let in_supported = supported.iter().any(|ext| unsafe {
                // really is unsafe
                CStr::from_ptr(ext.extension_name.as_ptr()) == CStr::from_ptr(req)
            });

            if !in_supported {
                anyhow::bail!("Required extension is unsupported: {}", unsafe {
                    CStr::from_ptr(req).to_str()?
                });
            }
        }

        Ok(())
    }

    fn check_validation_layer_support(
        &self,
        entry: &ash::Entry,
        layers: &Vec<*const c_char>,
    ) -> Result<()> {
        let available_layers = entry.enumerate_instance_layer_properties()?;

        for &req in layers {
            let in_supported = available_layers.iter().any(|layer| unsafe {
                CStr::from_ptr(layer.layer_name.as_ptr()) == CStr::from_ptr(req)
            });

            if !in_supported {
                anyhow::bail!(
                    "Required layer is unsupported: {:?} {:?}",
                    unsafe { CStr::from_ptr(req) },
                    available_layers
                );
            }
        }

        Ok(())
    }

    fn populate_debug_messenger_create_info(
        &self,
    ) -> Result<DebugUtilsMessengerCreateInfoEXTBuilder> {
        Ok(vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(vulkan_debug_callback)))
    }

    fn setup_debug_messenger(
        &mut self,
        entry: &ash::Entry,
        instance: &ash::Instance,
    ) -> Result<()> {
        if !self.enable_validation_layer {
            return Ok(());
        }

        let debug_utils_loader = DebugUtils::new(&entry, &instance);

        let debug_create_info = self.populate_debug_messenger_create_info()?;

        self.debug_callback = Some(unsafe {
            debug_utils_loader
                .create_debug_utils_messenger(&debug_create_info, None)
                .context("Failed to load debug callback")?
        });

        self.debug_utils_loader = Some(debug_utils_loader);

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
            if let (Some(debug_utils_loader), Some(debug_callback)) =
                (self.debug_utils_loader.take(), self.debug_callback.take())
            {
                debug_utils_loader.destroy_debug_utils_messenger(debug_callback, None)
            }

            if let Some(instance) = self.instance.take() {
                instance.destroy_instance(None);
            }
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();
    VulkanApp::new("Vulkan", 800, 600)?.run()
}
