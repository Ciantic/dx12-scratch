/// This roughly replicates
/// https://github.com/microsoft/DirectX-Graphics-Samples/blob/master/Samples/Desktop/D3D12HelloWorld/src/HelloTriangle/D3D12HelloTriangle.cpp
///
/// Includes some changes what I want e.g. transparency
///
use core::mem::MaybeUninit;
use ptr::{null, null_mut};
use std::{
    convert::TryInto,
    ffi::{c_void, CString},
    mem, ptr,
};
use winapi::shared::dxgi::*;
use winapi::shared::dxgi1_2::*;
use winapi::shared::dxgi1_3::*;
use winapi::shared::dxgi1_4::*;
use winapi::shared::dxgiformat::*;
use winapi::shared::dxgitype::*;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::d3d12::*;
use winapi::um::d3d12sdklayers::*;
use winapi::um::d3dcommon::*;
use winapi::um::d3dcompiler::*;
use winapi::um::dcomp::*;
use winapi::um::synchapi::*;
use winapi::um::winnt::*;
use winapi::um::winuser;
use winapi::vc::limits::UINT_MAX;
use winapi::Interface;
use wio::com::ComPtr;

const NUM_OF_FRAMES: usize = 2;

#[derive(Debug)]
#[repr(C)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}
impl Vertex {
    const fn new(position: [f32; 3], color: [f32; 4]) -> Self {
        Self { position, color }
    }
}

#[allow(dead_code)]
struct Window {
    factory: ComPtr<IDXGIFactory4>,
    adapter: ComPtr<IDXGIAdapter1>,
    device: ComPtr<ID3D12Device>,
    queue: ComPtr<ID3D12CommandQueue>,
    allocator: ComPtr<ID3D12CommandAllocator>,
    comp_device: ComPtr<IDCompositionDevice>,
    swap_chain: ComPtr<IDXGISwapChain3>,
    list: ComPtr<ID3D12GraphicsCommandList>,
    desc_heap: ComPtr<ID3D12DescriptorHeap>,
    desc_size: usize,
    comp_target: ComPtr<IDCompositionTarget>,
    comp_visual: ComPtr<IDCompositionVisual>,
    resources: [ComPtr<ID3D12Resource>; NUM_OF_FRAMES],
    pipeline_state: ComPtr<ID3D12PipelineState>,
    root_signature: ComPtr<ID3D12RootSignature>,
    vertex_shader: ComPtr<ID3DBlob>,
    pixel_shader: ComPtr<ID3DBlob>,
    viewport: D3D12_VIEWPORT,
    scissor: D3D12_RECT,

    // Synchronization
    fence: ComPtr<ID3D12Fence>,
    fence_value: u64,
    fence_event: HANDLE,

    // Resources
    vertex_buffer: ComPtr<ID3D12Resource>,
    vertex_buffer_view: D3D12_VERTEX_BUFFER_VIEW,
}

// fn hr(hresult: HRESULT, ptr: *mut *mut c_void)  -> ComPtr<T>

