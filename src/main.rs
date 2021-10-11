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

use ash::extensions::{
    ext::DebugUtils,
    khr::{Surface, Swapchain},
};
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
    surface_loader: Surface,
    swapchain: vk::SwapchainKHR,
    swapchain_loader: Swapchain,
    swapchain_extent: vk::Extent2D,
    swapchain_format: vk::Format,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
}

lazy_static! {
    static ref VALIDATION_LAYERS: [&'static CStr; 1] =
        [CStr::from_bytes_with_nul("VK_LAYER_KHRONOS_validation\0".as_bytes()).unwrap()];
    static ref APP_NAME: CString = CString::new("Vulkan".as_bytes()).unwrap();
    static ref ENGINE_NAME: CString = CString::new("No engine".as_bytes()).unwrap();
    static ref DEVICE_EXTENSIONS: [&'static CStr; 1] =
        [CStr::from_bytes_with_nul("VK_KHR_swapchain\0".as_bytes()).unwrap()];
    static ref SHADER_ENTRYPOINT: &'static CStr =
        CStr::from_bytes_with_nul("main\0".as_bytes()).unwrap();
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

struct SwapChainSupportDetails {
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>,
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

        let queue_family_indices =
            Self::find_queue_families(&instance, physical_device, surface, &surface_loader)?;

        let (logical_device, graphics_queue, presentation_queue) = Self::create_logical_device(
            &instance,
            physical_device,
            enable_validation_layer,
            &queue_family_indices,
        )?;

        let (swapchain, swapchain_loader, swapchain_format, swapchain_extent) =
            Self::create_swapchain(
                &instance,
                &logical_device,
                physical_device,
                surface,
                &surface_loader,
                &window,
                &queue_family_indices,
            )?;

        let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };

        let swapchain_image_views =
            Self::create_image_views(&logical_device, &swapchain_images, swapchain_format)?;

        Self::create_graphics_pipeline(&logical_device)?;

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
            swapchain,
            swapchain_images,
            swapchain_loader,
            swapchain_extent,
            swapchain_format,
            swapchain_image_views,
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
                    } => {
                        if window_id == id {
                            *control_flow = ControlFlow::Exit
                        }
                    }
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

    fn create_graphics_pipeline(device: &ash::Device) -> Result<()> {
        let vert_shader_module = Self::create_shader_module(device, "shaders/vert.spv")?;
        let frag_shader_module = Self::create_shader_module(device, "shaders/vert.spv")?;

        let shader_staged = (
            vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vert_shader_module)
                .name(&SHADER_ENTRYPOINT),
            vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(frag_shader_module)
                .name(&SHADER_ENTRYPOINT),
        );

        unsafe {
            device.destroy_shader_module(vert_shader_module, None);
            device.destroy_shader_module(frag_shader_module, None);
        };
        Ok(())
    }

    fn create_shader_module(device: &ash::Device, path: &str) -> Result<vk::ShaderModule> {
        let bitcode_bytes = std::fs::read(path)?;
        let bitcode = bitcode_bytes
            .chunks_exact(4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect::<Vec<u32>>();

        let create_info = vk::ShaderModuleCreateInfo::builder().code(&bitcode);

        unsafe {
            device
                .create_shader_module(&create_info, None)
                .context("could not create shader module")
        }
    }

    fn create_swapchain(
        instance: &ash::Instance,
        device: &ash::Device,
        physical_device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &Surface,
        window: &Window,
        queue_indices: &QueueFamilyIndices,
    ) -> Result<(vk::SwapchainKHR, Swapchain, vk::Format, vk::Extent2D)> {
        let support_details =
            Self::query_swap_chain_support(physical_device, surface, surface_loader)?;

        let surface_format = Self::choose_swap_surface_format(&support_details.formats)?;
        let present_mode = Self::choose_swap_present_mode(support_details.present_modes)?;
        let extent = Self::choose_swap_extent(support_details.capabilities, window)?;

        let max_image_count = support_details.capabilities.max_image_count;
        let mut image_count = support_details.capabilities.min_image_count + 1;

        if max_image_count > 0 && image_count > max_image_count {
            image_count = max_image_count;
        }

        let mut create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .pre_transform(support_details.capabilities.current_transform)
            .present_mode(present_mode)
            .clipped(true)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE);

        let indices = [
            queue_indices.graphics_family.unwrap(),
            queue_indices.presentation_family.unwrap(),
        ];
        if queue_indices.graphics_family != queue_indices.presentation_family {
            create_info = create_info
                .image_sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&indices);
        } else {
            create_info = create_info
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .queue_family_indices(&[]);
        }

        let swapchain_loader = ash::extensions::khr::Swapchain::new(instance, device);

        let swapchain = unsafe { swapchain_loader.create_swapchain(&create_info, None)? };

        Ok((swapchain, swapchain_loader, surface_format.format, extent))
    }

    fn create_image_views(
        device: &ash::Device,
        images: &Vec<vk::Image>,
        format: vk::Format,
    ) -> Result<Vec<vk::ImageView>> {
        let mut image_views = vec![];
        for image in images.iter() {
            let create_info = vk::ImageViewCreateInfo::builder()
                .image(*image)
                .format(format)
                .view_type(vk::ImageViewType::TYPE_2D)
                .components(vk::ComponentMapping {
                    r: vk::ComponentSwizzle::IDENTITY,
                    g: vk::ComponentSwizzle::IDENTITY,
                    b: vk::ComponentSwizzle::IDENTITY,
                    a: vk::ComponentSwizzle::IDENTITY,
                })
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            let view = unsafe { device.create_image_view(&create_info, None)? };

            image_views.push(view);
        }

        Ok(image_views)
    }

    fn pick_physical_device(
        instance: &ash::Instance,
        surface: vk::SurfaceKHR,
        surface_loader: &Surface,
    ) -> Result<vk::PhysicalDevice> {
        let instance = instance;
        let physical_device = unsafe {
            instance
                .enumerate_physical_devices()?
                .into_iter()
                .find(|&device| {
                    Self::is_device_suitable(instance, device, surface, surface_loader).unwrap()
                })
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
        enable_validation_layer: bool,
        indices: &QueueFamilyIndices,
    ) -> Result<(ash::Device, vk::Queue, vk::Queue)> {
        //let indices = Self::find_queue_families(instance, physical_device, surface, surface_loader)?;

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

        let extensions = [ash::extensions::khr::Swapchain::name()];

        let extension_ptrs: Vec<*const c_char> = extensions.iter().map(|s| s.as_ptr()).collect();

        let features = vk::PhysicalDeviceFeatures::builder();

        let mut create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_info)
            .enabled_extension_names(&extension_ptrs)
            .enabled_features(&features);

        let validation_layers = Self::get_required_validation_layers(enable_validation_layer)?;
        let validation_layer_ptrs: Vec<*const c_char> =
            validation_layers.iter().map(|l| l.as_ptr()).collect();
        if enable_validation_layer {
            create_info = create_info.enabled_layer_names(&validation_layer_ptrs);
        }

        let device = unsafe { instance.create_device(physical_device, &create_info, None)? };

        let graphics_queue =
            unsafe { device.get_device_queue(indices.graphics_family.unwrap(), 0) };
        let presentation_queue =
            unsafe { device.get_device_queue(indices.presentation_family.unwrap(), 0) };

        Ok((device, graphics_queue, presentation_queue))
    }

    fn query_swap_chain_support(
        physical_device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &Surface,
    ) -> Result<SwapChainSupportDetails> {
        unsafe {
            let capabilities = surface_loader
                .get_physical_device_surface_capabilities(physical_device, surface)?;
            let formats =
                surface_loader.get_physical_device_surface_formats(physical_device, surface)?;
            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(physical_device, surface)?;
            Ok(SwapChainSupportDetails {
                capabilities,
                formats,
                present_modes,
            })
        }
    }

    fn create_surface(
        entry: &ash::Entry,
        instance: &ash::Instance,
        window: &Window,
    ) -> Result<(vk::SurfaceKHR, Surface)> {
        let surface_loader = Surface::new(entry, instance);
        let surface = unsafe { ash_window::create_surface(entry, instance, window, None)? };
        Ok((surface, surface_loader))
    }

    unsafe fn is_device_suitable(
        instance: &ash::Instance,
        device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &Surface,
    ) -> Result<bool> {
        let indices = Self::find_queue_families(instance, device, surface, surface_loader)?;

        let extensions_supported = Self::check_device_extension_support(instance, device)?;

        let swapchain_support = if extensions_supported {
            let swapchain_support =
                Self::query_swap_chain_support(device, surface, surface_loader)?;
            !swapchain_support.formats.is_empty() && !swapchain_support.present_modes.is_empty()
        } else {
            false
        };

        return Ok(indices.is_complete() && swapchain_support);
    }

    fn choose_swap_surface_format<'a>(
        available_formats: &'a Vec<vk::SurfaceFormatKHR>,
    ) -> Result<&'a vk::SurfaceFormatKHR> {
        for format in available_formats.iter() {
            if format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                && format.format == vk::Format::B8G8R8A8_SRGB
            {
                return Ok(format);
            }
        }

        Ok(available_formats
            .get(0)
            .expect("failed to find an available format"))
    }

    fn choose_swap_present_mode<'a>(
        available_present_modes: Vec<vk::PresentModeKHR>,
    ) -> Result<vk::PresentModeKHR> {
        for mode in available_present_modes {
            if mode == vk::PresentModeKHR::MAILBOX {
                return Ok(mode);
            }
        }

        Ok(vk::PresentModeKHR::FIFO)
    }

    fn choose_swap_extent(
        capabilites: vk::SurfaceCapabilitiesKHR,
        window: &Window,
    ) -> Result<vk::Extent2D> {
        if capabilites.current_extent.width != u32::MAX {
            Ok(capabilites.current_extent)
        } else {
            let window_size = window.inner_size();
            Ok(vk::Extent2D {
                width: window_size.width.clamp(
                    capabilites.min_image_extent.width,
                    capabilites.max_image_extent.width,
                ),
                height: window_size.height.clamp(
                    capabilites.min_image_extent.height,
                    capabilites.max_image_extent.height,
                ),
            })
        }
    }

    fn check_device_extension_support(
        instance: &ash::Instance,
        device: vk::PhysicalDevice,
    ) -> Result<bool> {
        unsafe {
            let extensions = instance.enumerate_device_extension_properties(device)?;

            for vk::ExtensionProperties { extension_name, .. } in extensions {
                if CStr::from_ptr(extension_name.as_ptr())
                    == ash::extensions::khr::Swapchain::name()
                {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn find_queue_families(
        instance: &ash::Instance,
        device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &Surface,
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

            let supports_surface = unsafe {
                surface_loader.get_physical_device_surface_support(device, index as u32, surface)?
            };
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

            for image_view in self.swapchain_image_views.iter() {
                self.logical_device.destroy_image_view(*image_view, None);
            }

            self.logical_device.destroy_device(None);

            self.surface_loader.destroy_surface(self.surface, None);

            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);

            self.instance.destroy_instance(None);
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();
    VulkanApp::new("Vulkan", 800, 600)?.run()
}
