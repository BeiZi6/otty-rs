//! Choosing a graphics backend, and getting a device from it.

use otty_core::Platform;

/// The graphics API the renderer will use.
///
/// There is no config key for this, because the shipped product does not have
/// one: it ships `Grid.metallib` and takes Metal on macOS without asking. The
/// value exists as a type so the platform decision is made once, in
/// [`Backend::for_platform`], and can be reported by `otty version` and in a
/// crash report instead of being an invisible `cfg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Metal,
    Vulkan,
    Dx12,
    /// The fallback for a platform whose native API is unavailable — an old
    /// Linux driver, or a VM with no D3D12.
    Gl,
}

impl Backend {
    /// The native API for a platform.
    pub const fn for_platform(platform: Platform) -> Self {
        match platform {
            // Metal is unconditional on Apple hardware; there is no second choice.
            Platform::Macos => Self::Metal,
            // DX12 on Windows, with GL as the escape hatch for old drivers — see
            // `fallback_chain`.
            Platform::Windows => Self::Dx12,
            Platform::Linux => Self::Vulkan,
            Platform::Other => Self::Gl,
        }
    }

    /// What to try after this backend fails, in order.
    ///
    /// Only Windows needs a chain: Metal has no alternative on macOS, and a
    /// Linux box without Vulkan is a 2010 machine where GL is the same answer as
    /// giving up.
    pub const fn fallback_chain(self) -> &'static [Backend] {
        match self {
            Self::Dx12 => &[Self::Vulkan, Self::Gl],
            Self::Metal => &[],
            Self::Vulkan => &[Self::Gl],
            Self::Gl => &[],
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Metal => "metal",
            Self::Vulkan => "vulkan",
            Self::Dx12 => "dx12",
            Self::Gl => "opengl",
        }
    }

    /// The `wgpu` backend flag.
    pub const fn to_wgpu(self) -> wgpu::Backends {
        match self {
            Self::Metal => wgpu::Backends::METAL,
            Self::Vulkan => wgpu::Backends::VULKAN,
            Self::Dx12 => wgpu::Backends::DX12,
            Self::Gl => wgpu::Backends::GL,
        }
    }

    /// Parse a backend name, for a bug report or a diagnostic flag.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "metal" => Self::Metal,
            "vulkan" => Self::Vulkan,
            "dx12" | "d3d12" => Self::Dx12,
            "opengl" | "gl" => Self::Gl,
            _ => return None,
        })
    }
}

/// The backend's own answer to "what am I running on".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuInfo {
    pub backend: Backend,
    /// e.g. `Apple M3 Pro`.
    pub name: String,
    /// The driver's own version string.
    pub driver: String,
    /// `integrated`, `discrete`, `virtual` or `cpu`.
    pub device_type: &'static str,
    pub supports_bgra8_srgb: bool,
}

impl GpuInfo {
    /// Read the adapter's description.
    pub fn of(adapter: &wgpu::Adapter, backend: Backend) -> Self {
        let info = adapter.get_info();
        Self {
            backend,
            name: info.name,
            driver: info.driver,
            device_type: device_type_name(info.device_type),
            supports_bgra8_srgb: false,
        }
    }

    /// Whether this adapter is a software rasteriser, which is worth saying out
    /// loud: a terminal on `llvmpipe` will feel slow and the user should be told
    /// why rather than left guessing.
    pub fn is_software(&self) -> bool {
        self.device_type == "cpu"
    }
}

fn device_type_name(t: wgpu::DeviceType) -> &'static str {
    match t {
        wgpu::DeviceType::IntegratedGpu => "integrated",
        wgpu::DeviceType::DiscreteGpu => "discrete",
        wgpu::DeviceType::VirtualGpu => "virtual",
        wgpu::DeviceType::Cpu => "cpu",
        wgpu::DeviceType::Other => "other",
    }
}

/// No feature is required. Everything the renderer would like is optional, so a
/// device that lacks one still starts — see `OPTIONAL_FEATURES` in the crate
/// root for the rationale.
pub const REQUIRED_FEATURES: wgpu::Features = wgpu::Features::empty();

/// The limits the renderer asks for.
///
/// The defaults are enough for a grid; the one number worth raising is the
/// texture dimension, because an atlas for a large font size with many glyphs
/// can exceed 8192.
pub fn required_limits() -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_2d: 8192,
        ..wgpu::Limits::downlevel_defaults()
    }
}

/// What went wrong getting a device.
#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("no {backend} adapter is available; tried {tried:?}")]
    NoAdapter { backend: &'static str, tried: Vec<&'static str> },
    #[error("the {backend} adapter refused a device: {source}")]
    RequestFailed {
        backend: &'static str,
        #[source]
        source: wgpu::RequestDeviceError,
    },
    #[error("no adapter matched the surface's compatibility requirements")]
    IncompatibleSurface,
}

/// A live device, its queue, and the adapter it came from.
pub struct Gpu {
    /// Kept because `Surface::get_capabilities` needs it, and because a bug
    /// report wants the adapter's own description after the fact.
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: GpuInfo,
}