impl Window {
    /// Create drawing resources for the window
    pub fn new(hwnd: HWND) -> Self {
        println!("HWND {}", hwnd as u32);

        // Start "DebugView" to listen errors
        // https://docs.microsoft.com/en-us/sysinternals/downloads/debugview
        let debug = unsafe {
            let mut ptr = null_mut::<ID3D12Debug>();
            let hr = D3D12GetDebugInterface(
                &ID3D12Debug::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to create debug layer");

        unsafe {
            debug.EnableDebugLayer();
        }

        // Create Factory4
        let factory = unsafe {
            let mut ptr = null_mut::<IDXGIFactory4>();
            let hr = CreateDXGIFactory2(
                0,
                &IDXGIFactory4::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to create factory4");

        // Get any D3D adapter
        let adapter = (0..99)
            .into_iter()
            .find_map(|i| unsafe {
                let mut ptr = null_mut::<IDXGIAdapter1>();
                let hr = factory.EnumAdapters1(i, &mut ptr as *mut *mut _ as *mut *mut _);
                (hr == 0).then(|| ComPtr::from_raw(ptr))
            })
            .expect("Could not find d3d adapter");

        // Create device
        let device = unsafe {
            let mut ptr = null_mut::<ID3D12Device>();
            let hr = D3D12CreateDevice(
                adapter.as_raw() as *mut _,
                D3D_FEATURE_LEVEL_11_0,
                &ID3D12Device::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to create device");

        // Create command queue
        let queue = unsafe {
            let mut ptr = null_mut::<ID3D12CommandQueue>();
            let desc = D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                Priority: D3D12_COMMAND_QUEUE_PRIORITY_HIGH as _,
                Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
                NodeMask: 0,
            };
            let hr = device.CreateCommandQueue(
                &desc,
                &ID3D12CommandQueue::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to create command queue");

        // Create command allocator
        let allocator = unsafe {
            let mut ptr = null_mut::<ID3D12CommandAllocator>();
            let hr = device.CreateCommandAllocator(
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &ID3D12CommandAllocator::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to create command allocator");

        // Factory 2
        // let factory2 = factory
        //     .cast::<IDXGIFactory2>()
        //     .expect("Unable to cast to factory2");

        // Composition device
        let comp_device = unsafe {
            let mut ptr = null_mut::<IDCompositionDevice>();
            let hr = DCompositionCreateDevice(
                0 as _,
                &IDCompositionDevice::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to create composition device");

        // Create swap chain for composition
        let swap_chain = unsafe {
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                BufferCount: NUM_OF_FRAMES as _,
                Width: 1024,
                Height: 1024,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Flags: 0,
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Scaling: DXGI_SCALING_STRETCH,
                Stereo: TRUE,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            };
            let mut ptr = null_mut::<IDXGISwapChain1>();
            let hr = factory.CreateSwapChainForComposition(
                queue.as_raw() as _,
                &desc,
                null_mut(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );

            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to create swapchain")
        .cast::<IDXGISwapChain3>()
        .expect("Unable to cast to swapchain");

        // Create IDCompositionTarget for the window
        let comp_target = unsafe {
            let mut ptr = null_mut::<IDCompositionTarget>();
            let hr = comp_device.CreateTargetForHwnd(
                hwnd as _,
                1,
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to composition target");

        // Create IDCompositionTarget for the window
        let comp_visual = unsafe {
            let mut ptr = null_mut::<IDCompositionVisual>();
            let hr = comp_device.CreateVisual(&mut ptr as *mut *mut _ as *mut *mut _);
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to composition visual");

        // Set swap_chain and the root visual and commit
        unsafe {
            comp_visual.SetContent(swap_chain.as_raw() as _);
            comp_target.SetRoot(comp_visual.as_raw() as _);
            comp_device.Commit();
        }

        // // Create swap chain for composition
        // let swap_chain = {
        //     let sw = {
        //         let (ptr, hr) = factory2.create_swapchain_for_hwnd(
        //             queue,
        //             hwnd,
        //             &SwapchainDesc {
        //                 width: 1024,
        //                 height: 1024,
        //                 format: DXGI_FORMAT_B8G8R8A8_UNORM,
        //                 stereo: false,
        //                 sample: SampleDesc {
        //                     count: 1,
        //                     quality: 0,
        //                 },
        //                 buffer_usage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        //                 buffer_count: NUM_OF_FRAMES as _,
        //                 scaling: Scaling::Stretch,
        //                 swap_effect: SwapEffect::FlipSequential,
        //                 alpha_mode: AlphaMode::Ignore,
        //                 flags: 0,
        //             },
        //         );
        //         (hr == 0).then(|| ptr)
        //     }
        //     .expect("Unable to create swapchain");
        //     let (ptr, hr) = unsafe { sw.cast::<IDXGISwapChain3>() };
        //     (hr == 0).then(|| ptr)
        // }
        // .expect("Unable to cast swapchain");

        // Create heap descriptor
        let desc_heap = unsafe {
            let desc = D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: NUM_OF_FRAMES as _,
                Flags: 0,
                NodeMask: 0,
            };
            let mut ptr = null_mut::<ID3D12DescriptorHeap>();
            let hr = device.CreateDescriptorHeap(
                &desc,
                &ID3D12DescriptorHeap::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to create heap descriptor thing");

        // Create resource per frame
        let mut descriptor = unsafe { desc_heap.GetCPUDescriptorHandleForHeapStart() };
        let desc_size = unsafe {
            device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize
        };

        let resources = (0..NUM_OF_FRAMES)
            .map(|i| {
                let resource = unsafe {
                    let mut ptr = null_mut::<ID3D12Resource>();
                    let hr = swap_chain.GetBuffer(
                        i as _,
                        &ID3D12Resource::uuidof(),
                        &mut ptr as *mut *mut _ as *mut *mut _,
                    );
                    (hr == 0).then(|| ComPtr::from_raw(ptr))
                }
                .expect("Unable to create resource");

                unsafe {
                    // let desc = D3D12_TEX2D_RTV {
                    //     Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    //     u: D3D12_RTV_DIMENSION_UNKNOWN as _,
                    //     ViewDimension: 0,
                    // };
                    device.CreateRenderTargetView(resource.as_raw(), 0 as _, descriptor);
                    descriptor.ptr += desc_size as usize;
                }

                resource
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("Unable to get resources as array");

        let root_signature = unsafe {
            let root = {
                let mut blob = null_mut::<ID3DBlob>();
                let mut error = null_mut::<ID3DBlob>();

                let desc = D3D12_ROOT_SIGNATURE_DESC {
                    NumParameters: 0,
                    pParameters: null_mut() as _,
                    NumStaticSamplers: 0,
                    pStaticSamplers: null_mut() as _,
                    Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
                };

                if D3D12SerializeRootSignature(
                    &desc,
                    D3D_ROOT_SIGNATURE_VERSION_1_0,
                    &mut blob as _,
                    &mut error as _,
                ) != 0
                {
                    panic!("Unable to serialize root signature (serialization)");
                }

                if !error.is_null() {
                    panic!("Unable to serialize root signature (error blobbie)");
                }

                ComPtr::from_raw(blob)
            };
            let mut ptr = null_mut::<ID3D12RootSignature>();
            let hr = device.CreateRootSignature(
                0,
                root.GetBufferPointer(),
                root.GetBufferSize(),
                &ID3D12RootSignature::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to create root signature");

        let vertex_shader = unsafe {
            let data = include_bytes!("./simple.hlsl");
            let mut err = null_mut::<ID3DBlob>();
            let mut ptr = null_mut::<ID3DBlob>();
            let hr = D3DCompile(
                data.as_ptr() as LPCVOID,
                data.len(),
                "simple.hlsl\0".as_ptr() as _,
                null(),
                null_mut(),
                "VSMain\0".as_ptr() as _,
                "vs_5_0\0".as_ptr() as _,
                0,
                0,
                &mut ptr,
                &mut err,
            );
            if !err.is_null() {
                let err = ComPtr::from_raw(err);
                let errstr = CString::from_raw(err.GetBufferPointer() as _);
                panic!("Shader creation failed {}", errstr.to_string_lossy());
            }
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Could not create vertex shader");

        let pixel_shader = unsafe {
            let data = include_bytes!("./simple.hlsl");
            let mut err = null_mut::<ID3DBlob>();
            let mut ptr = null_mut::<ID3DBlob>();
            let hr = D3DCompile(
                data.as_ptr() as LPCVOID,
                data.len(),
                "simple.hlsl\0".as_ptr() as _,
                null(),
                null_mut(),
                "PSMain\0".as_ptr() as _,
                "ps_5_0\0".as_ptr() as _,
                0,
                0,
                &mut ptr,
                &mut err,
            );
            if !err.is_null() {
                let err = ComPtr::from_raw(err);
                let errstr = CString::from_raw(err.GetBufferPointer() as _);
                panic!("Shader creation failed {}", errstr.to_string_lossy());
            }
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Could not create vertex shader");

        // Pipeline state
        let els = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: "POSITION\0".as_ptr() as _,
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32_FLOAT,
                InputSlot: 0,
                InstanceDataStepRate: 0,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                AlignedByteOffset: 0,
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: "COLOR\0".as_ptr() as _,
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                InputSlot: 0,
                InstanceDataStepRate: 0,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                AlignedByteOffset: 12,
            },
        ];
        let pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: root_signature.as_raw(),
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                NumElements: els.len() as u32,
                pInputElementDescs: els.as_ptr(),
            },
            // CD3DX12_RASTERIZER_DESC( CD3DX12_DEFAULT )
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_BACK,
                FrontCounterClockwise: FALSE,
                DepthBias: D3D12_DEFAULT_DEPTH_BIAS as _,
                DepthBiasClamp: D3D12_DEFAULT_DEPTH_BIAS_CLAMP,
                SlopeScaledDepthBias: D3D12_DEFAULT_SLOPE_SCALED_DEPTH_BIAS,
                DepthClipEnable: TRUE,
                MultisampleEnable: FALSE,
                AntialiasedLineEnable: FALSE,
                ForcedSampleCount: 0,
                ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
            },
            // CD3DX12_BLEND_DESC(D3D12_DEFAULT)
            BlendState: D3D12_BLEND_DESC {
                AlphaToCoverageEnable: FALSE,
                IndependentBlendEnable: FALSE,
                RenderTarget: {
                    (0..D3D12_SIMULTANEOUS_RENDER_TARGET_COUNT)
                        .map(|_| D3D12_RENDER_TARGET_BLEND_DESC {
                            BlendEnable: FALSE,
                            LogicOpEnable: FALSE,
                            DestBlend: D3D12_BLEND_ZERO,
                            SrcBlend: D3D12_BLEND_ZERO,
                            DestBlendAlpha: D3D12_BLEND_ONE,
                            SrcBlendAlpha: D3D12_BLEND_ONE,
                            BlendOp: D3D12_BLEND_OP_ADD,
                            LogicOp: D3D12_LOGIC_OP_NOOP,
                            BlendOpAlpha: D3D12_BLEND_OP_ADD,
                            RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL as _,
                        })
                        .collect::<Vec<_>>()
                        .as_slice()
                        .try_into()
                        .unwrap()
                },
            },
            VS: D3D12_SHADER_BYTECODE {
                BytecodeLength: unsafe { vertex_shader.GetBufferSize() },
                pShaderBytecode: unsafe { vertex_shader.GetBufferPointer() },
            },
            PS: D3D12_SHADER_BYTECODE {
                BytecodeLength: unsafe { pixel_shader.GetBufferSize() },
                pShaderBytecode: unsafe { pixel_shader.GetBufferPointer() },
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                DepthEnable: FALSE,
                StencilEnable: FALSE,
                ..unsafe { mem::zeroed() }
            },
            // CD3DX12_DEPTH_STENCIL_DESC( CD3DX12_DEFAULT )
            // DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
            //     DepthEnable: TRUE,
            //     DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL,
            //     DepthFunc: D3D12_COMPARISON_FUNC_LESS,
            //     StencilEnable: FALSE,
            //     StencilReadMask: D3D12_DEFAULT_STENCIL_READ_MASK as _,
            //     StencilWriteMask: D3D12_DEFAULT_STENCIL_WRITE_MASK as _,
            //     FrontFace: D3D12_DEPTH_STENCILOP_DESC {
            //         StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
            //         StencilFailOp: D3D12_STENCIL_OP_KEEP,
            //         StencilPassOp: D3D12_STENCIL_OP_KEEP,
            //         StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
            //     },
            //     BackFace: D3D12_DEPTH_STENCILOP_DESC {
            //         StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
            //         StencilFailOp: D3D12_STENCIL_OP_KEEP,
            //         StencilPassOp: D3D12_STENCIL_OP_KEEP,
            //         StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
            //     },
            // },
            SampleMask: UINT_MAX,
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: {
                (0..D3D12_SIMULTANEOUS_RENDER_TARGET_COUNT)
                    .map(|i| {
                        if i == 0 {
                            DXGI_FORMAT_R8G8B8A8_UNORM
                        } else {
                            DXGI_FORMAT_UNKNOWN
                        }
                    })
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            },
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            DSVFormat: DXGI_FORMAT_UNKNOWN,
            // GS: D3D12_SHADER_BYTECODE
            // StreamOutput:
            ..unsafe { std::mem::zeroed() }
        };

        // Create graphics pipeline state
        let pipeline_state = unsafe {
            let mut ptr = null_mut::<ID3D12PipelineState>();
            let hr = device.CreateGraphicsPipelineState(
                &pso_desc,
                &ID3D12PipelineState::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            println!("{}", hr as u32);
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to create pipeline state");

        // Create command list
        let list = unsafe {
            let mut ptr = null_mut::<ID3D12GraphicsCommandList>();
            let hr = device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                allocator.as_raw(),
                pipeline_state.as_raw(),
                &ID3D12GraphicsCommandList::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            (hr == 0).then(|| {
                let ptr = ComPtr::from_raw(ptr);
                ptr.Close();
                ptr
            })
        }
        .expect("Unable to create command list");

        // Viewport
        let viewport = D3D12_VIEWPORT {
            Width: 1024.0,
            Height: 1024.0,
            MaxDepth: D3D12_MAX_DEPTH,
            MinDepth: D3D12_MIN_DEPTH,
            TopLeftX: 0.0,
            TopLeftY: 0.0,
        };

        let scissor = D3D12_RECT {
            top: 0,
            left: 0,
            bottom: 1024,
            right: 1024,
        };

        // Create fence
        let (fence, fence_value, fence_event) = unsafe {
            let mut fence = null_mut::<ID3D12Fence>();
            let hr = device.CreateFence(
                0,
                D3D12_FENCE_FLAG_NONE,
                &ID3D12Fence::uuidof(),
                &mut fence as *mut *mut _ as *mut *mut _,
            );
            let fence_event = CreateEventA(null_mut(), FALSE, FALSE, null());
            if hr != 0 {
                panic!("Unable to create fence")
            }
            if fence_event.is_null() {
                panic!("Unable to create fence event");
            }
            (ComPtr::from_raw(fence), 0, fence_event)
        };

        let (vertex_buffer, vertex_buffer_view) = unsafe {
            // Blue end of the triangle is semi transparent
            let ar = 1.0;
            let scale = 1.0;
            let cpu_triangle: [Vertex; 3] = [
                Vertex::new([0.0, scale * ar, 0.0], [1.0, 0.0, 0.0, 1.0]),
                Vertex::new([scale, -scale * ar, 0.0], [0.0, 1.0, 0.0, 1.0]),
                Vertex::new([-scale, -scale * ar, 0.0], [0.0, 0.0, 1.0, 0.5]),
            ];
            let triangle_size_bytes = mem::size_of_val(&cpu_triangle);
            let props = D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_UPLOAD,
                CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
                CreationNodeMask: 1,
                VisibleNodeMask: 1,
                MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
            };
            let desc = D3D12_RESOURCE_DESC {
                Alignment: 0,
                Flags: D3D12_RESOURCE_FLAG_NONE,
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                DepthOrArraySize: 1,
                Format: DXGI_FORMAT_UNKNOWN,
                Height: 1,
                Width: triangle_size_bytes as u64,
                Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                MipLevels: 1,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
            };
            // let clr = D3D12_CLEAR_VALUE {
            //     Format: DXGI_FORMAT_UNKNOWN,

            // };
            let mut ptr = null_mut::<ID3D12Resource>();
            let hr = device.CreateCommittedResource(
                &props,
                D3D12_HEAP_FLAG_NONE,
                &desc,
                D3D12_RESOURCE_STATE_GENERIC_READ,
                null(),
                &ID3D12Resource::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            if hr != 0 {
                panic!("Unable to create triangle resource");
            }
            let vertex_buffer = ComPtr::from_raw(ptr);
            let mut gpu_triangle = null_mut::<Vertex>();
            if vertex_buffer.Map(
                0,
                &D3D12_RANGE { Begin: 0, End: 0 },
                &mut gpu_triangle as *mut *mut _ as *mut *mut _,
            ) != 0
            {
                panic!("Unable to map vertex data");
            }
            if gpu_triangle.is_null() {
                panic!("Nullptr");
            }
            std::ptr::copy_nonoverlapping(cpu_triangle.as_ptr(), gpu_triangle as *mut _, 3);
            let gpu_slice = std::slice::from_raw_parts(gpu_triangle, 3);
            println!("{:?}", cpu_triangle);
            println!("{:?}", gpu_slice);
            vertex_buffer.Unmap(0, null());
            let vertex_buffer_view = D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: vertex_buffer.GetGPUVirtualAddress(),
                StrideInBytes: mem::size_of::<Vertex>() as _,
                SizeInBytes: triangle_size_bytes as _,
            };
            (vertex_buffer, vertex_buffer_view)
        };

        Window {
            factory,
            adapter,
            device,
            queue,
            allocator,
            comp_device,
            swap_chain,
            list,
            desc_heap,
            desc_size,
            comp_target,
            comp_visual,
            resources,
            pipeline_state,
            root_signature,
            vertex_shader,
            pixel_shader,
            fence,
            fence_event,
            fence_value,
            viewport,
            scissor,
            vertex_buffer,
            vertex_buffer_view,
        }
    }

    pub fn populate_command_list(&mut self) {
        let current_frame = unsafe { self.swap_chain.GetCurrentBackBufferIndex() as usize };
        let current_resource = &self.resources[current_frame];
        let desc_cpu = unsafe {
            let mut ptr = self.desc_heap.GetCPUDescriptorHandleForHeapStart();
            ptr.ptr += current_frame * self.desc_size;
            ptr
        };

        if unsafe { self.allocator.Reset() } != 0 {
            panic!("allocator reset failed");
        }

        if unsafe {
            self.list
                .Reset(self.allocator.as_raw(), self.pipeline_state.as_raw())
        } != 0
        {
            panic!("Unable to reset list");
        }

        unsafe {
            self.list
                .SetGraphicsRootSignature(self.root_signature.as_raw());

            self.list.RSSetViewports(1, &self.viewport);
            self.list.RSSetScissorRects(1, &self.scissor);
        };

        let barriers = {
            let mut barrier = D3D12_RESOURCE_BARRIER {
                Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
                ..unsafe { mem::zeroed() }
            };
            unsafe {
                *barrier.u.Transition_mut() = D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: current_resource.as_raw(),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_PRESENT,
                    StateAfter: D3D12_RESOURCE_STATE_RENDER_TARGET,
                };
            }
            [barrier]
        };
        unsafe { self.list.ResourceBarrier(1, barriers.as_ptr()) };

        // set render targets
        unsafe {
            self.list.OMSetRenderTargets(1, &desc_cpu, 0, ptr::null());
            let bg: [FLOAT; 4] = [1.0, 0.2, 0.4, 0.5];
            self.list.ClearRenderTargetView(desc_cpu, &bg, 0, null());
            self.list
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.list.IASetVertexBuffers(0, 1, &self.vertex_buffer_view);
            self.list.DrawInstanced(3, 1, 0, 0);
        }

        let barriers = {
            let mut barrier = D3D12_RESOURCE_BARRIER {
                Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
                ..unsafe { mem::zeroed() }
            };
            unsafe {
                *barrier.u.Transition_mut() = D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: current_resource.as_raw(),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_RENDER_TARGET,
                    StateAfter: D3D12_RESOURCE_STATE_PRESENT,
                };
            }
            [barrier]
        };
        unsafe { self.list.ResourceBarrier(1, barriers.as_ptr()) };

        if unsafe { self.list.Close() } > 0 {
            panic!("Unable to close command list");
        }
    }

    pub fn wait_for_previous_frame(&mut self) {
        // This is bad practice says Microsoft's C++ example
        unsafe {
            let old_fence_value = self.fence_value;
            self.queue.Signal(self.fence.as_raw(), old_fence_value);
            self.fence_value += 1;
            if self.fence.GetCompletedValue() < old_fence_value {
                self.fence
                    .SetEventOnCompletion(old_fence_value, self.fence_event);
                WaitForSingleObject(self.fence_event, 0xFFFFFFFF);
            }
        }
    }

    pub fn render(&mut self) {
        self.populate_command_list();
        unsafe {
            let lists = [self.list.as_raw().cast::<ID3D12CommandList>()];

            self.queue
                .ExecuteCommandLists(lists.len() as _, lists.as_ptr());

            if self.swap_chain.Present(1, 0) != 0 {
                panic!("Present failed");
            }
            println!("Render");
        }
        self.wait_for_previous_frame();
    }
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

/// Main message loop for the window
unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    #[allow(non_upper_case_globals)]
    static mut WINDOW: Option<Window> = None;
    match msg {
        winuser::WM_CREATE => {
            WINDOW = Some(Window::new(hwnd));
            winuser::DefWindowProcA(hwnd, msg, wparam, lparam)
        }
        winuser::WM_PAINT => {
            if let Some(window) = WINDOW.as_mut() {
                window.render();
            }
            winuser::ValidateRect(hwnd, null());
            0
        }
        winuser::WM_DESTROY => {
            winuser::PostQuitMessage(0);
            0
        }
        _ => winuser::DefWindowProcA(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    unsafe {
        winuser::SetThreadDpiAwarenessContext(2 as _); // DPI_AWARENESS_PER_MONITOR_AWARE;
        let cls = winuser::WNDCLASSA {
            style: 0,
            lpfnWndProc: Some(wndproc),
            hInstance: 0 as _,
            lpszClassName: "CompositionCls\0".as_ptr() as _,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hIcon: 0 as _,
            hCursor: winuser::LoadCursorW(0 as _, winuser::IDC_ARROW as _),
            hbrBackground: 0 as _,
            lpszMenuName: 0 as _,
        };
        winuser::RegisterClassA(&cls);
        let hwnd = winuser::CreateWindowExA(
            winuser::WS_EX_NOREDIRECTIONBITMAP,
            "CompositionCls\0".as_ptr() as _,
            "Composition example\0".as_ptr() as _,
            winuser::WS_OVERLAPPEDWINDOW | winuser::WS_VISIBLE,
            winuser::CW_USEDEFAULT,
            winuser::CW_USEDEFAULT,
            winuser::CW_USEDEFAULT,
            winuser::CW_USEDEFAULT,
            0 as _,
            0 as _,
            0 as _,
            0 as _,
        );
        loop {
            let mut msg = MaybeUninit::uninit();
            if winuser::GetMessageA(msg.as_mut_ptr(), hwnd, 0, 0) > 0 {
                winuser::TranslateMessage(msg.as_ptr());
                winuser::DispatchMessageA(msg.as_ptr());
            } else {
                break;
            }
        }
    }
}
