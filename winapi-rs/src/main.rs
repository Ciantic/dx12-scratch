use core::mem::MaybeUninit;
use std::{convert::TryInto, mem, ptr, rc::Weak};

use ptr::{null, null_mut};
use winapi::um::{d3d12::*, d3d12sdklayers::ID3D12Debug, unknwnbase::IUnknown, winuser};
use winapi::Interface;
use winapi::{
    shared::{
        dxgi::IDXGIAdapter1, dxgi1_3::*, dxgi1_4::*, dxgiformat::*, dxgitype::*, windef::HWND,
    },
    um::{d3dcommon::*, dcomp::IDCompositionDevice},
};
use winapi::{
    shared::{dxgi::*, dxgi1_2::*, minwindef::*},
    um::dcomp::*,
};
use wio::com::ComPtr;

const NUM_OF_FRAMES: usize = 2;

const CD3DX12_RASTERIZER_DESC_D3D12_DEFAULT: D3D12_RASTERIZER_DESC = D3D12_RASTERIZER_DESC {
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
};

struct Window {
    factory: Option<ComPtr<IDXGIFactory4>>,
    adapter: Option<ComPtr<IDXGIAdapter1>>,
    device: Option<ComPtr<ID3D12Device>>,
    queue: Option<ComPtr<ID3D12CommandQueue>>,
    allocator: Option<ComPtr<ID3D12CommandAllocator>>,
    comp_device: Option<ComPtr<IDCompositionDevice>>,
    swap_chain: Option<ComPtr<IDXGISwapChain3>>,
    list: Option<ComPtr<ID3D12GraphicsCommandList>>,
    desc_heap: Option<ComPtr<ID3D12DescriptorHeap>>,
    desc_size: Option<u32>,
    comp_target: Option<ComPtr<IDCompositionTarget>>,
    comp_visual: Option<ComPtr<IDCompositionVisual>>,
    resources: Option<[ComPtr<ID3D12Resource>; NUM_OF_FRAMES]>,
    // pipeline_state: Option<ComPtr<ID3D12PipelineState>>,
    root_signature: Option<ComPtr<ID3D12RootSignature>>,
}

// fn hr(hresult: HRESULT, ptr: *mut *mut c_void)  -> ComPtr<T>