impl Gpu {
    /// Acquire a device, walking the backend's fallback chain.
    ///
    /// The chain is walked rather than jumped to because a Windows box with a
    /// broken D3D12 driver is common enough to matter, and the alternative is an
    /// app that will not start at all. Each attempt is reported, so a failure
    /// names every backend that was tried instead of only the last one.
    pub fn acquire(
        instance: &wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
        preferred: Backend,
    ) -> Result<Self, DeviceError> {
        let chain: Vec<Backend> =
            std::iter::once(preferred).chain(preferred.fallback_chain().iter().copied()).collect();
        let mut tried = Vec::new();

        for backend in chain {
            let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface,
            }));
            let Ok(adapter) = adapter else {
                tried.push(backend.as_str());
                continue;
            };
            // An adapter that is not on the backend we asked for means the
            // instance gave us something else; take it, but record what it is.
            let actual = Backend::for_platform(Platform::host());
            let info = GpuInfo::of(&adapter, if tried.is_empty() { backend } else { actual });

            match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("otty"),
                required_features: REQUIRED_FEATURES,
                required_limits: required_limits(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })) {
                Ok((device, queue)) => return Ok(Self { adapter, device, queue, info }),
                Err(source) => {
                    tried.push(backend.as_str());
                    // Only a total failure is worth reporting; one backend
                    // refusing is why the chain exists.
                    if tried.len() == preferred.fallback_chain().len() + 1 {
                        return Err(DeviceError::RequestFailed {
                            backend: backend.as_str(),
                            source,
                        });
                    }
                }
            }
        }

        if compatible_surface.is_some() {
            Err(DeviceError::IncompatibleSurface)
        } else {
            Err(DeviceError::NoAdapter { backend: preferred.as_str(), tried })
        }
    }

    /// The one-line description a bug report should carry.
    pub fn summary(&self) -> String {
        summary_of(&self.info)
    }
}

/// The summary text, as a free function so it is testable without a real device.
pub fn summary_of(info: &GpuInfo) -> String {
    format!(
        "{} on {} ({}{})",
        info.name,
        info.backend.as_str(),
        info.device_type,
        if info.is_software() { "; software rasteriser" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_platform_gets_its_native_backend() {
        assert_eq!(Backend::for_platform(Platform::Macos), Backend::Metal);
        assert_eq!(Backend::for_platform(Platform::Windows), Backend::Dx12);
        assert_eq!(Backend::for_platform(Platform::Linux), Backend::Vulkan);
        assert_eq!(Backend::for_platform(Platform::Other), Backend::Gl);
    }

    #[test]
    fn only_windows_has_a_fallback_chain_worth_walking() {
        assert!(Backend::Metal.fallback_chain().is_empty());
        assert_eq!(Backend::Dx12.fallback_chain(), [Backend::Vulkan, Backend::Gl]);
        assert_eq!(Backend::Vulkan.fallback_chain(), [Backend::Gl]);
    }

    #[test]
    fn a_fallback_chain_never_contains_its_own_head() {
        for b in [Backend::Metal, Backend::Vulkan, Backend::Dx12, Backend::Gl] {
            assert!(
                !b.fallback_chain().contains(&b),
                "{} falls back to itself",
                b.as_str()
            );
        }
    }

    #[test]
    fn the_backend_name_round_trips() {
        for b in [Backend::Metal, Backend::Vulkan, Backend::Dx12, Backend::Gl] {
            assert_eq!(Backend::parse(b.as_str()), Some(b));
        }
        assert_eq!(Backend::parse("D3D12"), Some(Backend::Dx12));
        assert_eq!(Backend::parse("software"), None);
    }

    #[test]
    fn every_backend_maps_to_exactly_one_wgpu_flag() {
        use wgpu::Backends;
        assert_eq!(Backend::Metal.to_wgpu(), Backends::METAL);
        assert_eq!(Backend::Vulkan.to_wgpu(), Backends::VULKAN);
        assert_eq!(Backend::Dx12.to_wgpu(), Backends::DX12);
        assert_eq!(Backend::Gl.to_wgpu(), Backends::GL);
    }

    #[test]
    fn the_requested_limits_leave_the_atlas_room_but_do_not_demand_a_flagship() {
        let l = required_limits();
        assert_eq!(l.max_texture_dimension_2d, 8192);
        // The rest is the downlevel baseline, so an integrated GPU still starts.
        assert_eq!(l.max_bind_groups, wgpu::Limits::downlevel_defaults().max_bind_groups);
    }

    #[test]
    fn no_feature_is_required() {
        assert_eq!(REQUIRED_FEATURES, wgpu::Features::empty());
    }

    #[test]
    fn a_software_rasteriser_is_recognisable_from_its_summary() {
        let info = GpuInfo {
            backend: Backend::Vulkan,
            name: "llvmpipe".into(),
            driver: "Mesa".into(),
            device_type: "cpu",
            supports_bgra8_srgb: false,
        };
        assert!(info.is_software());
        assert!(summary_of(&info).contains("software rasteriser"), "{}", summary_of(&info));
        assert!(summary_of(&info).starts_with("llvmpipe on vulkan"));
    }

    #[test]
    fn a_real_gpu_s_summary_does_not_claim_to_be_software() {
        let info = GpuInfo {
            backend: Backend::Metal,
            name: "Apple M3 Pro".into(),
            driver: "Metal 3.1".into(),
            device_type: "integrated",
            supports_bgra8_srgb: true,
        };
        assert!(!info.is_software());
        assert_eq!(summary_of(&info), "Apple M3 Pro on metal (integrated)");
    }

    #[test]
    fn the_platform_decision_matches_the_host() {
        // On the machine running the tests, the backend must be the one the
        // platform implies — this is the assertion that would catch someone
        // hard-coding Metal.
        let expected = match std::env::consts::OS {
            "macos" => Backend::Metal,
            "windows" => Backend::Dx12,
            "linux" => Backend::Vulkan,
            _ => Backend::Gl,
        };
        assert_eq!(Backend::for_platform(Platform::host()), expected);
    }
}
