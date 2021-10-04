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

use ash::extensions::{ext::DebugUtils, khr::Surface as SurfaceLoader};
use ash::vk::{self, DebugUtilsMessengerCreateInfoEXTBuilder};
//use ash::vk::{ApplicationInfo, StructureType};

#[allow(dead_code)]
struct VulkanApp {
    //name: String,
    window: Window,
    event_loop: Option<EventLoop<()>>,
    instance: ash::Instance,
    entry: ash::Entry,
    physical_device: vk::PhysicalDevice,
    logical_device: ash::Device,
    graphics_queue: vk::Queue,
    presentation_queue: vk::Queue,
    debug_callback: Option<vk::DebugUtilsMessengerEXT>,
    debug_utils_loader: Option<DebugUtils>,
    surface: vk::SurfaceKHR,
    surface_loader: SurfaceLoader,
}

lazy_static! {
    static ref VALIDATION_LAYERS: [&'static CStr; 1] =
        [CStr::from_bytes_with_nul("VK_LAYER_KHRONOS_validation\0".as_bytes()).unwrap()];
    static ref APP_NAME: CString = CString::new("Vulkan".as_bytes()).unwrap();
    static ref ENGINE_NAME: CString = CString::new("No engine".as_bytes()).unwrap();
}

//#[derive(Default)]
//struct Config {
//    window_size: LogicalSize<u32>,
//    resizable: bool,
//}

struct QueueFamilyIndices {
    graphics_family: Option<u32>,
    presentation_family: Option<u32>,
}

impl QueueFamilyIndices {
    fn is_complete(&self) -> bool {
        self.graphics_family.is_some() && self.presentation_family.is_some()
    }
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
        let enable_validation_layer = true;

        let (window, event_loop) = Self::init_window(name, (width, height), true)?;
        let (entry, instance) = Self::create_instance(&window, enable_validation_layer)?;
        let (surface, surface_loader) = Self::create_surface(&entry, &instance, &window)?;

        let mut debug_callback = None;
        let mut debug_utils_loader = None;
        if let Some((debug_callback_, debug_utils_loader_)) =
            Self::setup_debug_messenger(&entry, &instance, enable_validation_layer)?
        {
            debug_callback = Some(debug_callback_);
            debug_utils_loader = Some(debug_utils_loader_);
        };

        let physical_device = Self::pick_physical_device(&instance, surface, &surface_loader)?;
        let (logical_device, graphics_queue, presentation_queue) =
            Self::create_logical_device(&instance, physical_device, surface, &surface_loader, enable_validation_layer)?;

        let app = VulkanApp {
            window,
            event_loop: Some(event_loop),
            instance,
            entry,
            debug_callback,
            debug_utils_loader,
            logical_device,
            physical_device,
            graphics_queue,
            presentation_queue,
            surface,
            surface_loader,
        };
        Ok(app)
    }

    pub fn run(mut self) -> Result<()> {
        let id = self.window.id();
        if let Some(event_loop) = self.event_loop.take() {
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
        } else {
            anyhow::bail!("event loop uninitialised")
        }
    }

    fn create_instance(
        window: &Window,
        enable_validation_layer: bool,
    ) -> Result<(ash::Entry, ash::Instance)> {
        let app_info = vk::ApplicationInfo::builder()
            .application_name(&APP_NAME)
            .application_version(vk::make_api_version(1, 0, 0, 0))
            .engine_name(&ENGINE_NAME)
            .engine_version(vk::make_api_version(1, 0, 0, 0))
            .api_version(vk::make_api_version(1, 0, 0, 0));

        let entry = unsafe { ash::Entry::new()? };

        //let extensions = ash_window::enumerate_required_extensions(self.window.as_ref().unwrap())?;
        let extensions = Self::get_required_extension(window, enable_validation_layer)?;

        let extension_ptrs: Vec<*const c_char> = extensions.iter().map(|s| s.as_ptr()).collect();

        Self::check_extension_support(&entry, &extension_ptrs)?;

        let validation_layers = Self::get_required_validation_layers(enable_validation_layer)?;

        let validation_layer_ptrs = validation_layers.iter().map(|l| l.as_ptr()).collect();

        Self::check_validation_layer_support(&entry, &validation_layer_ptrs)?;

        let mut instance_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_ptrs)
            .enabled_layer_names(&validation_layer_ptrs);

        let mut debug_create_info;

        if enable_validation_layer {
            debug_create_info = Self::populate_debug_messenger_create_info()?;
            instance_info = instance_info.push_next(&mut debug_create_info);
        }

        let instance = unsafe { entry.create_instance(&instance_info, None)? };

        Ok((entry, instance))
    }

    fn pick_physical_device(instance: &ash::Instance, surface: vk::SurfaceKHR, surface_loader: &SurfaceLoader) -> Result<vk::PhysicalDevice> {
        let instance = instance;
        let physical_device = unsafe {
            instance
                .enumerate_physical_devices()?
                .into_iter()
                .find(|&device| Self::is_device_suitable(instance, device, surface, surface_loader).unwrap())
        };

        if let Some(physical_device) = physical_device {
            Ok(physical_device)
        } else {
            anyhow::bail!("Failed to find suitable device")
        }
    }

    fn create_logical_device(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &SurfaceLoader,
        enable_validation_layer: bool,
    ) -> Result<(ash::Device, vk::Queue, vk::Queue)> {
        let indices = Self::find_queue_families(instance, physical_device, surface, surface_loader)?;

        if !indices.is_complete() {
            anyhow::bail!("incomplete queue family support");
        }

        let queue_create_info = [
            *vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(indices.graphics_family.unwrap())
                .queue_priorities(&[1.0]),
            *vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(indices.presentation_family.unwrap())
                .queue_priorities(&[1.0]),
        ];

        let features = vk::PhysicalDeviceFeatures::builder();

        let mut create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_info)
            .enabled_features(&features);

        let validation_layers = Self::get_required_validation_layers(enable_validation_layer)?;
        let validation_layer_ptrs: Vec<*const c_char> =
            validation_layers.iter().map(|l| l.as_ptr()).collect();
        if enable_validation_layer {
            create_info = create_info.enabled_layer_names(&validation_layer_ptrs);
        }

        let device = unsafe { instance.create_device(physical_device, &create_info, None)? };

        let graphics_queue = unsafe { device.get_device_queue(indices.graphics_family.unwrap(), 0) };
        let presentation_queue = unsafe { device.get_device_queue(indices.presentation_family.unwrap(), 0) };

        Ok((device, graphics_queue, presentation_queue))
    }

    fn create_surface(
        entry: &ash::Entry,
        instance: &ash::Instance,
        window: &Window,
    ) -> Result<(vk::SurfaceKHR, SurfaceLoader)> {
        let surface_loader = SurfaceLoader::new(entry, instance);
        let surface = unsafe { ash_window::create_surface(entry, instance, window, None)? };
        Ok((surface, surface_loader))
    }

    unsafe fn is_device_suitable(
        instance: &ash::Instance,
        device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &SurfaceLoader,
    ) -> Result<bool> {
        let indices = Self::find_queue_families(instance, device, surface, surface_loader)?;

        return Ok(indices.is_complete());
    }

    fn find_queue_families(
        instance: &ash::Instance,
        device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &SurfaceLoader,
    ) -> Result<QueueFamilyIndices> {
        let instance = instance;
        let mut indices = QueueFamilyIndices {
            graphics_family: None,
            presentation_family: None,
        };

        let families = unsafe { instance.get_physical_device_queue_family_properties(device) };
        for (index, family) in families.iter().enumerate() {
            if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                indices.graphics_family = Some(index as u32);
            }

            let supports_surface = unsafe { surface_loader.get_physical_device_surface_support(device, index as u32, surface)? };
            if supports_surface {
                indices.presentation_family = Some(index as u32);
            }
        }

        Ok(indices)
    }

    fn get_required_validation_layers(enable_validation_layer: bool) -> Result<Vec<&'static CStr>> {
        if enable_validation_layer {
            Ok(VALIDATION_LAYERS.to_vec())
        } else {
            Ok(vec![])
        }
    }

    fn get_required_extension(
        window: &Window,
        enable_validation_layer: bool,
    ) -> Result<Vec<&'static CStr>> {
        let mut extensions = ash_window::enumerate_required_extensions(window)?;

        if enable_validation_layer {
            extensions.push(ash::extensions::ext::DebugUtils::name());
        }

        debug!("Required extensions: {:?}", extensions);

        Ok(extensions)
    }

    fn check_extension_support(entry: &ash::Entry, required: &Vec<*const c_char>) -> Result<()> {
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

    fn populate_debug_messenger_create_info<'b>(
    ) -> Result<DebugUtilsMessengerCreateInfoEXTBuilder<'b>> {
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
        entry: &ash::Entry,
        instance: &ash::Instance,
        enable_validation_layer: bool,
    ) -> Result<Option<(vk::DebugUtilsMessengerEXT, DebugUtils)>> {
        //let entry = self.entry.as_ref().context("entry is None")?;
        //let instance = self.instance.as_ref().context("instance is None")?;
        if !enable_validation_layer {
            return Ok(None);
        }

        let debug_utils_loader = DebugUtils::new(&entry, &instance);

        let debug_create_info = Self::populate_debug_messenger_create_info()?;

        let debug_callback = unsafe {
            debug_utils_loader
                .create_debug_utils_messenger(&debug_create_info, None)
                .context("Failed to load debug callback")?
        };

        Ok(Some((debug_callback, debug_utils_loader)))
    }

    fn init_window(
        name: &str,
        window_size: (u32, u32),
        resizable: bool,
    ) -> Result<(Window, EventLoop<()>)> {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title(name)
            .with_inner_size(LogicalSize::<u32>::from(window_size))
            .with_resizable(resizable)
            .build(&event_loop)
            .context("Failed to create event loop")?;

        Ok((window, event_loop))
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

            self.logical_device.destroy_device(None);

            self.surface_loader.destroy_surface(self.surface, None);

            self.instance.destroy_instance(None);
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();
    VulkanApp::new("Vulkan", 800, 600)?.run()
}