impl Window {
    /// Create drawing resources for the window
    pub fn create_drawing_resources(&mut self, hwnd: HWND) {
        println!("HWND {}", hwnd as u32);

        // let debug = unsafe {
        //     let mut ptr = null_mut::<ID3D12Debug>();
        //     let hr = D3D12GetDebugInterface(
        //         &ID3D12Debug::uuidof(),
        //         &mut ptr as *mut *mut _ as *mut *mut _,
        //     );
        //     (hr == 0).then(|| ComPtr::from_raw(ptr))
        // }
        // .expect("Unable to create debug layer");

        // unsafe {
        //     debug.EnableDebugLayer();
        // }

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
                &*(adapter.as_raw() as *mut IUnknown) as *const _ as *mut _,
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
        let desc_rtv_size =
            unsafe { device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) };

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
                    descriptor.ptr += desc_rtv_size as usize;
                }

                resource
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("Unable to get resources as array");

        self.factory = Some(factory);
        self.adapter = Some(adapter);
        self.device = Some(device);
        self.queue = Some(queue);
        self.allocator = Some(allocator);
        self.desc_heap = Some(desc_heap);
        self.desc_size = Some(desc_rtv_size);
        self.swap_chain = Some(swap_chain);
        self.comp_device = Some(comp_device);
        self.comp_target = Some(comp_target);
        self.comp_visual = Some(comp_visual);
        self.resources = Some(resources);
    }

    pub fn load_assets(&mut self) {
        let device = self.device.as_ref().unwrap();
        let allocator = self.allocator.as_ref().unwrap();
        let root = {
            let mut blob = null_mut::<ID3DBlob>();
            let mut error = null_mut::<ID3DBlob>();

            let desc = D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: 0,
                pParameters: null_mut() as _,
                NumStaticSamplers: 0,
                pStaticSamplers: null_mut() as _,
                Flags: 0,
            };
            let hr = unsafe {
                D3D12SerializeRootSignature(
                    &desc,
                    D3D_ROOT_SIGNATURE_VERSION_1_0,
                    &mut blob as _,
                    &mut error as _,
                )
            };
            if hr > 0 {
                panic!("Unable to serialize root signature (serialization)");
            }
            if !error.is_null() {
                panic!("Unable to serialize root signature (error blobbie)");
            }
            unsafe { ComPtr::from_raw(blob) }
        };

        let root_signature = unsafe {
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

        // let rtvs = [DXGI_FORMAT_R8G8B8A8_UNORM; 8];
        // let dummy_target = D3D12_RENDER_TARGET_BLEND_DESC {
        //     BlendEnable: FALSE,
        //     LogicOpEnable: FALSE,
        //     SrcBlend: D3D12_BLEND_ZERO,
        //     DestBlend: D3D12_BLEND_ZERO,
        //     BlendOp: D3D12_BLEND_OP_ADD,
        //     SrcBlendAlpha: D3D12_BLEND_ZERO,
        //     DestBlendAlpha: D3D12_BLEND_ZERO,
        //     BlendOpAlpha: D3D12_BLEND_OP_ADD,
        //     LogicOp: D3D12_LOGIC_OP_CLEAR,
        //     RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL as _,
        // };
        // let render_targets = [dummy_target; 8];

        // let input_elements = [D3D12_INPUT_ELEMENT_DESC {
        //     SemanticName: "COLOR".as_ptr() as *const _,
        //     SemanticIndex: 0,
        //     Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
        //     AlignedByteOffset: 0,
        //     InputSlot: 0,
        //     InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
        //     InstanceDataStepRate: 0,
        // }];

        // let mut pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        //     pRootSignature: root_signature.as_raw(),
        //     // VS: Shader::from_blob(vs),
        //     // PS: Shader::from_blob(ps),
        //     VS: D3D12_SHADER_BYTECODE {
        //         BytecodeLength: 0,
        //         pShaderBytecode: ptr::null(),
        //     },
        //     PS: D3D12_SHADER_BYTECODE {
        //         BytecodeLength: 0,
        //         pShaderBytecode: ptr::null(),
        //     },
        //     GS: D3D12_SHADER_BYTECODE {
        //         BytecodeLength: 0,
        //         pShaderBytecode: ptr::null(),
        //     },
        //     DS: D3D12_SHADER_BYTECODE {
        //         BytecodeLength: 0,
        //         pShaderBytecode: ptr::null(),
        //     },
        //     HS: D3D12_SHADER_BYTECODE {
        //         BytecodeLength: 0,
        //         pShaderBytecode: ptr::null(),
        //     },
        //     StreamOutput: D3D12_STREAM_OUTPUT_DESC {
        //         pSODeclaration: ptr::null(),
        //         NumEntries: 0,
        //         pBufferStrides: ptr::null(),
        //         NumStrides: 0,
        //         RasterizedStream: 0,
        //     },
        //     BlendState: D3D12_BLEND_DESC {
        //         AlphaToCoverageEnable: FALSE,
        //         IndependentBlendEnable: FALSE,
        //         RenderTarget: render_targets,
        //     },
        //     SampleMask: !0,
        //     RasterizerState: D3D12_RASTERIZER_DESC {
        //         FillMode: D3D12_FILL_MODE_SOLID,
        //         CullMode: D3D12_CULL_MODE_BACK,
        //         FrontCounterClockwise: FALSE,
        //         DepthBias: D3D12_DEFAULT_DEPTH_BIAS as _,
        //         DepthBiasClamp: D3D12_DEFAULT_DEPTH_BIAS_CLAMP,
        //         SlopeScaledDepthBias: D3D12_DEFAULT_SLOPE_SCALED_DEPTH_BIAS,
        //         DepthClipEnable: TRUE,
        //         MultisampleEnable: FALSE,
        //         AntialiasedLineEnable: FALSE,
        //         ForcedSampleCount: 0,
        //         ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
        //     },
        //     DepthStencilState: unsafe { mem::zeroed() },
        //     InputLayout: D3D12_INPUT_LAYOUT_DESC {
        //         pInputElementDescs: input_elements.as_ptr(),
        //         NumElements: input_elements.len() as _,
        //     },
        //     IBStripCutValue: D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
        //     PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        //     NumRenderTargets: 1,
        //     RTVFormats: rtvs,
        //     DSVFormat: DXGI_FORMAT_UNKNOWN,
        //     SampleDesc: DXGI_SAMPLE_DESC {
        //         Count: 1,
        //         Quality: 0,
        //     },
        //     NodeMask: 0,
        //     CachedPSO: D3D12_CACHED_PIPELINE_STATE {
        //         pCachedBlob: ptr::null(),
        //         CachedBlobSizeInBytes: 0,
        //     },
        //     Flags: D3D12_PIPELINE_STATE_FLAG_NONE,
        // };

        let input_elements = [D3D12_INPUT_ELEMENT_DESC {
            SemanticName: "COLOR\0".as_ptr() as *const _,
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
            AlignedByteOffset: 0,
            InputSlot: 0,
            InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        }];

        let mut pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            ..unsafe { mem::zeroed() }
        };
        pso_desc.InputLayout = D3D12_INPUT_LAYOUT_DESC {
            NumElements: input_elements.len() as _,
            pInputElementDescs: &input_elements as _,
        };
        pso_desc.pRootSignature = root_signature.as_raw();
        pso_desc.RasterizerState = CD3DX12_RASTERIZER_DESC_D3D12_DEFAULT;

        // CD3DX12_BLEND_DESC(D3D12_DEFAULT)
        pso_desc.BlendState = D3D12_BLEND_DESC {
            AlphaToCoverageEnable: FALSE,
            IndependentBlendEnable: FALSE,
            ..unsafe { mem::zeroed() }
        };
        for i in 0..D3D12_SIMULTANEOUS_RENDER_TARGET_COUNT as usize {
            pso_desc.BlendState.RenderTarget[i] = D3D12_RENDER_TARGET_BLEND_DESC {
                BlendEnable: FALSE,
                LogicOpEnable: FALSE,
                BlendOp: D3D12_BLEND_OP_ADD,
                BlendOpAlpha: D3D12_BLEND_OP_ADD,
                SrcBlend: D3D12_BLEND_ONE,
                DestBlend: D3D12_BLEND_ONE,
                DestBlendAlpha: D3D12_BLEND_ZERO,
                SrcBlendAlpha: D3D12_BLEND_ZERO,
                LogicOp: D3D12_LOGIC_OP_NOOP,
                RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL as _,
            }
        }

        pso_desc.DepthStencilState.DepthEnable = FALSE;
        pso_desc.DepthStencilState.StencilEnable = FALSE;
        pso_desc.SampleMask = !0;
        pso_desc.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
        pso_desc.NumRenderTargets = 1;
        pso_desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
        pso_desc.SampleDesc.Count = 1;

        // // TODO: WTF
        // let pipeline = unsafe {
        //     let mut ptr = null_mut::<ID3D12PipelineState>();
        //     let hr = device.CreateGraphicsPipelineState(
        //         &pso_desc,
        //         &ID3D12PipelineState::uuidof(),
        //         &mut ptr as *mut *mut _ as *mut *mut _,
        //     );
        //     println!("HR {:?}", hr as u32);
        //     (hr == 0).then(|| ComPtr::from_raw(ptr))
        // }
        // .expect("Unable to create root pipeline state");

        let list = unsafe {
            let mut ptr = null_mut::<ID3D12GraphicsCommandList>();
            let hr = device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                allocator.as_raw(),
                null_mut(), //pipeline.as_raw(),
                &ID3D12GraphicsCommandList::uuidof(),
                &mut ptr as *mut *mut _ as *mut *mut _,
            );
            (hr == 0).then(|| ComPtr::from_raw(ptr))
        }
        .expect("Unable to create command list");

        if unsafe { list.Close() } > 0 {
            panic!("Unable to close command list");
        }

        self.root_signature = Some(root_signature);
        // self.pipeline_state = Some(pipeline);
        self.list = Some(list);
    }

    pub fn populate_command_list(&mut self) {
        let allocator = self.allocator.as_ref().unwrap();
        let list = self.list.as_ref().unwrap();
        let resources = self.resources.as_ref().unwrap();
        let swap_chain = self.swap_chain.as_ref().unwrap();
        let current_frame = unsafe { swap_chain.GetCurrentBackBufferIndex() as usize };
        let current_resource = &resources[current_frame];
        let desc_heap = self.desc_heap.as_ref().unwrap();
        let desc_cpu = unsafe { desc_heap.GetCPUDescriptorHandleForHeapStart() };
        // let pipeline = self.pipeline_state.as_ref().unwrap();
        let root_signature = self.root_signature.as_ref().unwrap();

        if unsafe { allocator.Reset() } != 0 {
            panic!("allocator reset failed");
        }

        // TODO pInitialState: pipeline.as_raw()
        if unsafe {
            list.Reset(allocator.as_raw(), null_mut() /*pipeline.as_raw()*/)
        } > 0
        {
            panic!("Unable to reset list");
        }

        unsafe { list.SetGraphicsRootSignature(root_signature.as_raw()) };

        // // TODO:
        // let viewport = D3D12_VIEWPORT {
        //     ..unsafe { mem::zeroed() }
        // };
        // unsafe {
        //     list.RSSetViewports(1, &viewport);
        // }

        // let scrects = D3D12_RECT {
        //     ..unsafe { mem::zeroed() }
        // };
        // unsafe {
        //     list.RSSetScissorRects(1, &scrects);
        // };
        // list.set_graphics_root_shader_resource_view()
        // m_commandList->SetGraphicsRootSignature(m_rootSignature.Get());
        // m_commandList->RSSetViewports(1, &m_viewport);
        // m_commandList->RSSetScissorRects(1, &m_scissorRect);

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
        unsafe { list.ResourceBarrier(1, barriers.as_ptr()) };

        // TODO:
        // CD3DX12_CPU_DESCRIPTOR_HANDLE rtvHandle(m_rtvHeap->GetCPUDescriptorHandleForHeapStart(), m_frameIndex, m_rtvDescriptorSize);

        // set render targets
        unsafe {
            list.OMSetRenderTargets(1, &desc_cpu, 0, ptr::null());
        }
        unsafe {
            let bg: [FLOAT; 4] = [1.0, 0.2, 0.4, 0.5];
            list.ClearRenderTargetView(desc_cpu, &bg, 0, null());
        }

        // let _descriptor_inc_size = device.get_descriptor_increment_size(DescriptorHeapType::Rtv);
        // // let oo = heap.GetCPUDescriptorHandleForHeapStart();
        // list.ClearRenderTargetView(RenderTargetView, ColorRGBA, NumRects, pRects)

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
        unsafe { list.ResourceBarrier(1, barriers.as_ptr()) };

        if unsafe { list.Close() } > 0 {
            panic!("Unable to close command list");
        }
    }

    pub fn render(&mut self) {
        self.populate_command_list();
        let queue = self.queue.as_ref().unwrap();
        let list = self.list.as_ref().unwrap();
        let swap_chain = self.swap_chain.as_ref().unwrap();
        let lists = [list.as_raw().cast::<ID3D12CommandList>()];
        let hr = unsafe {
            queue.ExecuteCommandLists(lists.len() as _, lists.as_ptr());
            swap_chain.Present(1, 0)
        };
        // TODO: Present fails, probably because of things...
        // if hr != 0 {
        //     panic!("Present failed");
        // }
        println!("Render");
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
    static mut window: Window = Window {
        factory: None,
        adapter: None,
        device: None,
        queue: None,
        allocator: None,
        list: None,
        desc_heap: None,
        desc_size: None,
        comp_device: None,
        comp_target: None,
        comp_visual: None,
        resources: None,
        swap_chain: None,
        // pipeline_state: None,
        root_signature: None,
    };

    match msg {
        winuser::WM_CREATE => {
            window.create_drawing_resources(hwnd);
            window.load_assets();
            window.render();
            winuser::DefWindowProcA(hwnd, msg, wparam, lparam)
        }
        winuser::WM_PAINT => {
            window.render();
            winuser::DefWindowProcA(hwnd, msg, wparam, lparam)
            // 0
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
